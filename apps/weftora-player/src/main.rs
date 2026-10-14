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
        [] => run_story(default_story_dir(), None, false),
        [dir] => run_story(PathBuf::from(dir), None, false),
        [dir, flag, artifact] if flag == "--dialogue" => {
            run_story(PathBuf::from(dir), Some(PathBuf::from(artifact)), false)
        }
        [dir, flag] if flag == "--validate" => run_story(PathBuf::from(dir), None, true),
        [dir, flag, artifact, validate] if flag == "--dialogue" && validate == "--validate" => {
            run_story(PathBuf::from(dir), Some(PathBuf::from(artifact)), true)
        }
        _ => {
            eprintln!(
                "usage: weftora-player [--version | <story-dir> [--dialogue <artifact>] [--validate]]"
            );
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

fn run_story(dir: PathBuf, artifact: Option<PathBuf>, validate: bool) -> ExitCode {
    match load(&dir, artifact.as_deref()) {
        Ok((story, dialogue, state)) => {
            if validate {
                println!(
                    "OK: compiled dialogue validated for {}",
                    story.manifest().id
                );
            } else {
                weftora_render_bevy::run(story, dialogue, state);
            }
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

fn load(
    dir: &std::path::Path,
    artifact: Option<&std::path::Path>,
) -> Result<Loaded, Vec<Diagnostic>> {
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
    let default_artifact = dir.join("..").join(weftora_yarn::ARTIFACT_FILE);
    let dialogue =
        weftora_yarn::CompiledDialogue::load_artifact(artifact.unwrap_or(&default_artifact))
            .map_err(|e| vec![Diagnostic::new("", None, e)])?;
    dialogue
        .verify_sources(&sources)
        .map_err(|e| vec![Diagnostic::new("", None, e)])?;
    let errors = weftora_yarn::check_story(&story, &dialogue);
    if !errors.is_empty() {
        return Err(errors);
    }
    Ok((story, dialogue, state))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    const ARTIFACT: &[u8] =
        include_bytes!("../../../crates/weftora-yarn/tests/fixtures/budget.json");
    const SOURCE: &str =
        "title: Start\n---\nFirst #line:budget-first\nSecond #line:budget-second\n===\n";

    fn package() -> (tempfile::TempDir, PathBuf) {
        let tmp = tempfile::tempdir().unwrap();
        let story = tmp.path().join("story");
        fs::create_dir(&story).unwrap();
        fs::create_dir(tmp.path().join("compiled")).unwrap();
        fs::write(story.join("manifest.jsonc"), r#"{"manifestSchema":0,"id":"dev.weftora.artifact","storyVersion":"0.1.0","title":"Artifact","entrypoint":"Start","dialogue":["test.yarn"]}"#).unwrap();
        fs::write(story.join("test.yarn"), SOURCE).unwrap();
        fs::write(tmp.path().join(weftora_yarn::ARTIFACT_FILE), ARTIFACT).unwrap();
        (tmp, story)
    }

    #[test]
    fn packaged_and_explicit_preview_artifacts_load_without_a_compiler() {
        let (tmp, story) = package();
        let (_, dialogue, state) = load(&story, None).unwrap();
        let mut runner = weftora_yarn::DialogueRunner::new(&dialogue, state);
        runner.start("Start").unwrap();
        assert!(
            matches!(runner.next_event().unwrap(), weftora_yarn::DialogueEvent::Line {text, ..} if text=="First")
        );
        let preview = tmp.path().join("preview.json");
        fs::rename(tmp.path().join(weftora_yarn::ARTIFACT_FILE), &preview).unwrap();
        assert!(load(&story, Some(&preview)).is_ok());
        assert!(
            load(&story, None).unwrap_err()[0]
                .message
                .contains("use `weftora run")
        );
    }

    #[test]
    fn player_rejects_unknown_format_stale_sources_and_missing_node_before_window() {
        let (tmp, story) = package();
        let artifact = tmp.path().join(weftora_yarn::ARTIFACT_FILE);
        fs::write(
            &artifact,
            br#"{"dialogueArtifactFormat":999,"futureBody":null}"#,
        )
        .unwrap();
        assert!(
            load(&story, None).unwrap_err()[0]
                .message
                .contains("unsupported dialogueArtifactFormat 999")
        );
        fs::write(&artifact, ARTIFACT).unwrap();
        fs::write(story.join("test.yarn"), SOURCE.replace("First", "Edited")).unwrap();
        assert!(
            load(&story, None).unwrap_err()[0]
                .message
                .contains("stale dialogue artifact")
        );
        fs::write(story.join("test.yarn"), SOURCE).unwrap();
        let manifest = story.join("manifest.jsonc");
        fs::write(
            &manifest,
            fs::read_to_string(&manifest)
                .unwrap()
                .replace("\"entrypoint\":\"Start\"", "\"entrypoint\":\"Missing\""),
        )
        .unwrap();
        assert!(
            load(&story, None).unwrap_err()[0]
                .to_string()
                .contains("manifest.jsonc: entrypoint `Missing`")
        );
    }

    #[test]
    fn oversized_artifact_file_is_rejected_by_the_player() {
        let (tmp, story) = package();
        let file = fs::File::create(tmp.path().join(weftora_yarn::ARTIFACT_FILE)).unwrap();
        file.set_len(weftora_yarn::MAX_ARTIFACT_BYTES as u64 + 1)
            .unwrap();
        assert!(
            load(&story, None).unwrap_err()[0]
                .message
                .contains("exceeds 16 MiB")
        );
    }
}
