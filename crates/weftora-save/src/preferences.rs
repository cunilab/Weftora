//! Versioned per-Story preferences, separate from progress saves.
use crate::{Error, storage};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::Path};
use weftora_api::Value;

pub const MAX_BYTES: usize = 64 * 1024;
const MAX_KEYS: usize = 256;

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Profile {
    preferences_format: u32,
    story_id: String,
    values: BTreeMap<String, Value>,
}

fn validate(values: &BTreeMap<String, Value>) -> Result<(), Error> {
    if values.len() > MAX_KEYS {
        return Err(Error::TooLarge {
            resource: "preference keys",
            limit: MAX_KEYS,
        });
    }
    for (key, value) in values {
        if key.len() > 512 || value.payload_bytes() > 16 * 1024 {
            return Err(Error::Invalid(
                "preference key/value exceeds its size limit".into(),
            ));
        }
    }
    weftora_core::State::with_values(values.iter())?;
    Ok(())
}

/// The host resolves this directory outside the Story, and applies only supported Settings keys.
pub fn load(directory: &Path, story_id: &str) -> Result<BTreeMap<String, Value>, Error> {
    let bytes = storage::read_bounded(
        &directory.join("preferences.json"),
        MAX_BYTES,
        "preferences",
    )?;
    let text = std::str::from_utf8(&bytes).map_err(|e| Error::Corrupt(e.to_string()))?;
    let profile: Profile =
        weftora_core::json::from_str(text).map_err(|e| Error::Corrupt(e.to_string()))?;
    if profile.preferences_format != 1 {
        return Err(Error::Invalid(format!(
            "unsupported preferencesFormat {}; supported: 1",
            profile.preferences_format
        )));
    }
    if profile.story_id != story_id {
        return Err(Error::Invalid("preferences belong to another Story".into()));
    }
    validate(&profile.values)?;
    Ok(profile.values)
}

pub fn write(
    directory: &Path,
    story_id: &str,
    values: &BTreeMap<String, Value>,
) -> Result<(), Error> {
    write_with_backup(directory, story_id, values, "preferences.bak")
}

/// Explicit recovery preserves the replaced file under a unique name that later writes never rotate.
pub fn replace(directory: &Path, story_id: &str) -> Result<Option<std::path::PathBuf>, Error> {
    std::fs::create_dir_all(directory).map_err(|source| Error::Io {
        operation: "create preferences directory",
        source,
    })?;
    let backup = tempfile::Builder::new()
        .prefix("preferences.replaced-")
        .suffix(".json")
        .tempfile_in(directory)
        .map_err(|source| Error::Io {
            operation: "reserve replaced preferences",
            source,
        })?;
    let (file, path) = backup.keep().map_err(|e| Error::Io {
        operation: "retain replaced preferences",
        source: e.error,
    })?;
    drop(file);
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| Error::Invalid("invalid preference backup filename".into()))?;
    let result = write_with_backup(directory, story_id, &BTreeMap::new(), name);
    let empty = std::fs::metadata(&path).is_ok_and(|m| m.len() == 0);
    if empty {
        let _ = std::fs::remove_file(&path);
    }
    result?;
    Ok((!empty).then_some(path))
}

