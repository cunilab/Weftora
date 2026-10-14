//! Yarn `VariableStorage` backed by the canonical Weftora state (architecture §6).

use std::any::Any;
use std::fmt;
use std::sync::{Arc, RwLock};

use bevy_platform::collections::HashMap;
use weftora_api::Value;
use weftora_core::SharedState;
use yarnspinner::core::YarnValue;
use yarnspinner::runtime::{VariableStorage, VariableStorageError};

use crate::DialogueError;

/// Largest integer magnitude an `f32` represents exactly.
pub(crate) const MAX_EXACT_INT: i64 = 1 << 24;

/// Yarn's own bookkeeping variables (for example `$Yarn.Internal.Visiting.Node`). They are not Story state,
/// so they live beside it and are never saved or visible through canonical keys.
const INTERNAL_PREFIX: &str = "$Yarn.Internal.";

type Result<T> = std::result::Result<T, VariableStorageError>;

/// A variable-level failure carried through Yarn's storage error so the runner can name the variable.
#[derive(Debug)]
struct VarError {
    variable: String,
    message: String,
}

impl fmt::Display for VarError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.variable, self.message)
    }
}

impl std::error::Error for VarError {}

fn var_error(variable: &str, message: impl Into<String>) -> VariableStorageError {
    VariableStorageError::InternalError {
        error: Box::new(VarError {
            variable: variable.to_string(),
            message: message.into(),
        }),
    }
}

pub(crate) fn map_storage_error(error: VariableStorageError) -> DialogueError {
    match error {
        VariableStorageError::InternalError { error } => match error.downcast::<VarError>() {
            Ok(v) => DialogueError::Variable {
                variable: v.variable,
                message: v.message,
            },
            Err(other) => DialogueError::Runtime(other.to_string()),
        },
        VariableStorageError::InvalidVariableName { name } => DialogueError::Variable {
            variable: name,
            message: "variable names must start with `$`".into(),
        },
        VariableStorageError::VariableNotFound { name } => DialogueError::Variable {
            variable: name,
            message: "variable is not defined".into(),
        },
    }
}

/// Check declaration defaults before the upstream runtime can silently discard storage errors.
pub(crate) fn validate_default(
    name: &str,
    value: &YarnValue,
) -> std::result::Result<(), DialogueError> {
    if is_internal(name) {
        return Ok(());
    }
    key_for(name).map_err(map_storage_error)?;
    from_yarn(name, None, value).map_err(map_storage_error)?;
    Ok(())
}

/// Yarn `$a__b` to state key `a.b`.
fn key_for(name: &str) -> Result<String> {
    let stripped =
        name.strip_prefix('$')
            .ok_or_else(|| VariableStorageError::InvalidVariableName {
                name: name.to_string(),
            })?;
    let key = stripped.replace("__", ".");
    weftora_core::state::validate_key(&key).map_err(|e| var_error(name, e.to_string()))?;
    Ok(key)
}

/// State key `a.b` to Yarn `$a__b`.
fn name_for(key: &str) -> String {
    format!("${}", key.replace('.', "__"))
}

fn to_yarn(name: &str, value: &Value) -> Result<YarnValue> {
    match value {
        Value::Bool(b) => Ok(YarnValue::Boolean(*b)),
        Value::String(s) => Ok(YarnValue::String(s.clone())),
        Value::Int(i) if (-MAX_EXACT_INT..=MAX_EXACT_INT).contains(i) => {
            Ok(YarnValue::Number(*i as f32))
        }
        Value::Int(i) => Err(var_error(
            name,
            format!("int {i} is outside ±{MAX_EXACT_INT} and cannot be read exactly by Yarn"),
        )),
        Value::Float(f) => {
            let narrowed = *f as f32;
            if narrowed.is_finite() {
                Ok(YarnValue::Number(narrowed))
            } else {
                Err(var_error(
                    name,
                    format!("float {f} does not fit Yarn's f32"),
                ))
            }
        }
    }
}

fn from_yarn(name: &str, existing: Option<&Value>, value: &YarnValue) -> Result<Value> {
    let mismatch = |expected: &str| {
        var_error(
            name,
            format!(
                "state holds a {expected}, cannot store a Yarn {}",
                yarn_type(value)
            ),
        )
    };
    match (existing, value) {
        (Some(Value::Bool(_)) | None, YarnValue::Boolean(b)) => Ok(Value::Bool(*b)),
        (Some(Value::String(_)) | None, YarnValue::String(s)) => Ok(Value::String(s.clone())),
        (Some(Value::Int(_)), YarnValue::Number(n)) => {
            if !n.is_finite() || n.fract() != 0.0 {
                return Err(var_error(
                    name,
                    format!("{n} is not an integer but the state key holds an int"),
                ));
            }
            if n.abs() > MAX_EXACT_INT as f32 {
                return Err(var_error(
                    name,
                    format!("{n} is outside ±{MAX_EXACT_INT} for an int key"),
                ));
            }
            Ok(Value::Int(*n as i64))
        }
        (Some(Value::Float(_)) | None, YarnValue::Number(n)) => {
            Value::float(f64::from(*n)).map_err(|e| var_error(name, e.to_string()))
        }
        (Some(other), _) => Err(mismatch(other.type_name())),
    }
}

