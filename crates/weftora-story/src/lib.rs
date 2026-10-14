//! Story package loading for Weftora.
//!
//! Must never depend on Bevy, Yarn Spinner, Rhai or any Story.

pub mod compatibility;
pub mod jsonc;
pub mod paths;
pub mod ui;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use compatibility::{CAPABILITIES, ENGINE_API, Requirements};
use serde::Deserialize;
use weftora_api::{Diagnostic, Value};
use weftora_core::catalog::Catalog;
use weftora_core::{State, StateDefinition};

/// Current manifest schema; legacy Phase 0 schema 0 remains loadable.
pub const SUPPORTED_MANIFEST_SCHEMA: u32 = 1;

/// Manifest file name at the Story root (JSONC, ADR 0004).
pub const MANIFEST_FILE: &str = "manifest.jsonc";

/// Manifest v1, with legacy schema 0 compatibility (`manifest.jsonc` at the Story root).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Manifest {
    pub manifest_schema: u32,
    #[serde(default)]
    pub save_schema: Option<u32>,
    #[serde(default)]
    pub requires: Option<Requirements>,
    #[serde(default)]
    pub state: BTreeMap<String, StateDefinition>,
    #[serde(default)]
    pub catalogs: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    pub screens: Vec<String>,
    pub initial_screen: Option<String>,
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

/// Declared media type for catalog assets. Legacy manifest assets remain untyped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetKind {
    Image,
    Audio,
    Font,
}
impl AssetKind {
    pub fn category(self) -> &'static str {
        match self {
            Self::Image => "images",
            Self::Audio => "audio",
            Self::Font => "fonts",
        }
    }
    fn from_category(category: &str) -> Option<Self> {
        match category {
            "images" => Some(Self::Image),
            "audio" => Some(Self::Audio),
            "fonts" => Some(Self::Font),
            _ => None,
        }
    }
}
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct AssetEntry {
    id: String,
    path: String,
}
#[derive(Debug, Clone)]
struct Asset {
    kind: Option<AssetKind>,
    path: String,
}

/// A loaded, validated Story package.
#[derive(Debug, Clone)]
pub struct Story {
    root: PathBuf,
    manifest: Manifest,
    catalogs: BTreeMap<String, Catalog<serde_json::Value>>,
    assets: Catalog<Asset>,
    screens: Catalog<ui::ScreenDefinition>,
}

/// One Yarn source file read from the Story.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DialogueSource {
    /// Story-relative path with `/` separators, used in diagnostics.
    pub file: String,
    pub contents: String,
}

