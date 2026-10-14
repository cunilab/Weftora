//! The one canonical game state.
//!
//! Keys are namespaced strings such as `world.day`; the Engine never interprets them.

use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};

use serde::{Deserialize, Serialize};
use weftora_api::{StateError, Value};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Scope {
    #[default]
    Persistent,
    Session,
    Settings,
}

impl Scope {
    fn label(self) -> &'static str {
        match self {
            Self::Persistent => "persistent",
            Self::Session => "session",
            Self::Settings => "settings",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StateDefinition {
    pub default: Value,
    #[serde(default)]
    pub scope: Scope,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StateChange {
    pub key: String,
    pub scope: Scope,
    pub before: Option<Value>,
    pub after: Option<Value>,
}

/// Canonical game state. `BTreeMap` keeps iteration and serialisation order stable.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct State {
    values: BTreeMap<String, Value>,
    definitions: BTreeMap<String, StateDefinition>,
    changes: Vec<StateChange>,
}

impl State {
    pub fn new() -> Self {
        Self::default()
    }

    /// Builds a state from initial values (e.g. a manifest's `initialState`), validating every key and value.
    pub fn with_values<'a>(
        values: impl IntoIterator<Item = (&'a String, &'a Value)>,
    ) -> Result<Self, StateError> {
        let mut state = Self::new();
        for (key, value) in values {
            state.set(key, value.clone())?;
        }
        state.changes.clear();
        Ok(state)
    }

    pub fn get(&self, key: &str) -> Result<&Value, StateError> {
        self.values
            .get(key)
            .ok_or_else(|| StateError::MissingKey(key.to_string()))
    }

    /// Declares a typed key with its default and persistence scope.
    pub fn declare(&mut self, key: &str, definition: StateDefinition) -> Result<(), StateError> {
        validate_key(key)?;
        validate_value(&definition.default)?;
        if self.definitions.contains_key(key) {
            return Err(StateError::DuplicateKey(key.into()));
        }
        self.values.insert(key.into(), definition.default.clone());
        self.definitions.insert(key.into(), definition);
        Ok(())
    }

    pub fn with_definitions<'a>(
        definitions: impl IntoIterator<Item = (&'a String, &'a StateDefinition)>,
    ) -> Result<Self, StateError> {
        let mut state = Self::new();
        for (key, definition) in definitions {
            state.declare(key, definition.clone())?;
        }
        Ok(state)
    }

    /// New keys infer a persistent default; an existing declaration fixes the type until state is discarded.
    pub fn set(&mut self, key: &str, value: Value) -> Result<(), StateError> {
        validate_key(key)?;
        validate_value(&value)?;
        if let Some(definition) = self.definitions.get(key) {
            if definition.default.type_name() != value.type_name() {
                return Err(StateError::TypeMismatch {
                    key: key.into(),
                    expected: definition.default.type_name(),
                    actual: value.type_name(),
                });
            }
        } else {
            self.definitions.insert(
                key.into(),
                StateDefinition {
                    default: value.clone(),
                    scope: Scope::Persistent,
                },
            );
        }
        let before = self.values.insert(key.into(), value.clone());
        if before.as_ref() != Some(&value) {
            self.changes.push(StateChange {
                key: key.into(),
                scope: self.definitions[key].scope,
                before,
                after: Some(value),
            });
        }
        Ok(())
    }

    pub fn scope(&self, key: &str) -> Result<Scope, StateError> {
        self.definitions
            .get(key)
            .map(|d| d.scope)
            .ok_or_else(|| StateError::MissingKey(key.into()))
    }

    /// The declaration survives removal and supplies the type for a later write or default fill.
    pub fn default_value(&self, key: &str) -> Result<&Value, StateError> {
        self.definitions
            .get(key)
            .map(|definition| &definition.default)
            .ok_or_else(|| StateError::MissingKey(key.into()))
    }

    /// Restores a declared default, including a key removed earlier.
    pub fn reset(&mut self, key: &str) -> Result<(), StateError> {
        let default = self
            .definitions
            .get(key)
            .ok_or_else(|| StateError::MissingKey(key.into()))?
            .default
            .clone();
        self.set(key, default)
    }

    /// Notifications in mutation order; no-op writes and rejected writes emit nothing.
    pub fn drain_changes(&mut self) -> Vec<StateChange> {
        std::mem::take(&mut self.changes)
    }

    /// A deterministic snapshot for one scope. Save containers arrive in Phase 3.
    pub fn snapshot(&self, scope: Scope) -> BTreeMap<String, Value> {
        self.values
            .iter()
            .filter(|(key, _)| self.definitions[*key].scope == scope)
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect()
    }

