//! Typed commands with caller-driven futures and cancellation by dropping pending work.

use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use weftora_api::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueType {
    Bool,
    Int,
    Float,
    String,
}
impl ValueType {
    fn accepts(self, value: &Value) -> bool {
        matches!(
            (self, value),
            (Self::Bool, Value::Bool(_))
                | (Self::Int, Value::Int(_))
                | (Self::String, Value::String(_))
        ) || matches!((self, value), (Self::Float, Value::Float(f)) if f.is_finite())
    }
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum CommandError {
    #[error("unknown command `{0}`")]
    Unknown(String),
    #[error("command `{0}` already registered")]
    Duplicate(String),
    #[error("invalid command ID `{0}`")]
    InvalidId(String),
    #[error("command `{command}` expected {expected} arguments, got {actual}")]
    Arity {
        command: String,
        expected: usize,
        actual: usize,
    },
    #[error("command `{command}` argument {index} expected {expected:?}, got {actual}")]
    Argument {
        command: String,
        index: usize,
        expected: ValueType,
        actual: &'static str,
    },
    #[error("command `{command}` returned {actual}, expected {expected:?}")]
    Result {
        command: String,
        expected: ValueType,
        actual: &'static str,
    },
    #[error("command failed: {0}")]
    Failed(String),
    #[error("command cancelled")]
    Cancelled,
}

pub type CommandFuture = Pin<Box<dyn Future<Output = Result<Value, CommandError>>>>;
type Handler = Box<dyn Fn(Vec<Value>) -> CommandFuture>;
struct Command {
    arguments: Vec<ValueType>,
    result: ValueType,
    handler: Handler,
}
#[derive(Default)]
pub struct CommandRegistry {
    commands: BTreeMap<String, Command>,
}

impl CommandRegistry {
    pub fn register(
        &mut self,
        id: &str,
        arguments: Vec<ValueType>,
        result: ValueType,
        handler: impl Fn(Vec<Value>) -> CommandFuture + 'static,
    ) -> Result<(), CommandError> {
        crate::state::validate_key(id).map_err(|_| CommandError::InvalidId(id.into()))?;
        if self.commands.contains_key(id) {
            return Err(CommandError::Duplicate(id.into()));
        }
        self.commands.insert(
            id.into(),
            Command {
                arguments,
                result,
                handler: Box::new(handler),
            },
        );
        Ok(())
    }

    pub fn start(&self, id: &str, arguments: Vec<Value>) -> Result<CommandTask, CommandError> {
        let command = self
            .commands
            .get(id)
            .ok_or_else(|| CommandError::Unknown(id.into()))?;
        if arguments.len() != command.arguments.len() {
            return Err(CommandError::Arity {
                command: id.into(),
                expected: command.arguments.len(),
                actual: arguments.len(),
            });
        }
        for (index, (value, expected)) in arguments.iter().zip(&command.arguments).enumerate() {
            if !expected.accepts(value) {
                return Err(CommandError::Argument {
                    command: id.into(),
                    index,
                    expected: *expected,
                    actual: value.type_name(),
                });
            }
        }
        Ok(CommandTask {
            command: id.into(),
            expected: command.result,
            future: Some((command.handler)(arguments)),
            result: None,
        })
    }
}

pub struct CommandTask {
    command: String,
    expected: ValueType,
    future: Option<CommandFuture>,
    result: Option<Result<Value, CommandError>>,
}
impl CommandTask {
    pub fn poll(&mut self, context: &mut Context<'_>) -> Poll<Result<Value, CommandError>> {
        if let Some(result) = &self.result {
            return Poll::Ready(result.clone());
        }
        let Some(future) = &mut self.future else {
            return Poll::Ready(Err(CommandError::Cancelled));
        };
        match future.as_mut().poll(context) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(result) => {
                let result = result.and_then(|value| {
                    if self.expected.accepts(&value) {
                        Ok(value)
                    } else {
                        Err(CommandError::Result {
                            command: self.command.clone(),
                            expected: self.expected,
                            actual: value.type_name(),
                        })
                    }
                });
                self.future = None;
                self.result = Some(result.clone());
                Poll::Ready(result)
            }
        }
    }
    /// Pending futures are dropped immediately, releasing owned resources. Completed results are preserved.
    pub fn cancel(&mut self) {
        if self.result.is_none() {
            self.future = None;
            self.result = Some(Err(CommandError::Cancelled));
        }
    }
}
