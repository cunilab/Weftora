//! Story package loading for Weftora.
//!
//! Must never depend on Bevy, Yarn Spinner, Rhai or any Story.

pub mod jsonc;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use weftora_api::{Diagnostic, Value};
use weftora_core::State;

/// The only manifest schema Phase 0 understands. `0` is explicitly unstable (ADR 0002).
pub const SUPPORTED_MANIFEST_SCHEMA: u32 = 0;

/// Manifest file name at the Story root (JSONC, ADR 0004).
pub const MANIFEST_FILE: &str = "manifest.jsonc";

/// Provisional Phase 0 manifest (`manifest.jsonc` at the Story root).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Manifest {
    pub manifest_schema: u32,
    /// Reverse-domain Story ID, e.g. `com.example.hello-story`.
    pub id: String,
    pub story_version: String,
    pub title: String,
    /// Yarn node to start from.
    pub entrypoint: String,
    /// Yarn source files, relative to the Story root.
    pub dialogue: Vec<String>,
    /// Asset ID → file path relative to the Story root.
    #[serde(default)]
    pub assets: BTreeMap<String, String>,
    /// State keys and their starting values.
    #[serde(default)]
    pub initial_state: BTreeMap<String, Value>,
}

/// A loaded, validated Story package.
#[derive(Debug, Clone)]
pub struct Story {
    root: PathBuf,
    manifest: Manifest,
}

/// One Yarn source file read from the Story.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DialogueSource {
    /// Story-relative path with `/` separators, used in diagnostics.
    pub file: String,
    pub contents: String,
}