    /// Includes removed declarations so restoring a save cannot resurrect their defaults.
    pub fn snapshot_entries(&self, scope: Scope) -> BTreeMap<String, Option<Value>> {
        self.definitions
            .iter()
            .filter(|(_, definition)| definition.scope == scope)
            .map(|(key, _)| (key.clone(), self.values.get(key).cloned()))
            .collect()
    }

    /// Applies known keys atomically; newly declared fields retain the receiver's defaults.
    pub fn apply_snapshot(
        &mut self,
        scope: Scope,
        entries: &BTreeMap<String, Option<Value>>,
    ) -> Result<(), StateError> {
        let mut candidate = self.clone();
        for (key, value) in entries {
            let actual = candidate.scope(key)?;
            if actual != scope {
                return Err(StateError::ScopeMismatch {
                    key: key.clone(),
                    expected: scope.label(),
                    actual: actual.label(),
                });
            }
            if let Some(value) = value {
                candidate.set(key, value.clone())?;
            } else if candidate.exists(key) {
                candidate.remove(key)?;
            }
        }
        candidate.changes.clear();
        *self = candidate;
        Ok(())
    }

    /// Adds a number to an existing `Int` or `Float` value. Missing keys and type mismatches are errors.
    pub fn add(&mut self, key: &str, amount: &Value) -> Result<(), StateError> {
        let current = self
            .values
            .get(key)
            .ok_or_else(|| StateError::MissingKey(key.to_string()))?;
        let mismatch = |expected: &'static str| StateError::TypeMismatch {
            key: key.to_string(),
            expected,
            actual: amount.type_name(),
        };
        let result = match (current, amount) {
            (Value::Int(a), Value::Int(b)) => Value::Int(
                a.checked_add(*b)
                    .ok_or_else(|| StateError::Overflow(key.to_string()))?,
            ),
            (Value::Float(a), Value::Float(b)) => Value::float(a + b)?,
            (Value::Float(a), Value::Int(b)) => Value::float(a + *b as f64)?,
            (Value::Int(_), _) => return Err(mismatch("int")),
            (Value::Float(_), _) => return Err(mismatch("float or int")),
            (other, _) => {
                return Err(StateError::TypeMismatch {
                    key: key.to_string(),
                    expected: "int or float",
                    actual: other.type_name(),
                });
            }
        };
        self.set(key, result)
    }

    pub fn remove(&mut self, key: &str) -> Result<Value, StateError> {
        let before = self
            .values
            .remove(key)
            .ok_or_else(|| StateError::MissingKey(key.into()))?;
        self.changes.push(StateChange {
            key: key.into(),
            scope: self.definitions[key].scope,
            before: Some(before.clone()),
            after: None,
        });
        Ok(before)
    }

    pub fn exists(&self, key: &str) -> bool {
        self.values.contains_key(key)
    }

    /// All entries in stable key order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &Value)> {
        self.values.iter().map(|(k, v)| (k.as_str(), v))
    }
}

fn validate_value(value: &Value) -> Result<(), StateError> {
    if let Value::Float(f) = value
        && !f.is_finite()
    {
        return Err(StateError::NonFinite);
    }
    Ok(())
}

/// Validates a state key: non-empty dot-separated segments of `[a-z0-9_]`, each starting with a letter or `_`.
/// Double underscores and underscores adjacent to dots are reserved for the Yarn mapping.
pub fn validate_key(key: &str) -> Result<(), StateError> {
    let invalid = || StateError::InvalidKey(key.to_string());
    if key.replace('.', "__").replace("__", ".") != key || key.contains("_.") || key.contains("._")
    {
        return Err(StateError::InvalidKey(format!(
            "{key} (`__` and underscores next to `.` are reserved for the Yarn mapping)"
        )));
    }
    if key.is_empty() {
        return Err(invalid());
    }
    for segment in key.split('.') {
        let mut chars = segment.chars();
        match chars.next() {
            Some(c) if c.is_ascii_lowercase() || c == '_' => {}
            _ => return Err(invalid()),
        }
        if !chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_') {
            return Err(invalid());
        }
    }
    Ok(())
}

