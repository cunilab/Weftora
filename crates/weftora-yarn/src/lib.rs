//! Yarn Spinner adapter for Weftora (architecture §6).
//!
//! Uses the Bevy-independent `yarnspinner` crates only. Yarn variables are a view onto the canonical
//! [`SharedState`]; there is no separate Yarn variable store.
//!
//! Variable mapping (provisional, Phase 0): Yarn `$name` ↔ state key `name`, with `__` in the Yarn name
//! standing for `.` in the key (`$world__day` ↔ `world.day`). `bool` and `string` map directly. `Float`
//! maps to Yarn's `f32` number. `Int` maps to number only within ±2^24; reading a larger `Int` is an error.
//! A Yarn number written to a key that holds an `Int` must be integral, otherwise it is a type error.

use weftora_api::{Diagnostic, StoryCommand};
use weftora_core::SharedState;
use weftora_story::DialogueSource;

/// Dialogue compiled from a Story's Yarn files.
pub struct CompiledDialogue {
    // private: yarnspinner program + string table
}

impl std::fmt::Debug for CompiledDialogue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CompiledDialogue").finish_non_exhaustive()
    }
}

impl CompiledDialogue {
    /// Names of all nodes, sorted.
    pub fn node_names(&self) -> Vec<String> {
        todo!("P0")
    }
}

/// Compiles Yarn sources. Errors carry the Story-relative file and line.
pub fn compile(sources: &[DialogueSource]) -> Result<CompiledDialogue, Vec<Diagnostic>> {
    todo!("P0: {sources:?}")
}

/// One thing the player must present or do.
#[derive(Debug, Clone, PartialEq)]
pub enum DialogueEvent {
    /// Show a line; the player waits for the user before calling [`DialogueRunner::next_event`] again.
    Line {
        character: Option<String>,
        text: String,
    },
    /// Show choices; the player calls [`DialogueRunner::choose`] with one `index`, then `next_event`.
    Choices(Vec<Choice>),
    /// Execute a Story command (e.g. `background bg.room`), then call `next_event` again immediately.
    Command(StoryCommand),
    /// The dialogue has finished.
    End,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Choice {
    pub index: usize,
    pub text: String,
    /// `false` when the option's Yarn condition failed; the player shows it disabled or hides it.
    pub available: bool,
}

/// Runtime errors from dialogue execution.
#[derive(Debug, thiserror::Error)]
pub enum DialogueError {
    #[error("dialogue node `{0}` does not exist")]
    MissingNode(String),
    #[error("choice {0} is not one of the offered choices")]
    InvalidChoice(usize),
    #[error("Yarn variable `{variable}`: {message}")]
    Variable { variable: String, message: String },
    #[error("dialogue error: {0}")]
    Runtime(String),
}

/// Runs compiled dialogue against canonical state, one presentation event at a time.
pub struct DialogueRunner {
    // private: yarnspinner Dialogue + pending event queue
}

impl std::fmt::Debug for DialogueRunner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DialogueRunner").finish_non_exhaustive()
    }
}

impl DialogueRunner {
    pub fn new(dialogue: &CompiledDialogue, state: SharedState) -> Self {
        todo!("P0: {dialogue:?} {state:?}")
    }

    /// Starts (or restarts) the dialogue at `node`.
    pub fn start(&mut self, node: &str) -> Result<(), DialogueError> {
        todo!("P0: {node}")
    }

    /// Returns the next event. After [`DialogueEvent::End`] it keeps returning `End` until `start` is called.
    pub fn next_event(&mut self) -> Result<DialogueEvent, DialogueError> {
        todo!("P0")
    }

    /// Selects one of the choices from the last [`DialogueEvent::Choices`].
    pub fn choose(&mut self, index: usize) -> Result<(), DialogueError> {
        todo!("P0: {index}")
    }
}