impl Story {
    /// Loads and validates the Story at `root`. All problems are returned together; nothing is partially loaded.
    pub fn load(root: &Path) -> Result<Story, Vec<Diagnostic>> {
        let manifest_diag =
            |line: Option<u32>, msg: String| Diagnostic::new(MANIFEST_FILE, line, msg);
        let manifest_path = resolve_in_root(root, MANIFEST_FILE)
            .map_err(|d| vec![manifest_diag(None, d.message)])?;
        let text = match std::fs::read_to_string(manifest_path) {
            Ok(text) => text,
            Err(e) => {
                return Err(vec![manifest_diag(
                    None,
                    format!("cannot read {MANIFEST_FILE} in `{}`: {e}", root.display()),
                )]);
            }
        };
        let manifest: Manifest = match jsonc::from_str(&text) {
            Ok(m) => m,
            Err(e) => {
                let line = u32::try_from(e.line()).ok().filter(|l| *l > 0);
                return Err(vec![manifest_diag(line, e.to_string())]);
            }
        };

        let mut diags = Vec::new();
        if manifest.manifest_schema != SUPPORTED_MANIFEST_SCHEMA {
            diags.push(manifest_diag(
                None,
                format!(
                    "unsupported manifestSchema {} (this engine supports {SUPPORTED_MANIFEST_SCHEMA})",
                    manifest.manifest_schema
                ),
            ));
        }
        for (name, value) in [
            ("id", &manifest.id),
            ("title", &manifest.title),
            ("entrypoint", &manifest.entrypoint),
        ] {
            if value.trim().is_empty() {
                diags.push(manifest_diag(None, format!("`{name}` must not be empty")));
            }
        }
        if manifest.dialogue.is_empty() {
            diags.push(manifest_diag(
                None,
                "`dialogue` must list at least one Yarn file".into(),
            ));
        }

        let mut check_file = |what: String, relative: &str| match resolve_in_root(root, relative) {
            Ok(path) if path.is_file() => {}
            Ok(_) => diags.push(manifest_diag(
                None,
                format!("{what} `{relative}` is not a file"),
            )),
            Err(d) => diags.push(manifest_diag(
                None,
                format!("{what} `{relative}`: {}", d.message),
            )),
        };
        for path in &manifest.dialogue {
            check_file("dialogue".into(), path);
        }
        for (id, path) in &manifest.assets {
            check_file(format!("asset `{id}`"), path);
        }

        let mut scratch = State::new();
        for (key, value) in &manifest.initial_state {
            if let Err(e) = scratch.set(key, value.clone()) {
                diags.push(manifest_diag(None, format!("initialState `{key}`: {e}")));
            }
        }

        if diags.is_empty() {
            Ok(Story {
                root: root.to_path_buf(),
                manifest,
            })
        } else {
            Err(diags)
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    /// Resolves a Story-relative reference to a file on disk, enforcing the path policy.
    pub fn resolve(&self, relative: &str) -> Result<PathBuf, Diagnostic> {
        resolve_in_root(&self.root, relative)
    }

    /// Resolves an asset ID declared in the manifest to its file.
    pub fn asset_path(&self, asset_id: &str) -> Result<PathBuf, Diagnostic> {
        let relative = self.manifest.assets.get(asset_id).ok_or_else(|| {
            Diagnostic::new(
                MANIFEST_FILE,
                None,
                format!("unknown asset ID `{asset_id}`"),
            )
        })?;
        self.resolve(relative)
    }

    /// Reads every Yarn file listed in the manifest.
    pub fn dialogue_sources(&self) -> Result<Vec<DialogueSource>, Vec<Diagnostic>> {
        let mut sources = Vec::new();
        let mut diags = Vec::new();
        for file in &self.manifest.dialogue {
            let read = self
                .resolve(file)
                .map_err(|d| d.message)
                .and_then(|p| std::fs::read_to_string(p).map_err(|e| e.to_string()));
            match read {
                Ok(contents) => sources.push(DialogueSource {
                    file: file.clone(),
                    contents,
                }),
                Err(msg) => diags.push(Diagnostic::new(file.clone(), None, msg)),
            }
        }
        if diags.is_empty() {
            Ok(sources)
        } else {
            Err(diags)
        }
    }
}

/// Path policy (Phase 0 subset of architecture §4): `relative` must be a relative path with `/` separators,
/// no hidden or `..` segments, no drive/UNC/absolute form, and must resolve (after following symlinks) inside `root`.
pub fn resolve_in_root(root: &Path, relative: &str) -> Result<PathBuf, Diagnostic> {
    let bad = |msg: &str| Diagnostic::new(relative, None, msg);
    if relative.is_empty() {
        return Err(bad("path is empty"));
    }
    if relative.contains('\\') {
        return Err(bad("path must use `/` separators, not backslashes"));
    }
    if relative.contains(':') {
        return Err(bad("path must not contain `:` (drive letters, streams)"));
    }
    if relative.starts_with('/') {
        return Err(bad("path must be relative to the Story root, not absolute"));
    }
    if relative.split('/').any(|seg| seg == "..") {
        return Err(bad("path must not contain `..` segments"));
    }
    if relative.split('/').any(|seg| seg.starts_with('.')) {
        return Err(bad(
            "hidden files and folders are not allowed in Story references",
        ));
    }
    let root_canon = root
        .canonicalize()
        .map_err(|e| bad(&format!("cannot access Story root: {e}")))?;
    let target = root_canon
        .join(relative)
        .canonicalize()
        .map_err(|e| bad(&format!("file cannot be accessed: {e}")))?;
    if !target.starts_with(&root_canon) {
        return Err(bad("path resolves outside the Story root"));
    }
    Ok(target)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    const GOOD: &str = r#"{
        "manifestSchema": 0, "id": "a.b", "storyVersion": "1", "title": "T",
        "entrypoint": "Start", "dialogue": ["d/a.yarn"],
        "assets": {"bg": "a/bg.png"}, "initialState": {"x.y": 1}
    }"#;

    fn story_dir(manifest: &str) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("d")).unwrap();
        fs::create_dir_all(dir.path().join("a")).unwrap();
        fs::write(dir.path().join("d/a.yarn"), "title: Start\n---\n===\n").unwrap();
        fs::write(dir.path().join("a/bg.png"), "x").unwrap();
        fs::write(dir.path().join(MANIFEST_FILE), manifest).unwrap();
        dir
    }

    fn messages(dir: &tempfile::TempDir) -> Vec<String> {
        Story::load(dir.path())
            .unwrap_err()
            .iter()
            .map(|d| d.to_string())
            .collect()
    }

    #[test]
    fn loads_good_story() {
        let dir = story_dir(GOOD);
        let story = Story::load(dir.path()).unwrap();
        assert!(story.asset_path("bg").unwrap().ends_with("a/bg.png"));
        assert!(story.asset_path("nope").is_err());
        let src = story.dialogue_sources().unwrap();
        assert_eq!(src[0].file, "d/a.yarn");
    }

    #[test]
    fn json_error_has_line() {
        let dir = story_dir("{\n  \"manifestSchema\": 0,\n  oops\n}");
        let d = Story::load(dir.path()).unwrap_err();
        assert_eq!(d[0].file, MANIFEST_FILE);
        assert_eq!(d[0].line, Some(3));
    }

    #[test]
    fn collects_all_diagnostics() {
        let dir = story_dir(
            r#"{"manifestSchema": 1, "id": "", "storyVersion": "1", "title": "",
            "entrypoint": "", "dialogue": [], "assets": {"a": "missing.png", "b": "../x"},
            "initialState": {"Bad": 1}}"#,
        );
        let msgs = messages(&dir);
        assert_eq!(msgs.len(), 8, "{msgs:#?}");
        assert!(msgs.iter().all(|m| m.starts_with("manifest.jsonc: ")));
    }

    #[test]
    fn rejects_bad_paths() {
        let dir = story_dir(GOOD);
        for bad in [
            "",
            "/etc/passwd",
            "a\\b",
            "C:/x",
            "C:\\x",
            "\\\\srv\\s",
            "../x",
            "a/../b",
            "a/b:stream",
            "d/../../x",
        ] {
            assert!(resolve_in_root(dir.path(), bad).is_err(), "{bad:?}");
        }
        assert!(resolve_in_root(dir.path(), "d/a.yarn").is_ok());
        assert!(resolve_in_root(dir.path(), "d/missing.yarn").is_err());
    }

    #[cfg(unix)]
    #[test]
    fn rejects_manifest_symlink_escape() {
        let outside = tempfile::tempdir().unwrap();
        let external_manifest = outside.path().join(MANIFEST_FILE);
        fs::write(&external_manifest, GOOD).unwrap();
        let dir = story_dir(GOOD);
        fs::remove_file(dir.path().join(MANIFEST_FILE)).unwrap();
        std::os::unix::fs::symlink(external_manifest, dir.path().join(MANIFEST_FILE)).unwrap();
        let diagnostics = Story::load(dir.path()).unwrap_err();
        assert_eq!(diagnostics[0].file, MANIFEST_FILE);
        assert!(diagnostics[0].message.contains("outside the Story root"));
    }

    #[test]
    fn rejects_hidden_references() {
        for (original, hidden) in [
            ("d/a.yarn", ".dialogue/intro.yarn"),
            ("d/a.yarn", "d/.intro.yarn"),
            ("a/bg.png", ".assets/bg.png"),
            ("a/bg.png", "a/.bg.png"),
            ("d/a.yarn", "./d/a.yarn"),
        ] {
            let dir = story_dir(&GOOD.replace(original, hidden));
            let target = dir.path().join(hidden);
            fs::create_dir_all(target.parent().unwrap()).unwrap();
            fs::write(target, "content").unwrap();
            let msgs = messages(&dir);
            assert!(
                msgs.iter().any(|msg| msg
                    .contains("hidden files and folders are not allowed in Story references")),
                "{hidden}: {msgs:?}"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlink_escape() {
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("secret.txt"), "s").unwrap();
        let dir = story_dir(GOOD);
        std::os::unix::fs::symlink(outside.path(), dir.path().join("link")).unwrap();
        std::os::unix::fs::symlink(
            outside.path().join("secret.txt"),
            dir.path().join("file-link"),
        )
        .unwrap();
        let d = resolve_in_root(dir.path(), "link/secret.txt").unwrap_err();
        assert!(d.message.contains("outside"), "{d}");
        assert!(resolve_in_root(dir.path(), "file-link").is_err());
    }
}
