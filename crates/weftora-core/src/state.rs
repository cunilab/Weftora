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
        todo!("P0: return the value or StateError::MissingKey; key = {key}")
    }

    /// Sets a value. Rejects invalid keys and non-finite floats.
    pub fn set(&mut self, key: &str, value: Value) -> Result<(), StateError> {
        todo!(
            "P0: validate key with `validate_key`, reject non-finite floats, insert; {key} {value:?}"
        )
    }

    /// Adds a number to an existing `Int` or `Float` value. Missing keys and type mismatches are errors.
    pub fn add(&mut self, key: &str, amount: &Value) -> Result<(), StateError> {
        todo!("P0: {key} {amount:?}")
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
pub fn validate_key(key: &str) -> Result<(), StateError> {
    todo!("P0: {key}")
}

/// Thread-safe handle to the canonical state, shared by the Yarn adapter and the player.
pub type SharedState = Arc<RwLock<State>>;
