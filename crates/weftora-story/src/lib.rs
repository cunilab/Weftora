//! Story package loading for Weftora.
//!
//! Must never depend on Bevy, Yarn Spinner, Rhai or any Story.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use weftora_api::{Diagnostic, Value};

/// The only manifest schema Phase 0 understands. `0` is explicitly unstable (ADR 0002).
pub const SUPPORTED_MANIFEST_SCHEMA: u32 = 0;

/// Provisional Phase 0 manifest (`manifest.json` at the Story root).
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
        todo!("P0: {root:?}")
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
        todo!("P0: {asset_id}")
    }

    /// Reads every Yarn file listed in the manifest.
    pub fn dialogue_sources(&self) -> Result<Vec<DialogueSource>, Vec<Diagnostic>> {
        todo!("P0")
    }
}

/// Path policy (Phase 0 subset of architecture §4): `relative` must be a relative path with `/` separators,
/// no `..` segments, no drive/UNC/absolute form, and must resolve (after following symlinks) inside `root`.
pub fn resolve_in_root(root: &Path, relative: &str) -> Result<PathBuf, Diagnostic> {
    todo!("P0: {root:?} {relative}")
}
