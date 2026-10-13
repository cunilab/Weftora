//! Story-facing Weftora types shared by every Engine crate.
//!
//! This crate must never depend on Bevy, Yarn Spinner, Rhai or any Story.

use serde::{Deserialize, Serialize};

/// A typed game-state value.
///
/// Phase 0 supports scalars only. Lists and maps arrive in Phase 4 (ADR 0003).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Value {
    Bool(bool),
    Int(i64),
    /// Always finite; constructors reject NaN and infinity.
    Float(f64),
    String(String),
}

impl Value {
    /// Builds a float value, rejecting NaN and infinity.
    pub fn float(value: f64) -> Result<Self, StateError> {
        if value.is_finite() {
            Ok(Self::Float(value))
        } else {
            Err(StateError::NonFinite)
        }
    }

    /// Human-readable type name used in diagnostics.
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Bool(_) => "bool",
            Self::Int(_) => "int",
            Self::Float(_) => "float",
            Self::String(_) => "string",
        }
    }
}

/// Errors from reading or writing game state.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum StateError {
    #[error("state key `{0}` does not exist")]
    MissingKey(String),
    #[error("state key `{key}` holds a {actual}, expected {expected}")]
    TypeMismatch {
        key: String,
        expected: &'static str,
        actual: &'static str,
    },
    #[error("float values must be finite")]
    NonFinite,
    #[error("invalid state key `{0}`: use lowercase letters, digits, `_` and `.` separators")]
    InvalidKey(String),
    #[error("integer overflow while adding to state key `{0}`")]
    Overflow(String),
}

/// An author-facing error or warning that points into a Story file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    /// Path relative to the Story root, using `/` separators, e.g. `dialogue/intro.yarn`.
    /// `manifest.json` for manifest problems; empty only when no file applies.
    pub file: String,
    /// 1-based line number when known.
    pub line: Option<u32>,
    pub message: String,
}

impl Diagnostic {
    pub fn new(file: impl Into<String>, line: Option<u32>, message: impl Into<String>) -> Self {
        Self {
            file: file.into(),
            line,
            message: message.into(),
        }
    }
}

impl std::fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.line {
            Some(line) => write!(f, "{}:{}: {}", self.file, line, self.message),
            None if self.file.is_empty() => write!(f, "{}", self.message),
            None => write!(f, "{}: {}", self.file, self.message),
        }
    }
}

/// A presentation command requested by Story content (for example from a Yarn `<<background bg.room>>` line).
///
/// Phase 0 commands: `background <assetId>` and `sound <assetId>`. The player rejects anything else.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoryCommand {
    pub name: String,
    pub args: Vec<String>,
}