impl Story {
    /// Loads and validates the Story at `root`. Semantic problems are collected after parsing; nothing is partially loaded.
    pub fn load(root: &Path) -> Result<Story, Vec<Diagnostic>> {
        paths::inventory(root).map_err(|mut d| {
            if d.file.is_empty() {
                d.file = MANIFEST_FILE.into();
                d.message = format!("Story root `{}`: {}", root.display(), d.message);
            }
            vec![d]
        })?;
        let manifest_diag =
            |line: Option<u32>, msg: String| Diagnostic::new(MANIFEST_FILE, line, msg);
        let manifest_path = resolve_in_root(root, MANIFEST_FILE).map_err(|d| {
            vec![manifest_diag(
                None,
                format!(
                    "cannot read {MANIFEST_FILE} in `{}`: {}",
                    root.display(),
                    d.message
                ),
            )]
        })?;
        let text = match paths::read_text(manifest_path) {
            Ok(text) => text,
            Err(e) => {
                return Err(vec![manifest_diag(
                    None,
                    format!("cannot read {MANIFEST_FILE} in `{}`: {e}", root.display()),
                )]);
            }
        };
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Header {
            manifest_schema: u32,
        }
        let header: Header = jsonc::from_str(&text).map_err(|error| {
            vec![manifest_diag(
                u32::try_from(error.line()).ok().filter(|line| *line > 0),
                error.to_string(),
            )]
        })?;
        if header.manifest_schema > SUPPORTED_MANIFEST_SCHEMA {
            return Err(vec![manifest_diag(
                None,
                format!(
                    "unsupported manifestSchema {} (this engine supports {SUPPORTED_MANIFEST_SCHEMA})",
                    header.manifest_schema
                ),
            )]);
        }
        let manifest: Manifest = match jsonc::from_str(&text) {
            Ok(m) => m,
            Err(e) => {
                let line = u32::try_from(e.line()).ok().filter(|l| *l > 0);
                return Err(vec![manifest_diag(line, e.to_string())]);
            }
        };

        let mut diags = Vec::new();
        if manifest.manifest_schema == 1 {
            match &manifest.requires {
                Some(requirements) => {
                    if let Err(errors) =
                        compatibility::check(requirements, ENGINE_API, CAPABILITIES)
                    {
                        diags.extend(errors.into_iter().map(|e| manifest_diag(None, e)));
                    }
                }
                None => diags.push(manifest_diag(
                    None,
                    "manifest v1 requires `requires.engineApi`".into(),
                )),
            }
            if let Err(error) = semver::Version::parse(&manifest.story_version) {
                diags.push(manifest_diag(
                    None,
                    format!("invalid storyVersion `{}`: {error}", manifest.story_version),
                ));
            }
        } else if manifest.requires.is_some()
            || !manifest.state.is_empty()
            || !manifest.catalogs.is_empty()
            || !manifest.screens.is_empty()
            || manifest.initial_screen.is_some()
            || manifest.save_schema.is_some()
        {
            diags.push(manifest_diag(
                None,
                "requires, state, catalogs and screens need manifestSchema 1".into(),
            ));
        }
        if manifest
            .catalogs
            .keys()
            .any(|category| AssetKind::from_category(category).is_some())
            && !manifest
                .requires
                .as_ref()
                .is_some_and(|r| r.capabilities.iter().any(|c| c == "assets.catalog/1"))
        {
            diags.push(manifest_diag(
                None,
                "typed asset catalogs require `assets.catalog/1` in requires.capabilities".into(),
            ));
        }
        if (!manifest.screens.is_empty() || manifest.initial_screen.is_some())
            && !manifest
                .requires
                .as_ref()
                .is_some_and(|r| r.capabilities.iter().any(|c| c == "ui.screens/1"))
        {
            diags.push(manifest_diag(
                None,
                "declarative screens require `ui.screens/1` in requires.capabilities".into(),
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
        let mut dialogue_paths = std::collections::BTreeSet::new();
        for path in &manifest.dialogue {
            check_file("dialogue".into(), path);
        }

        for (id, path) in &manifest.assets {
            check_file(format!("asset `{id}`"), path);
        }

        for (index, path) in manifest.dialogue.iter().enumerate() {
            if !dialogue_paths.insert(path) {
                diags.push(manifest_diag(
                    None,
                    format!("dialogue[{index}]: duplicate source `{path}`"),
                ));
            }
        }
        let mut scratch = State::new();
        for (key, definition) in &manifest.state {
            if let Err(error) = scratch.declare(key, definition.clone()) {
                diags.push(manifest_diag(None, format!("state `{key}`: {error}")));
            }
        }
        for (key, value) in &manifest.initial_state {
            if let Err(e) = scratch.set(key, value.clone()) {
                diags.push(manifest_diag(None, format!("initialState `{key}`: {e}")));
            }
        }

        if let Some(schema) = manifest.save_schema {
            if schema == 0 {
                diags.push(manifest_diag(None, "saveSchema must be positive".into()));
            }
            if !manifest
                .requires
                .as_ref()
                .is_some_and(|r| r.capabilities.iter().any(|c| c == "save/1"))
            {
                diags.push(manifest_diag(None, "saveSchema requires save/1".into()));
            }
        }

        let mut assets = Catalog::default();
        for (id, path) in &manifest.assets {
            if let Err(error) = assets.insert(
                id,
                Asset {
                    kind: None,
                    path: path.clone(),
                },
                MANIFEST_FILE,
                &format!("/assets/{id}"),
            ) {
                diags.push(error);
            }
        }
        let mut catalogs = BTreeMap::new();
        for (category, files) in &manifest.catalogs {
            let kind = AssetKind::from_category(category);
            if category != "data" && kind.is_none() {
                diags.push(manifest_diag(
                    None,
                    format!("unsupported catalog category `{category}` (supports data, images, audio, fonts)"),
                ));
                continue;
            }
            let mut catalog = Catalog::default();
            let mut sources = std::collections::BTreeSet::new();
            for (index, file) in files.iter().enumerate() {
                if !sources.insert(file) {
                    diags.push(manifest_diag(
                        None,
                        format!("/catalogs/{category}/{index}: duplicate catalog source `{file}`"),
                    ));
                    continue;
                }
                let text = match resolve_in_root(root, file)
                    .map_err(|d| d.message)
                    .and_then(paths::read_text)
                {
                    Ok(text) => text,
                    Err(error) => {
                        diags.push(Diagnostic::new(file, None, error));
                        continue;
                    }
                };
                let entries: Vec<serde_json::Value> = match jsonc::from_str(&text) {
                    Ok(entries) => entries,
                    Err(error) => {
                        diags.push(Diagnostic::new(
                            file,
                            u32::try_from(error.line()).ok().filter(|line| *line > 0),
                            error.to_string(),
                        ));
                        continue;
                    }
                };
                for (index, entry) in entries.into_iter().enumerate() {
                    let location = format!("/{index}/id");
                    let Some(id) = entry
                        .get("id")
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_owned)
                    else {
                        diags.push(Diagnostic::new(
                            file,
                            None,
                            format!("{location}: missing string catalog ID"),
                        ));
                        continue;
                    };
                    if let Some(kind) = kind {
                        let asset: AssetEntry = match serde_json::from_value(entry.clone()) {
                            Ok(asset) => asset,
                            Err(error) => {
                                diags.push(Diagnostic::new(
                                    file,
                                    None,
                                    format!("/{index}: {error}"),
                                ));
                                continue;
                            }
                        };
                        match resolve_in_root(root, &asset.path) {
                            Ok(path) if path.is_file() => {}
                            Ok(_) => {
                                diags.push(Diagnostic::new(
                                    file,
                                    None,
                                    format!("/{index}/path: `{}` is not a file", asset.path),
                                ));
                                continue;
                            }
                            Err(error) => {
                                diags.push(Diagnostic::new(
                                    file,
                                    None,
                                    format!("/{index}/path: `{}`: {}", asset.path, error.message),
                                ));
                                continue;
                            }
                        }
                        if let Err(error) = assets.insert(
                            &asset.id,
                            Asset {
                                kind: Some(kind),
                                path: asset.path,
                            },
                            file,
                            &location,
                        ) {
                            diags.push(error);
                            continue;
                        }
                    }
                    if let Err(error) = catalog.insert(&id, entry, file, &location) {
                        diags.push(error);
                    }
                }
            }
            catalogs.insert(category.clone(), catalog);
        }
        let screens = match ui::load(root, &manifest) {
            Ok(screens) => screens,
            Err(errors) => {
                diags.extend(errors);
                Catalog::default()
            }
        };
        if !diags.is_empty() {
            return Err(diags);
        }
        let story = Story {
            root: root.to_path_buf(),
            manifest,
            catalogs,
            assets,
            screens,
        };
        let errors = ui::reference_errors(&story);
        if errors.is_empty() {
            Ok(story)
        } else {
            Err(errors)
        }
    }

    pub fn screen(&self, id: &str) -> Result<&ui::ScreenDefinition, Diagnostic> {
        self.screens.get(id, MANIFEST_FILE, "screen reference")
    }
    pub fn screens(&self) -> impl Iterator<Item = (&str, &ui::ScreenDefinition)> {
        self.screens.iter()
    }
    pub fn asset_kind(&self, id: &str) -> Result<Option<AssetKind>, Diagnostic> {
        self.assets
            .get(id, MANIFEST_FILE, "asset reference")
            .map(|a| a.kind)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    /// Defaults and initial overrides are validated before the Story is returned.
    pub fn initial_state(&self) -> Result<State, weftora_api::StateError> {
        let mut state = State::with_definitions(&self.manifest.state)?;
        for (key, value) in &self.manifest.initial_state {
            state.set(key, value.clone())?;
        }
        state.drain_changes();
        Ok(state)
    }

    pub fn catalog(&self, category: &str) -> Result<&Catalog<serde_json::Value>, Diagnostic> {
        self.catalogs.get(category).ok_or_else(|| {
            Diagnostic::new(
                MANIFEST_FILE,
                None,
                format!("/catalogs/{category}: missing catalog `{category}`"),
            )
        })
    }

    /// Resolves a Story-relative reference to a file on disk, enforcing the path policy.
    pub fn resolve(&self, relative: &str) -> Result<PathBuf, Diagnostic> {
        resolve_in_root(&self.root, relative)
    }

    /// Resolves any legacy or catalog asset ID through the Story path policy.
    pub fn asset_path(&self, asset_id: &str) -> Result<PathBuf, Diagnostic> {
        let asset = self
            .assets
            .get(asset_id, MANIFEST_FILE, "asset reference")
            .map_err(|_| {
                Diagnostic::new(
                    MANIFEST_FILE,
                    None,
                    format!("unknown asset ID `{asset_id}`"),
                )
            })?;
        self.resolve(&asset.path)
    }

    /// Enforces declared catalog types. Legacy entries accept any type; the adapter must decode them.
    pub fn asset_path_of(&self, kind: AssetKind, asset_id: &str) -> Result<PathBuf, Diagnostic> {
        let asset = self.assets.get(
            asset_id,
            MANIFEST_FILE,
            &format!("{} reference", kind.category()),
        )?;
        if let Some(actual) = asset.kind
            && actual != kind
        {
            return Err(Diagnostic::new(
                MANIFEST_FILE,
                None,
                format!(
                    "asset `{asset_id}` belongs to {}, expected {}",
                    actual.category(),
                    kind.category()
                ),
            ));
        }
        self.resolve(&asset.path)
    }

    pub fn asset_count(&self) -> usize {
        self.assets.iter().count()
    }

    /// Includes typed catalog media paths, not just the legacy manifest map.
    pub fn asset_references(&self) -> impl Iterator<Item = (&str, &str)> {
        self.assets
            .iter()
            .map(|(id, asset)| (id, asset.path.as_str()))
    }

    /// Reads every Yarn file listed in the manifest.
    pub fn dialogue_sources(&self) -> Result<Vec<DialogueSource>, Vec<Diagnostic>> {
        let mut sources = Vec::new();
        let mut diags = Vec::new();
        for file in &self.manifest.dialogue {
            let read = self
                .resolve(file)
                .map_err(|d| d.message)
                .and_then(paths::read_text);
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
/// no hidden, `.` or `..` segments, no drive/UNC/absolute form, and no symlink or junction segments inside `root`.
/// References must match the canonical on-disk spelling, including case.
pub fn resolve_in_root(root: &Path, relative: &str) -> Result<PathBuf, Diagnostic> {
    let bad = |msg: &str| Diagnostic::new(relative, None, msg);
    paths::validate_relative(relative).map_err(|e| bad(&e))?;
    if relative.split('/').any(|seg| seg.starts_with('.')) {
        return Err(bad(
            "hidden files and folders are not allowed in Story references",
        ));
    }
    let root_canon = root
        .canonicalize()
        .map_err(|e| bad(&format!("cannot access Story root: {e}")))?;
    let mut segment_path = root_canon.clone();
    for segment in Path::new(relative).components() {
        segment_path.push(segment);
        let metadata = std::fs::symlink_metadata(&segment_path)
            .map_err(|e| bad(&format!("file cannot be accessed: {e}")))?;
        if is_symlink_or_reparse_point(&metadata) {
            return Err(bad(&format!(
                "`{relative}` passes through a symlink at `{}`; symlinks are not allowed in Stories",
                segment_path.display()
            )));
        }
    }
    let target = root_canon
        .join(relative)
        .canonicalize()
        .map_err(|e| bad(&format!("file cannot be accessed: {e}")))?;
    if !target.starts_with(&root_canon) {
        return Err(bad("path resolves outside the Story root"));
    }
    let on_disk = target.strip_prefix(&root_canon).expect("contained path");
    if !on_disk.components().eq(Path::new(relative).components()) {
        let spelling = on_disk
            .components()
            .map(|component| component.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/");
        let requested: Vec<_> = relative.split('/').filter(|s| !s.is_empty()).collect();
        let actual: Vec<_> = spelling.split('/').collect();
        if requested.len() == actual.len()
            && requested
                .iter()
                .zip(&actual)
                .all(|(a, b)| a.to_lowercase() == b.to_lowercase())
        {
            return Err(bad(&format!(
                "`{relative}` differs in case from `{spelling}` on disk"
            )));
        }
        return Err(bad(&format!(
            "`{relative}` resolves to a different on-disk path `{spelling}`"
        )));
    }
    Ok(target)
}

fn is_symlink_or_reparse_point(metadata: &std::fs::Metadata) -> bool {
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
            r#"{"manifestSchema": 0, "id": "", "storyVersion": "1", "title": "",
            "entrypoint": "", "dialogue": [], "assets": {"a": "missing.png", "b": "../x"},
            "initialState": {"Bad": 1}}"#,
        );
        let msgs = messages(&dir);
        assert_eq!(msgs.len(), 7, "{msgs:#?}");
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
        assert!(
            diagnostics[0]
                .message
                .contains("symlinks are not allowed in Stories")
        );
    }

    #[test]
    fn rejects_hidden_references() {
        for (original, hidden) in [
            ("d/a.yarn", ".dialogue/intro.yarn"),
            ("d/a.yarn", "d/.intro.yarn"),
            ("a/bg.png", ".assets/bg.png"),
            ("a/bg.png", "a/.bg.png"),
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

    #[test]
    fn rejects_dot_segments_with_traversal_diagnostic() {
        let dir = story_dir(GOOD);
        for reference in ["./d/a.yarn", "d/./a.yarn", "d/../d/a.yarn"] {
            let diagnostic = resolve_in_root(dir.path(), reference).unwrap_err();
            assert_eq!(
                diagnostic.message,
                "path must not contain `.` or `..` segments"
            );
        }
    }

    #[test]
    fn rejects_case_mismatched_references_on_case_insensitive_filesystems() {
        let dir = story_dir(GOOD);
        if !dir.path().join("D/A.YARN").is_file() {
            return; // The temporary directory is on a case-sensitive filesystem.
        }
        for reference in ["D/a.yarn", "d/A.YARN", "D/A.YARN"] {
            let diagnostic = resolve_in_root(dir.path(), reference).unwrap_err();
            assert!(diagnostic.message.contains(reference), "{diagnostic}");
            assert!(
                diagnostic.message.contains("`d/a.yarn` on disk"),
                "{diagnostic}"
            );
        }
        fs::write(
            dir.path().join(MANIFEST_FILE),
            GOOD.replace("d/a.yarn", "D/A.YARN"),
        )
        .unwrap();
        assert!(
            messages(&dir)
                .iter()
                .any(|message| message.contains("differs in case"))
        );
    }

    #[cfg(unix)]
    #[test]
    fn rejects_internal_file_symlink() {
        let dir = story_dir(GOOD);
        fs::rename(dir.path().join("d/a.yarn"), dir.path().join("d/real.yarn")).unwrap();
        std::os::unix::fs::symlink("real.yarn", dir.path().join("d/a.yarn")).unwrap();
        let diagnostic = resolve_in_root(dir.path(), "d/a.yarn").unwrap_err();
        assert!(
            diagnostic
                .message
                .contains("`d/a.yarn` passes through a symlink")
        );
        assert!(diagnostic.message.contains("/d/a.yarn`"));
        assert!(!diagnostic.message.contains("differs in case"));
        assert!(Story::load(dir.path()).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn rejects_internal_directory_symlink() {
        let dir = story_dir(GOOD);
        fs::rename(dir.path().join("d"), dir.path().join("real")).unwrap();
        std::os::unix::fs::symlink("real", dir.path().join("d")).unwrap();
        let diagnostic = resolve_in_root(dir.path(), "d/a.yarn").unwrap_err();
        assert!(diagnostic.message.contains("passes through a symlink"));
        assert!(diagnostic.message.contains("/d`"));
        assert!(!diagnostic.message.contains("differs in case"));
        assert!(Story::load(dir.path()).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn rejects_internal_manifest_symlink() {
        let dir = story_dir(GOOD);
        fs::rename(
            dir.path().join(MANIFEST_FILE),
            dir.path().join("real.jsonc"),
        )
        .unwrap();
        std::os::unix::fs::symlink("real.jsonc", dir.path().join(MANIFEST_FILE)).unwrap();
        let diagnostics = Story::load(dir.path()).unwrap_err();
        assert!(diagnostics[0].message.contains("passes through a symlink"));
        assert!(!diagnostics[0].message.contains("differs in case"));
    }

    #[cfg(unix)]
    #[test]
    fn permits_symlinked_story_root() {
        let dir = story_dir(GOOD);
        let aliases = tempfile::tempdir().unwrap();
        let alias = aliases.path().join("story");
        std::os::unix::fs::symlink(dir.path(), &alias).unwrap();
        let story = Story::load(&alias).unwrap();
        assert_eq!(story.dialogue_sources().unwrap()[0].file, "d/a.yarn");
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
        assert!(d.message.contains("passes through a symlink"), "{d}");
        assert!(resolve_in_root(dir.path(), "file-link").is_err());
    }
}
