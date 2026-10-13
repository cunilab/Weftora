//! The one canonical game state.
//!
//! Keys are namespaced strings such as `world.day`; the Engine never interprets them.

use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};

use weftora_api::{StateError, Value};

/// Canonical game state. `BTreeMap` keeps iteration and serialisation order stable.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct State {
    values: BTreeMap<String, Value>,
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
        Ok(state)
    }

    pub fn get(&self, key: &str) -> Result<&Value, StateError> {
        self.values
            .get(key)
            .ok_or_else(|| StateError::MissingKey(key.to_string()))
    }

    /// Sets a value. Rejects invalid keys and non-finite floats.
    pub fn set(&mut self, key: &str, value: Value) -> Result<(), StateError> {
        validate_key(key)?;
        if let Value::Float(f) = &value
            && !f.is_finite()
        {
            return Err(StateError::NonFinite);
        }
        self.values.insert(key.to_string(), value);
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
        self.values.insert(key.to_string(), result);
        Ok(())
    }

    pub fn remove(&mut self, key: &str) -> Option<Value> {
        self.values.remove(key)
    }

    pub fn exists(&self, key: &str) -> bool {
        self.values.contains_key(key)
    }

    /// All entries in stable key order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &Value)> {
        self.values.iter().map(|(k, v)| (k.as_str(), v))
    }
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
        assert_eq!(s.remove("a"), Some(Value::Int(1)));
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