/// Thread-safe handle to the canonical state, shared by the Yarn adapter and the player.
pub type SharedState = Arc<RwLock<State>>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_snapshot_preserves_removal_and_additive_defaults() {
        let mut original = State::new();
        original.set("removed", Value::Bool(true)).unwrap();
        original.remove("removed").unwrap();
        let entries = original.snapshot_entries(Scope::Persistent);
        assert_eq!(entries.get("removed"), Some(&None));
        let mut updated = State::new();
        updated.set("removed", Value::Bool(true)).unwrap();
        updated.set("new_field", Value::Int(7)).unwrap();
        updated.apply_snapshot(Scope::Persistent, &entries).unwrap();
        assert!(!updated.exists("removed"));
        assert_eq!(updated.default_value("removed"), Ok(&Value::Bool(true)));
        assert_eq!(updated.get("new_field"), Ok(&Value::Int(7)));
        assert!(updated.drain_changes().is_empty());
    }

    #[test]
    fn rejected_snapshot_leaves_values_declarations_and_notifications_unchanged() {
        let mut state = State::new();
        state.set("a", Value::Bool(true)).unwrap();
        state
            .declare(
                "setting",
                StateDefinition {
                    default: Value::Int(1),
                    scope: Scope::Settings,
                },
            )
            .unwrap();
        for (key, value) in [
            ("unknown", Some(Value::Bool(false))),
            ("setting", Some(Value::Int(2))),
            ("a", Some(Value::String("wrong".into()))),
            ("a", Some(Value::Float(f64::NAN))),
        ] {
            let before = state.clone();
            let entries =
                BTreeMap::from([("a".into(), Some(Value::Bool(false))), (key.into(), value)]);
            assert!(state.apply_snapshot(Scope::Persistent, &entries).is_err());
            assert_eq!(state, before);
        }
    }

    #[test]
    fn key_validation() {
        for ok in [
            "a",
            "world.day",
            "_x",
            "a_1.b2",
            "character.alice.affection",
        ] {
            assert!(validate_key(ok).is_ok(), "{ok}");
        }
        for bad in [
            "", ".", "a.", ".a", "a..b", "1a", "A", "a-b", "a b", "a.1b", "é",
        ] {
            assert!(validate_key(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn double_underscore_is_reserved() {
        for key in ["a__b", "a_.b", "a._b", "__", "world.a__b"] {
            let error = validate_key(key).unwrap_err();
            assert!(error.to_string().contains("reserved for the Yarn mapping"));
            assert!(State::new().set(key, Value::Bool(true)).is_err());
        }
        for key in [
            "a.b",
            "world.day",
            "character.alice.affection",
            "_private.x",
        ] {
            assert!(validate_key(key).is_ok(), "{key}");
        }
    }

    #[test]
    fn set_get_remove_exists() {
        let mut s = State::new();
        assert_eq!(s.get("a"), Err(StateError::MissingKey("a".into())));
        s.set("a", Value::Int(1)).unwrap();
        assert!(s.exists("a"));
        assert_eq!(s.get("a"), Ok(&Value::Int(1)));
        assert_eq!(s.remove("a"), Ok(Value::Int(1)));
        assert!(!s.exists("a"));
        assert!(s.set("Bad", Value::Bool(true)).is_err());
        assert_eq!(
            s.set("f", Value::Float(f64::NAN)),
            Err(StateError::NonFinite)
        );
    }

    #[test]
    fn add_numbers() {
        let mut s = State::new();
        s.set("i", Value::Int(i64::MAX - 1)).unwrap();
        s.set("f", Value::Float(1.5)).unwrap();
        s.set("b", Value::Bool(true)).unwrap();
        s.add("i", &Value::Int(1)).unwrap();
        assert_eq!(s.get("i"), Ok(&Value::Int(i64::MAX)));
        assert_eq!(
            s.add("i", &Value::Int(1)),
            Err(StateError::Overflow("i".into()))
        );
        assert!(matches!(
            s.add("i", &Value::Float(1.0)),
            Err(StateError::TypeMismatch { .. })
        ));
        s.add("f", &Value::Int(2)).unwrap();
        s.add("f", &Value::Float(0.5)).unwrap();
        assert_eq!(s.get("f"), Ok(&Value::Float(4.0)));
        s.set("f", Value::Float(f64::MAX)).unwrap();
        assert_eq!(
            s.add("f", &Value::Float(f64::MAX)),
            Err(StateError::NonFinite)
        );
        assert_eq!(s.get("f"), Ok(&Value::Float(f64::MAX)));
        assert!(matches!(
            s.add("b", &Value::Int(1)),
            Err(StateError::TypeMismatch { .. })
        ));
        assert!(matches!(
            s.add("nope", &Value::Int(1)),
            Err(StateError::MissingKey(_))
        ));
    }

    #[test]
    fn with_values_validates() {
        let good: BTreeMap<String, Value> = [("a.b".to_string(), Value::Int(1))].into();
        assert!(State::with_values(&good).is_ok());
        let bad: BTreeMap<String, Value> = [("A".to_string(), Value::Int(1))].into();
        assert!(State::with_values(&bad).is_err());
    }
}
