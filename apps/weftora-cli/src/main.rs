//! The `weftora` command-line tool: check, run and export Story folders.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use sha2::{Digest, Sha256};
use weftora_story::Story;
use weftora_yarn::{CompiledDialogue, check_state, compile};

const VERSION: &str = env!("CARGO_PKG_VERSION");
const PLAYER_ENV: &str = "WEFTORA_PLAYER";
const EXPORT_FORMAT: u32 = 0;

const USAGE: &str = "usage:
  weftora --version
  weftora check <story-dir>
  weftora run <story-dir>
  weftora export <story-dir> --out <dir>";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["--version"] | ["-V"] => version(),
        ["check", dir] => check_command(Path::new(dir)),
        ["run", dir] => run_command(Path::new(dir)),
        ["export", dir, "--out", out] | ["export", "--out", out, dir] => {
            export_command(Path::new(dir), Path::new(out))
        }
        _ => Err(Failure::Usage),
    };
    match result {
        Ok(code) => code,
        Err(Failure::Usage) => {
            eprintln!("{USAGE}");
            ExitCode::from(2)
        }
        Err(Failure::Messages(messages)) => {
            for message in messages {
                eprintln!("{message}");
            }
            ExitCode::from(1)
        }
    }
}

enum Failure {
    Usage,
    Messages(Vec<String>),
}

impl Failure {
    fn one(message: impl Into<String>) -> Self {
        Self::Messages(vec![message.into()])
    }
}

type CliResult = Result<ExitCode, Failure>;

fn version() -> CliResult {
    let exe = std::env::current_exe().map_err(|e| Failure::one(format!("error: {e}")))?;
    let hash = sha256_file(&exe)
        .map_err(|e| Failure::one(format!("error: hashing {}: {e}", exe.display())))?;
    println!("weftora {VERSION} (sha256 {hash})");
    Ok(ExitCode::SUCCESS)
}

struct Checked {
    story: Story,
    dialogue: CompiledDialogue,
}

/// Loads and compiles the Story and validates its commands. Errors are `file:line: message` strings.
fn check(dir: &Path) -> Result<Checked, Vec<String>> {
    let story =
        Story::load(dir).map_err(|d| d.iter().map(ToString::to_string).collect::<Vec<_>>())?;
    let sources = story
        .dialogue_sources()
        .map_err(|d| d.iter().map(ToString::to_string).collect::<Vec<_>>())?;
    let dialogue =
        compile(&sources).map_err(|d| d.iter().map(ToString::to_string).collect::<Vec<_>>())?;

    let mut errors = Vec::new();
    match weftora_core::State::with_values(&story.manifest().initial_state) {
        Ok(state) => errors.extend(
            check_state(&dialogue, &state)
                .into_iter()
                .map(|error| format!("manifest.jsonc: {error}")),
        ),
        Err(error) => errors.push(format!("manifest.jsonc: initialState: {error}")),
    }
    let entrypoint = &story.manifest().entrypoint;
    if !dialogue.node_names().contains(entrypoint) {
        errors.push(format!(
            "manifest.jsonc: entrypoint `{entrypoint}` is not a node in the dialogue files"
        ));
    }
    for site in dialogue.command_sites() {
        if let Some(message) = command_problem(&story, &site.command) {
            let location = match site.line {
                Some(line) => format!("{}:{line}", site.file),
                None => site.file.clone(),
            };
            errors.push(format!("{location}: {message} (node `{}`)", site.node));
        }
    }
    if errors.is_empty() {
        Ok(Checked { story, dialogue })
    } else {
        Err(errors)
    }
}

fn command_problem(story: &Story, command: &weftora_api::StoryCommand) -> Option<String> {
    match command.name.as_str() {
        "background" | "sound" => match command.args.as_slice() {
            [id] if id.contains('{') => Some(format!(
                "`{}` needs a literal asset ID, expressions are not supported",
                command.name
            )),
            [id] if story.manifest().assets.contains_key(id) => None,
            [id] => Some(format!(
                "`{}` uses undeclared asset ID `{id}`",
                command.name
            )),
            args => Some(format!(
                "`{}` takes exactly one asset ID, found {} arguments",
                command.name,
                args.len()
            )),
        },
        other => Some(format!(
            "unknown command `{other}` (Phase 0 supports `background` and `sound`)"
        )),
    }
}

fn check_command(dir: &Path) -> CliResult {
    let checked = check(dir).map_err(Failure::Messages)?;
    let manifest = checked.story.manifest();
    println!(
        "OK: {} v{} - {} nodes, {} assets, {} commands",
        manifest.id,
        manifest.story_version,
        checked.dialogue.node_names().len(),
        manifest.assets.len(),
        checked.dialogue.commands().len()
    );
    Ok(ExitCode::SUCCESS)
}

fn player_path() -> Result<PathBuf, Failure> {
    let path = match std::env::var_os(PLAYER_ENV) {
        Some(p) if !p.is_empty() => PathBuf::from(p),
        _ => {
            let exe = std::env::current_exe().map_err(|e| Failure::one(format!("error: {e}")))?;
            let name = format!("weftora-player{}", std::env::consts::EXE_SUFFIX);
            exe.parent()
                .map(|d| d.join(&name))
                .unwrap_or_else(|| PathBuf::from(name))
        }
    };
    if path.is_file() {
        Ok(path)
    } else {
        Err(Failure::one(format!(
            "error: player executable not found at `{}`; build it next to `weftora` or set {PLAYER_ENV}",
            path.display()
        )))
    }
}

