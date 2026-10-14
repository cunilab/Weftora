//! Headless Weftora kernel.
//!
//! Must never depend on Bevy, Yarn Spinner, Rhai or any Story.

pub mod state;

pub use state::{SharedState, State};
