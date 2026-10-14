//! The `weftora` command-line tool: check, run and export Story folders.

mod headless;

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
  weftora run <story-dir> --headless [--choices <id,id,...>]
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
        ["run", dir, "--headless"] => headless::run(Path::new(dir), &[]),
        ["run", dir, "--headless", "--choices", choices] => headless::parse_choices(choices)
            .and_then(|choices| headless::run(Path::new(dir), &choices)),
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
    match story.initial_state() {
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
    let registry = presentation_commands();
    for site in dialogue.command_sites() {
        if let Some(message) = command_problem(&story, &registry, &site.command) {
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

fn presentation_commands() -> weftora_core::command::CommandRegistry {
    use weftora_core::command::{CommandRegistry, ValueType};
    let mut registry = CommandRegistry::default();
    for name in ["background", "sound"] {
        registry
            .register(name, vec![ValueType::String], ValueType::Bool, |_| {
                Box::pin(async { Ok(weftora_api::Value::Bool(true)) })
            })
            .expect("unique built-in command IDs");
    }
    registry
}

fn command_problem(
    story: &Story,
    registry: &weftora_core::command::CommandRegistry,
    command: &weftora_api::StoryCommand,
) -> Option<String> {
    let arguments: Vec<_> = command
        .args
        .iter()
        .cloned()
        .map(weftora_api::Value::String)
        .collect();
    if let Err(error) = registry.validate(&command.name, &arguments) {
        return Some(error.to_string());
    }
    let id = &command.args[0]; // Registry validation guarantees one string argument for each built-in.
    if id.contains('{') {
        return Some(format!(
            "`{}` needs a literal asset ID, expressions are not supported",
            command.name
        ));
    }
    if !story.manifest().assets.contains_key(id) {
        return Some(format!(
            "`{}` uses undeclared asset ID `{id}`",
            command.name
        ));
    }
    None
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
    let requested_out: PathBuf = out.components().collect();
    match fs::symlink_metadata(&requested_out) {
        Ok(metadata) if is_symlink_or_reparse_point(&metadata) => {
            return Err(Failure::one(format!(
                "error: output directory must not be a symlink or junction: `{}`",
                out.display()
            )));
        }
        Ok(_) => {}
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => {
            return Err(Failure::one(format!(
                "error: accessing output directory: {e}"
            )));
        }
    }
    let story_root = dir
        .canonicalize()
        .map_err(|e| Failure::one(format!("error: accessing Story root: {e}")))?;
    let output_root = canonicalize_output(out)
        .map_err(|e| Failure::one(format!("error: resolving output directory: {e}")))?;
    if output_root.starts_with(&story_root) || story_root.starts_with(&output_root) {
        return Err(Failure::one(
            "error: output directory and Story root must not overlap (equal, inside, or containing the Story root)",
        ));
    }
    // Use the same resolved destination for writes and failure cleanup.
    let out = output_root.as_path();
    let checked = check(dir).map_err(Failure::Messages)?;
    let player = player_path()?;
    let io_err = |what: &str, e: io::Error| Failure::one(format!("error: {what}: {e}"));

    let out_preexisted = match fs::read_dir(out) {
        Ok(entries) => {
            if entries.count() > 0 {
                return Err(Failure::one(format!(
                    "error: output directory `{}` is not empty",
                    out.display()
                )));
            }
            true
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => false,
        Err(_) if out.exists() => {
            return Err(Failure::one(format!(
                "error: `{}` exists and is not a directory",
                out.display()
            )));
        }
        Err(e) => return Err(io_err("reading output directory", e)),
    };
    let mut missing_directories = Vec::new();
    if !out_preexisted {
        let mut missing = out;
        loop {
            match fs::symlink_metadata(missing) {
                Ok(_) => break,
                Err(e) if e.kind() == io::ErrorKind::NotFound => {
                    missing_directories.push(missing.to_path_buf());
                    missing = missing
                        .parent()
                        .expect("absolute output has an existing ancestor");
                }
                Err(e) => return Err(io_err("accessing output ancestor", e)),
            }
        }
    }
    let mut created_root = None;

    let mut result = (|| {
        for directory in missing_directories.iter().rev() {
            fs::create_dir(directory).map_err(|e| io_err("creating output directory", e))?;
            if created_root.is_none() {
                created_root = Some(directory.as_path());
            }
        }
        let player_name = format!("weftora-player{}", std::env::consts::EXE_SUFFIX);
        fs::copy(&player, out.join(&player_name)).map_err(|e| io_err("copying player", e))?;

        let story_out = out.join("story");
        let mut copied = BTreeMap::new();
        copy_story(dir, &story_out, "", &mut copied)?;
        verify_copied_references(&checked.story, &copied)?;

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
    })();
    if let Err(Failure::Messages(messages)) = &mut result {
        let cleanup = if out_preexisted {
            remove_export_contents(out)
        } else if let Some(created) = created_root {
            fs::remove_dir_all(created)
        } else {
            Ok(())
        };
        if let Err(error) = cleanup {
            messages.push(format!(
                "error: removing failed export `{}`: {error}",
                out.display()
            ));
        }
    }
    result
}

fn remove_export_contents(out: &Path) -> io::Result<()> {
    for entry in fs::read_dir(out)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            fs::remove_dir_all(entry.path())?;
        } else {
            fs::remove_file(entry.path())?;
        }
    }
    Ok(())
}

fn is_symlink_or_reparse_point(metadata: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_type().is_symlink() || metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}

/// Resolves existing ancestors through symlinks, retaining the nonexistent suffix.
fn canonicalize_output(path: &Path) -> io::Result<PathBuf> {
    let absolute = std::path::absolute(path)?;
    let mut ancestor = absolute.as_path();
    let mut suffix = Vec::new();
    let mut resolved = loop {
        match ancestor.canonicalize() {
            Ok(path) => break path,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                let component = ancestor.components().next_back().ok_or(e)?;
                suffix.push(component.as_os_str().to_owned());
                ancestor = ancestor.parent().ok_or_else(|| {
                    io::Error::new(io::ErrorKind::NotFound, "no existing output ancestor")
                })?;
            }
            Err(e) => return Err(e),
        }
    };
    for component in suffix.into_iter().rev() {
        if component == ".." {
            resolved.pop();
        } else if component != "." {
            resolved.push(component);
            // A `..` after a missing component can lead back to an existing symlink.
            match resolved.canonicalize() {
                Ok(path) => resolved = path,
                Err(e) if e.kind() == io::ErrorKind::NotFound => {}
                Err(e) => return Err(e),
            }
        }
    }
    Ok(resolved)
}

fn verify_copied_references(
    story: &Story,
    copied: &BTreeMap<String, String>,
) -> Result<(), Failure> {
    let manifest = story.manifest();
    for reference in std::iter::once(weftora_story::MANIFEST_FILE)
        .chain(manifest.dialogue.iter().map(String::as_str))
        .chain(manifest.assets.values().map(String::as_str))
        .chain(manifest.catalogs.values().flatten().map(String::as_str))
    {
        let relative = reference
            .split('/')
            .filter(|segment| !segment.is_empty())
            .collect::<Vec<_>>()
            .join("/");
        if !copied.contains_key(&relative) {
            return Err(Failure::one(format!(
                "error: export is incomplete: referenced file `{reference}` was not copied"
            )));
        }
    }
    Ok(())
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
        let metadata = fs::symlink_metadata(&path).map_err(|e| io_err(&path, e))?;
        let file_type = metadata.file_type();
        if is_symlink_or_reparse_point(&metadata) {
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
mod tests {
    use super::*;

    #[test]
    fn incomplete_export_fails_reference_verification() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/hello-story");
        let story = Story::load(&root).unwrap();
        let mut copied = BTreeMap::new();
        copy_story(
            &root,
            &tempfile::tempdir().unwrap().path().join("story"),
            "",
            &mut copied,
        )
        .unwrap_or_else(|_| panic!("fixture must copy successfully"));
        assert!(verify_copied_references(&story, &copied).is_ok());
        for reference in std::iter::once(weftora_story::MANIFEST_FILE)
            .chain(story.manifest().dialogue.iter().map(String::as_str))
            .chain(story.manifest().assets.values().map(String::as_str))
            .chain(
                story
                    .manifest()
                    .catalogs
                    .values()
                    .flatten()
                    .map(String::as_str),
            )
        {
            let hash = copied.remove(reference).unwrap();
            let Err(Failure::Messages(messages)) = verify_copied_references(&story, &copied) else {
                panic!("missing reference must fail export");
            };
            assert!(messages[0].contains(reference));
            assert!(messages[0].contains("was not copied"));
            copied.insert(reference.to_owned(), hash);
        }
    }
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

    #[test]
    fn check_reports_initial_state_integer_precision() {
        let story = tempfile::tempdir().unwrap();
        fs::write(
            story.path().join("manifest.jsonc"),
            r#"{
            "manifestSchema": 0, "id": "dev.weftora.precision",
            "storyVersion": "0.1.0", "title": "Precision",
            "entrypoint": "Start", "dialogue": ["main.yarn"],
            "initialState": {"world.day": -9223372036854775808}
        }"#,
        )
        .unwrap();
        fs::write(
            story.path().join("main.yarn"),
            "title: Start\n---\n<<declare $world__day = 0>>\nHello\n===\n",
        )
        .unwrap();
        let errors = check(story.path())
            .err()
            .expect("out-of-range int must fail check");
        assert_eq!(
            errors,
            [
                "manifest.jsonc: initialState `world.day` is an int (-9223372036854775808) outside ±16777216 and cannot be read exactly by Yarn $world__day"
            ]
        );
    }
}
