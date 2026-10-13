//! Bevy presentation adapter for the Weftora player (Phase 0 spike).

use weftora_core::SharedState;
use weftora_story::Story;
use weftora_yarn::CompiledDialogue;

/// Opens the game window and runs `story` until the window closes.
pub fn run(story: Story, dialogue: CompiledDialogue, state: SharedState) {
    todo!("P0: {story:?} {dialogue:?} {state:?}")
}