fn write_with_backup(
    directory: &Path,
    story_id: &str,
    values: &BTreeMap<String, Value>,
    backup: &str,
) -> Result<(), Error> {
    validate(values)?;
    let mut bytes = serde_json::to_vec(&Profile {
        preferences_format: 1,
        story_id: story_id.to_owned(),
        values: values.clone(),
    })
    .map_err(|e| Error::Invalid(e.to_string()))?;
    bytes.push(b'\n');
    if bytes.len() > MAX_BYTES {
        return Err(Error::TooLarge {
            resource: "preferences",
            limit: MAX_BYTES,
        });
    }
    storage::write_atomic(
        directory,
        "preferences.json",
        backup,
        "preferences.lock",
        &bytes,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn profile_roundtrip_backup_and_progress_are_independent() {
        let temp = tempfile::tempdir().unwrap();
        let directory = temp.path();
        std::fs::write(directory.join("save.json"), b"progress").unwrap();
        let mut values = BTreeMap::from([("settings.audio.master".into(), Value::Float(0.5))]);
        write(directory, "test.story", &values).unwrap();
        let first = std::fs::read(directory.join("preferences.json")).unwrap();
        assert_eq!(load(directory, "test.story").unwrap(), values);
        values.insert("settings.audio.master".into(), Value::Float(0.2));
        write(directory, "test.story", &values).unwrap();
        assert_eq!(
            std::fs::read(directory.join("preferences.bak")).unwrap(),
            first
        );
        assert_eq!(
            std::fs::read(directory.join("save.json")).unwrap(),
            b"progress"
        );
        assert!(load(directory, "other.story").is_err());
        assert!(!directory.join("save.bak").exists());
    }

    #[test]
    fn malformed_duplicate_newer_and_oversized_profiles_are_refused_without_writes() {
        let temp = tempfile::tempdir().unwrap();
        for bytes in [
            br#"{"preferencesFormat":2,"storyId":"test.story","values":{}}"#.to_vec(),
            br#"{"preferencesFormat":1,"storyId":"test.story","values":{"x":1,"x":2}}"#.to_vec(),
            br#"{"preferencesFormat":1,"storyId":"test.story","values":{},"extra":true}"#.to_vec(),
            vec![b' '; MAX_BYTES + 1],
            b"broken".to_vec(),
        ] {
            std::fs::write(temp.path().join("preferences.json"), &bytes).unwrap();
            assert!(load(temp.path(), "test.story").is_err());
            assert_eq!(
                std::fs::read(temp.path().join("preferences.json")).unwrap(),
                bytes
            );
            assert!(!temp.path().join("preferences.bak").exists());
        }
    }

    #[test]
    fn busy_profile_writer_preserves_both_files() {
        let temp = tempfile::tempdir().unwrap();
        let values = BTreeMap::new();
        write(temp.path(), "test.story", &values).unwrap();
        let before = std::fs::read(temp.path().join("preferences.json")).unwrap();
        let lock = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(temp.path().join("preferences.lock"))
            .unwrap();
        lock.lock().unwrap();
        assert!(matches!(
            write(temp.path(), "test.story", &values),
            Err(Error::Busy)
        ));
        assert_eq!(
            std::fs::read(temp.path().join("preferences.json")).unwrap(),
            before
        );
        assert!(!temp.path().join("preferences.bak").exists());
        lock.unlock().unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn profile_and_backup_symlinks_never_write_their_target() {
        let temp = tempfile::tempdir().unwrap();
        let outside = temp.path().join("outside");
        std::fs::write(&outside, b"keep").unwrap();
        for name in ["preferences.json", "preferences.bak", "preferences.lock"] {
            let directory = temp.path().join(name.replace('.', "-"));
            std::fs::create_dir(&directory).unwrap();
            std::os::unix::fs::symlink(&outside, directory.join(name)).unwrap();
            assert!(write(&directory, "test.story", &BTreeMap::new()).is_err());
            assert_eq!(std::fs::read(&outside).unwrap(), b"keep");
        }
    }
    #[test]
    fn reset_without_previous_file_or_with_busy_writer_leaves_no_empty_retained_files() {
        let temp = tempfile::tempdir().unwrap();
        assert_eq!(replace(temp.path(), "test.story").unwrap(), None);
        let lock = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(temp.path().join("preferences.lock"))
            .unwrap();
        lock.lock().unwrap();
        assert!(matches!(
            replace(temp.path(), "test.story"),
            Err(Error::Busy)
        ));
        assert!(!std::fs::read_dir(temp.path()).unwrap().any(|entry| {
            entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with("preferences.replaced-")
        }));
        lock.unlock().unwrap();
    }
}
