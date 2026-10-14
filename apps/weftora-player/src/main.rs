//! The prebuilt Weftora player: runs a Story folder.

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::{Arc, RwLock};

use sha2::{Digest, Sha256};
use weftora_api::Diagnostic;
use weftora_story::Story;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [flag] if flag == "--version" => match version_line() {
            Ok(line) => {
                println!("{line}");
                ExitCode::SUCCESS
            }
            Err(message) => {
                eprintln!("weftora-player: {message}");
                ExitCode::FAILURE
            }
        },
        [] => run_story(default_story_dir()),
        [dir] => run_story(PathBuf::from(dir)),
        _ => {
            eprintln!("usage: weftora-player [--version | <story-dir>]");
            ExitCode::from(2)
        }
    }
}

/// `story/` next to the executable, the layout `weftora export` produces.
fn default_story_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join("story")))
        .unwrap_or_else(|| PathBuf::from("story"))
}

fn version_line() -> Result<String, String> {
    let exe = std::env::current_exe().map_err(|e| format!("cannot locate executable: {e}"))?;
    let bytes = std::fs::read(&exe).map_err(|e| format!("cannot read {}: {e}", exe.display()))?;
    let digest = Sha256::digest(&bytes);
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    Ok(format!(
        "weftora-player {} (sha256 {hex})",
        env!("CARGO_PKG_VERSION")
    ))
}

fn run_story(dir: PathBuf) -> ExitCode {
    match load(&dir) {
        Ok((story, dialogue, state)) => {
            weftora_render_bevy::run(story, dialogue, state);
            ExitCode::SUCCESS
        }
        Err(diagnostics) => {
            for diagnostic in diagnostics {
                eprintln!("{diagnostic}");
            }
            ExitCode::FAILURE
        }
    }
}

type Loaded = (
    Story,
    weftora_yarn::CompiledDialogue,
    weftora_core::SharedState,
);

fn load(dir: &std::path::Path) -> Result<Loaded, Vec<Diagnostic>> {
    let story = Story::load(dir)?;
    let state = story.initial_state().map_err(|e| {
        vec![Diagnostic::new(
            weftora_story::MANIFEST_FILE,
            None,
            e.to_string(),
        )]
    })?;
    let state = Arc::new(RwLock::new(state));
    let sources = story.dialogue_sources()?;
    let dialogue = weftora_yarn::compile(&sources)?;
    Ok((story, dialogue, state))
}