fn yarn_type(value: &YarnValue) -> &'static str {
    match value {
        YarnValue::Number(_) => "number",
        YarnValue::String(_) => "string",
        YarnValue::Boolean(_) => "bool",
    }
}

fn poisoned(name: &str) -> VariableStorageError {
    var_error(name, "state lock is poisoned")
}

#[derive(Debug, Clone)]
pub(crate) struct StateStorage {
    state: SharedState,
    internal: Arc<RwLock<HashMap<String, YarnValue>>>,
}

impl StateStorage {
    pub(crate) fn new(state: SharedState) -> Self {
        Self {
            state,
            internal: Arc::default(),
        }
    }
}

fn is_internal(name: &str) -> bool {
    name.starts_with(INTERNAL_PREFIX)
}

impl VariableStorage for StateStorage {
    fn clone_shallow(&self) -> Box<dyn VariableStorage> {
        Box::new(self.clone())
    }

    fn set(&mut self, name: String, value: YarnValue) -> Result<()> {
        if is_internal(&name) {
            self.internal
                .write()
                .map_err(|_| poisoned(&name))?
                .insert(name, value);
            return Ok(());
        }
        let key = key_for(&name)?;
        let mut state = self.state.write().map_err(|_| poisoned(&name))?;
        let converted = from_yarn(&name, state.get(&key).ok(), &value)?;
        state
            .set(&key, converted)
            .map_err(|e| var_error(&name, e.to_string()))
    }

    fn get(&self, name: &str) -> Result<YarnValue> {
        if is_internal(name) {
            return self
                .internal
                .read()
                .map_err(|_| poisoned(name))?
                .get(name)
                .cloned()
                .ok_or_else(|| VariableStorageError::VariableNotFound {
                    name: name.to_string(),
                });
        }
        let key = key_for(name)?;
        let state = self.state.read().map_err(|_| poisoned(name))?;
        match state.get(&key) {
            Ok(value) => to_yarn(name, value),
            Err(weftora_api::StateError::MissingKey(_)) => {
                Err(VariableStorageError::VariableNotFound {
                    name: name.to_string(),
                })
            }
            Err(e) => Err(var_error(name, e.to_string())),
        }
    }

    fn contains(&self, name: &str) -> bool {
        if is_internal(name) {
            return self.internal.read().is_ok_and(|m| m.contains_key(name));
        }
        match (key_for(name), self.state.read()) {
            (Ok(key), Ok(state)) => state.exists(&key),
            _ => false,
        }
    }

    /// Declared defaults only fill in missing keys; they never overwrite state that Rust or a save already set.
    fn extend(&mut self, values: HashMap<String, YarnValue>) -> Result<()> {
        let mut first_error = None;
        let mut values: Vec<_> = values.into_iter().collect();
        values.sort_by(|(a, _), (b, _)| a.cmp(b));
        for (name, value) in values {
            if !self.contains(&name)
                && let Err(error) = self.set(name, value)
                && first_error.is_none()
            {
                first_error = Some(error);
            }
        }
        match first_error {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    fn variables(&self) -> HashMap<String, YarnValue> {
        let mut out: HashMap<String, YarnValue> =
            self.internal.read().map(|m| m.clone()).unwrap_or_default();
        if let Ok(state) = self.state.read() {
            for (key, value) in state.iter() {
                let name = name_for(key);
                if let Ok(v) = to_yarn(&name, value) {
                    out.insert(name, v);
                }
            }
        }
        out
    }

    /// Clears only Yarn's internal bookkeeping; canonical state is never wiped by Yarn.
    fn clear(&mut self) {
        if let Ok(mut internal) = self.internal.write() {
            internal.clear();
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use weftora_core::State;

    #[test]
    fn integer_conversion_checks_extremes_and_exact_boundaries() {
        for value in [i64::MIN, i64::MAX, -MAX_EXACT_INT - 1, MAX_EXACT_INT + 1] {
            assert!(to_yarn("$n", &Value::Int(value)).is_err());
        }
        for value in [-MAX_EXACT_INT, MAX_EXACT_INT] {
            assert_eq!(
                to_yarn("$n", &Value::Int(value)).unwrap(),
                YarnValue::Number(value as f32)
            );
        }
    }

    #[test]
    fn invalid_defaults_do_not_skip_valid_defaults() {
        let state = Arc::new(RwLock::new(State::new()));
        state
            .write()
            .unwrap()
            .set("existing", Value::Bool(true))
            .unwrap();
        let mut storage = StateStorage::new(state.clone());
        let defaults = HashMap::from([
            ("$Bad".into(), YarnValue::Boolean(true)),
            ("missing-dollar".into(), YarnValue::Boolean(true)),
            ("$a___b".into(), YarnValue::Boolean(true)),
            ("$existing".into(), YarnValue::Boolean(false)),
            ("$flag".into(), YarnValue::Boolean(true)),
            ("$name".into(), YarnValue::String("Ann".into())),
            ("$world__day".into(), YarnValue::Number(2.0)),
        ]);
        assert!(storage.extend(defaults).is_err());
        let state = state.read().unwrap();
        assert_eq!(state.get("existing"), Ok(&Value::Bool(true)));
        assert_eq!(state.get("flag"), Ok(&Value::Bool(true)));
        assert_eq!(state.get("name"), Ok(&Value::String("Ann".into())));
        assert_eq!(state.get("world.day"), Ok(&Value::Float(2.0)));
    }
}
