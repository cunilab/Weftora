//! Headless Weftora kernel.
//!
//! Must never depend on Bevy, Yarn Spinner, Rhai or any Story.

pub mod catalog;
pub mod command;
pub mod lifecycle;
pub mod presentation;
pub mod signal;
pub mod state;

pub use state::{Scope, SharedState, State, StateChange, StateDefinition};