fn run_command(dir: &Path) -> CliResult {
    check(dir).map_err(Failure::Messages)?;
    let player = player_path()?;
    let status = Command::new(&player)
        .arg(dir)
        .status()
        .map_err(|e| Failure::one(format!("error: cannot start `{}`: {e}", player.display())))?;
    Ok(status
        .code()
        .and_then(|c| u8::try_from(c).ok())
        .map_or(ExitCode::from(1), ExitCode::from))
}

fn export_command(dir: &Path, out: &Path) -> CliResult {
    let checked = check(dir).map_err(Failure::Messages)?;
    let player = player_path()?;
    let io_err = |what: &str, e: io::Error| Failure::one(format!("error: {what}: {e}"));

    match fs::read_dir(out) {
        Ok(entries) => {
            if entries.count() > 0 {
                return Err(Failure::one(format!(
                    "error: output directory `{}` is not empty",
                    out.display()
                )));
            }
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(_) if out.exists() => {
            return Err(Failure::one(format!(
                "error: `{}` exists and is not a directory",
                out.display()
            )));
        }
        Err(e) => return Err(io_err("reading output directory", e)),
    }

    fs::create_dir_all(out).map_err(|e| io_err("creating output directory", e))?;
    let player_name = format!("weftora-player{}", std::env::consts::EXE_SUFFIX);
    fs::copy(&player, out.join(&player_name)).map_err(|e| io_err("copying player", e))?;

    let story_out = out.join("story");
    let mut copied = BTreeMap::new();
    copy_story(dir, &story_out, "", &mut copied)?;

    let exe = std::env::current_exe().map_err(|e| io_err("locating weftora", e))?;
    let manifest = checked.story.manifest();
    let report = serde_json::json!({
        "exportFormat": EXPORT_FORMAT,
        "cli": {
            "version": VERSION,
            "sha256": sha256_file(&exe).map_err(|e| io_err("hashing weftora", e))?,
        },
        "player": {
            "file": player_name,
            "sha256": sha256_file(&player).map_err(|e| io_err("hashing player", e))?,
        },
        "story": { "id": manifest.id, "storyVersion": manifest.story_version },
        "files": copied
            .iter()
            .map(|(path, hash)| serde_json::json!({ "path": path, "sha256": hash }))
            .collect::<Vec<_>>(),
    });
    let text = serde_json::to_string_pretty(&report).expect("export report serialises") + "\n";
    fs::write(out.join("weftora-export.json"), text)
        .map_err(|e| io_err("writing weftora-export.json", e))?;

    println!(
        "exported {} v{} to {} ({} story files)",
        manifest.id,
        manifest.story_version,
        out.display(),
        copied.len()
    );
    Ok(ExitCode::SUCCESS)
}

/// Copies `src` into `dst`, skipping dotfiles and refusing symlinks. `copied` maps Story-relative `/` paths to SHA-256.
fn copy_story(
    src: &Path,
    dst: &Path,
    prefix: &str,
    copied: &mut BTreeMap<String, String>,
) -> Result<(), Failure> {
    let io_err =
        |path: &Path, e: io::Error| Failure::one(format!("error: {}: {e}", path.display()));
    fs::create_dir_all(dst).map_err(|e| io_err(dst, e))?;
    let mut entries: Vec<_> = fs::read_dir(src)
        .map_err(|e| io_err(src, e))?
        .collect::<Result<_, _>>()
        .map_err(|e| io_err(src, e))?;
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let name = entry.file_name();
        let Some(name_str) = name.to_str() else {
            return Err(Failure::one(format!(
                "error: `{}` has a non-UTF-8 name",
                entry.path().display()
            )));
        };
        if name_str.starts_with('.') {
            continue;
        }
        let path = entry.path();
        let relative = format!("{prefix}{name_str}");
        let file_type = entry.file_type().map_err(|e| io_err(&path, e))?;
        if file_type.is_symlink() {
            return Err(Failure::one(format!(
                "error: `{relative}` is a symlink; symlinks are not exported"
            )));
        } else if file_type.is_dir() {
            copy_story(&path, &dst.join(&name), &format!("{relative}/"), copied)?;
        } else if file_type.is_file() {
            fs::copy(&path, dst.join(&name)).map_err(|e| io_err(&path, e))?;
            let hash = sha256_file(&path).map_err(|e| io_err(&path, e))?;
            copied.insert(relative, hash);
        }
    }
    Ok(())
}

fn sha256_file(path: &Path) -> io::Result<String> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    io::copy(&mut file, &mut hasher)?;
    Ok(hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect())
}

#[cfg(test)]
mod check_tests {
    use super::*;

    #[test]
    fn check_reports_initial_state_type_mismatch() {
        let story = tempfile::tempdir().unwrap();
        fs::write(
            story.path().join("manifest.jsonc"),
            r#"{
            "manifestSchema": 0,
            "id": "dev.weftora.type-mismatch",
            "storyVersion": "0.1.0",
            "title": "Type mismatch",
            "entrypoint": "Start",
            "dialogue": ["main.yarn"],
            "initialState": {"flag": "not-a-bool"}
        }"#,
        )
        .unwrap();
        fs::write(
            story.path().join("main.yarn"),
            "title: Start\n---\n<<declare $flag = true>>\n<<if $flag>>\nHello\n<<endif>>\n===\n",
        )
        .unwrap();
        let errors = check(story.path())
            .err()
            .expect("invalid initialState must fail check");
        assert_eq!(
            errors,
            ["manifest.jsonc: initialState `flag` is a string but Yarn declares $flag as bool"]
        );
    }
}
