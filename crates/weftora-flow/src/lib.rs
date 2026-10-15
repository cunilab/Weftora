//! Headless Story conditions. File formats and event scheduling are separate concerns.

use std::cmp::Ordering;
use weftora_api::{StateError, Value};
use weftora_core::State;

/// The root counts as depth one; comparison operands do not count as condition nodes.
pub const MAX_DEPTH: usize = 32;
pub const MAX_NODES: usize = 256;

#[derive(Debug, Clone, PartialEq)]
pub enum Operand {
    Literal(Value),
    State(String),
}

impl Operand {
    fn resolve<'a>(&'a self, state: &'a State, defaults: bool) -> Result<&'a Value, StateError> {
        match self {
            Self::Literal(value) => Ok(value),
            Self::State(key) if defaults => state.default_value(key),
            Self::State(key) => state.get(key),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompareOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Condition {
    Compare {
        op: CompareOp,
        left: Operand,
        right: Operand,
    },
    And(Vec<Condition>),
    Or(Vec<Condition>),
    Not(Box<Condition>),
}

#[derive(Debug, PartialEq, thiserror::Error)]
pub enum ConditionError {
    #[error(transparent)]
    State(#[from] StateError),
    #[error("condition {op:?} does not accept {left} and {right}")]
    Operands {
        op: CompareOp,
        left: &'static str,
        right: &'static str,
    },
    #[error("and/or conditions must contain at least one child")]
    Empty,
    #[error("condition depth exceeds {MAX_DEPTH}")]
    Depth,
    #[error("condition node count exceeds {MAX_NODES}")]
    Nodes,
}

impl Condition {
    /// Checks every branch against declared state types, even branches evaluation would skip.
    /// Removed keys retain their declarations and are valid here, but error if read at runtime.
    pub fn validate(&self, state: &State) -> Result<(), ConditionError> {
        let mut remaining = MAX_NODES;
        self.validate_node(state, 1, &mut remaining)
    }

    fn validate_node(
        &self,
        state: &State,
        depth: usize,
        remaining: &mut usize,
    ) -> Result<(), ConditionError> {
        if depth > MAX_DEPTH {
            return Err(ConditionError::Depth);
        }
        *remaining = remaining.checked_sub(1).ok_or(ConditionError::Nodes)?;
        match self {
            Self::Compare { op, left, right } => {
                compare(*op, left.resolve(state, true)?, right.resolve(state, true)?)?;
            }
            Self::And(children) | Self::Or(children) => {
                if children.is_empty() {
                    return Err(ConditionError::Empty);
                }
                for child in children {
                    child.validate_node(state, depth + 1, remaining)?;
                }
            }
            Self::Not(child) => child.validate_node(state, depth + 1, remaining)?,
        }
        Ok(())
    }

    /// Validates the entire tree, then evaluates in declaration order with short-circuiting.
    /// Values must have matching types; numeric conversions are never implicit.
    pub fn evaluate(&self, state: &State) -> Result<bool, ConditionError> {
        self.validate(state)?;
        self.evaluate_validated(state)
    }

    fn evaluate_validated(&self, state: &State) -> Result<bool, ConditionError> {
        match self {
            Self::Compare { op, left, right } => compare(
                *op,
                left.resolve(state, false)?,
                right.resolve(state, false)?,
            ),
            Self::And(children) => {
                for child in children {
                    if !child.evaluate_validated(state)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            Self::Or(children) => {
                for child in children {
                    if child.evaluate_validated(state)? {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
            Self::Not(child) => Ok(!child.evaluate_validated(state)?),
        }
    }
}

fn compare(op: CompareOp, left: &Value, right: &Value) -> Result<bool, ConditionError> {
    for value in [left, right] {
        if let Value::Float(value) = value
            && !value.is_finite()
        {
            return Err(StateError::NonFinite.into());
        }
    }
    let ordering = match (left, right) {
        (Value::Bool(a), Value::Bool(b)) if matches!(op, CompareOp::Eq | CompareOp::Ne) => {
            Some(a.cmp(b))
        }
        (Value::Int(a), Value::Int(b)) => Some(a.cmp(b)),
        (Value::Float(a), Value::Float(b)) => a.partial_cmp(b),
        (Value::String(a), Value::String(b)) => Some(a.cmp(b)),
        _ => None,
    }
    .ok_or(ConditionError::Operands {
        op,
        left: left.type_name(),
        right: right.type_name(),
    })?;
    Ok(match op {
        CompareOp::Eq => ordering == Ordering::Equal,
        CompareOp::Ne => ordering != Ordering::Equal,
        CompareOp::Lt => ordering == Ordering::Less,
        CompareOp::Le => ordering != Ordering::Greater,
        CompareOp::Gt => ordering == Ordering::Greater,
        CompareOp::Ge => ordering != Ordering::Less,
    })
}
