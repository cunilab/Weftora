use std::{env, path::Path, process::Command};

const HEADLESS: &[&str] = &["weftora-api", "weftora-core", "weftora-story"];
const FORBIDDEN: &[&str] = &[
    "bevy",
    "weftora-yarn",
    "weftora-yarn-compiler",
    "weftora-render-bevy",
    "weftora-player",
    "weftora-cli",
    "xtask",
];
const FORBIDDEN_PREFIXES: &[&str] = &["bevy_", "yarnspinner", "rhai", "mlua"];

mod deps;
mod docs;

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args: Vec<_> = env::args().skip(1).collect();
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or("xtask must be in the workspace root")?;
    match args.first().map(String::as_str) {
        Some("deps") if args[1..].iter().all(|arg| arg == "--offline") => {
            deps::run(root, args[1..].iter().any(|arg| arg == "--offline"))
        }
        Some("docs") if args.len() == 1 => docs::run(root),
        _ => Err("usage: cargo xtask deps [--offline] | cargo xtask docs".into()),
    }
}

fn command_output(command: &mut Command) -> Result<Vec<u8>, String> {
    let output = command
        .output()
        .map_err(|error| format!("{command:?}: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "{command:?} failed ({}):\n{}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(output.stdout)
}

/// Include new, nonignored documents as well as files already in the Git index.
fn markdown_files(root: &Path) -> Result<Vec<std::path::PathBuf>, String> {
    let output = Command::new("git")
        .current_dir(root)
        .args([
            "ls-files",
            "--cached",
            "--others",
            "--exclude-standard",
            "-z",
            "--",
            "*.md",
        ])
        .output();
    if let Ok(output) = output
        && output.status.success()
    {
        let mut paths = Vec::new();
        for name in output
            .stdout
            .split(|byte| *byte == 0)
            .filter(|name| !name.is_empty())
        {
            let name = std::str::from_utf8(name).map_err(|error| error.to_string())?;
            paths.push(std::path::PathBuf::from(name));
        }
        paths.sort();
        paths.dedup();
        return Ok(paths);
    }
    files(root)?
        .into_iter()
        .filter(|path| path.extension().is_some_and(|ext| ext == "md"))
        .map(|path| {
            path.strip_prefix(root)
                .map(Path::to_path_buf)
                .map_err(|error| error.to_string())
        })
        .collect()
}

/// Walk repository content without following directory symlinks or build/VCS output.
fn files(root: &Path) -> Result<Vec<std::path::PathBuf>, String> {
    let mut result = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(dir) = pending.pop() {
        for entry in
            std::fs::read_dir(&dir).map_err(|error| format!("{}: {error}", dir.display()))?
        {
            let entry = entry.map_err(|error| error.to_string())?;
            let kind = entry.file_type().map_err(|error| error.to_string())?;
            if kind.is_dir() {
                if entry.file_name() != ".git" && entry.file_name() != "target" {
                    pending.push(entry.path());
                }
            } else {
                result.push(entry.path());
            }
        }
    }
    result.sort();
    Ok(result)
}
