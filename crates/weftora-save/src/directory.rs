//! Host-side save directory policy, independent of any renderer or Story loader.
use crate::Error;
use std::path::{Component, Path, PathBuf};

pub fn resolve(
    story_id: &str,
    story_root: &Path,
    override_directory: Option<&Path>,
) -> Result<PathBuf, Error> {
    if story_id.is_empty()
        || story_id.len() > 128
        || !story_id.as_bytes()[0].is_ascii_lowercase()
        || story_id.ends_with('.')
        || story_id.contains("..")
        || !story_id.bytes().all(|c| {
            c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, b'.' | b'-' | b'_')
        })
    {
        return Err(Error::Invalid(
            "Story ID is not a safe save-directory name".into(),
        ));
    }
    let first = story_id.split('.').next().unwrap_or_default();
    if matches!(first, "con" | "prn" | "aux" | "nul")
        || (first.len() == 4
            && (first.starts_with("com") || first.starts_with("lpt"))
            && matches!(first.as_bytes()[3], b'1'..=b'9'))
    {
        return Err(Error::Invalid("Story ID is a reserved Windows name".into()));
    }
    let chosen = if let Some(directory) = override_directory {
        directory.to_path_buf()
    } else {
        user_root()?.join("Weftora").join("saves").join(story_id)
    };
    if chosen.as_os_str().is_empty() {
        return Err(Error::Invalid("save directory must not be empty".into()));
    }
    if chosen
        .components()
        .any(|c| matches!(c, Component::ParentDir))
    {
        return Err(Error::Invalid(
            "save directory must not contain parent traversal".into(),
        ));
    }
    let absolute = if chosen.is_absolute() {
        chosen
    } else {
        std::env::current_dir()
            .map_err(|source| Error::Io {
                operation: "current directory",
                source,
            })?
            .join(chosen)
    };
    let directory = resolve_existing_parent(&absolute)?;
    let story = std::fs::canonicalize(story_root).map_err(|source| Error::Io {
        operation: "resolve Story directory",
        source,
    })?;
    if inside(&directory, &story) {
        return Err(Error::Invalid(
            "save directory must be outside the Story package".into(),
        ));
    }
    Ok(directory)
}

fn inside(path: &Path, parent: &Path) -> bool {
    #[cfg(windows)]
    {
        let path = PathBuf::from(path.to_string_lossy().to_lowercase());
        let parent = PathBuf::from(parent.to_string_lossy().to_lowercase());
        path.starts_with(parent)
    }
    #[cfg(not(windows))]
    {
        path.starts_with(parent)
    }
}

fn resolve_existing_parent(path: &Path) -> Result<PathBuf, Error> {
    let mut ancestor = path;
    let mut suffix = Vec::new();
    loop {
        match std::fs::canonicalize(ancestor) {
            Ok(mut resolved) => {
                for name in suffix.into_iter().rev() {
                    resolved.push(name);
                }
                return Ok(resolved);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                suffix.push(ancestor.file_name().ok_or_else(|| {
                    Error::Invalid("save directory has no existing ancestor".into())
                })?);
                ancestor = ancestor.parent().ok_or_else(|| {
                    Error::Invalid("save directory has no existing ancestor".into())
                })?;
            }
            Err(source) => {
                return Err(Error::Io {
                    operation: "resolve save directory",
                    source,
                });
            }
        }
    }
}

fn environment_path(name: &str) -> Result<PathBuf, Error> {
    let value = std::env::var_os(name)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| Error::Invalid(format!("{name} is not configured for save storage")))?;
    let path = PathBuf::from(value);
    if !path.is_absolute() {
        return Err(Error::Invalid(format!(
            "{name} must be an absolute save-data path"
        )));
    }
    Ok(path)
}
fn user_root() -> Result<PathBuf, Error> {
    #[cfg(windows)]
    {
        environment_path("LOCALAPPDATA")
    }
    #[cfg(target_os = "macos")]
    {
        Ok(environment_path("HOME")?
            .join("Library")
            .join("Application Support"))
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        if std::env::var_os("XDG_DATA_HOME").is_some_and(|value| !value.is_empty()) {
            environment_path("XDG_DATA_HOME")
        } else {
            Ok(environment_path("HOME")?.join(".local").join("share"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selected_directory_never_writes_inside_the_story_or_accepts_unsafe_ids() {
        let root = tempfile::tempdir().unwrap();
        let story = root.path().join("story");
        std::fs::create_dir(&story).unwrap();
        assert!(resolve("test.story", &story, Some(&story.join("saves"))).is_err());
        assert!(!story.join("saves").exists());
        let outside = root.path().join("data").join("slots");
        assert!(resolve("test.story", &story, Some(&outside)).is_ok());
        assert!(!outside.exists());
        for id in [
            "../story",
            "Con.foo",
            "con.foo",
            "test..story",
            "test/story",
            "test.story.",
            "com1.test",
        ] {
            assert!(resolve(id, &story, Some(&outside)).is_err(), "{id}");
        }
    }
    #[cfg(unix)]
    #[test]
    fn selected_symlink_alias_into_the_story_is_rejected() {
        let root = tempfile::tempdir().unwrap();
        let story = root.path().join("story");
        std::fs::create_dir(&story).unwrap();
        let alias = root.path().join("alias");
        std::os::unix::fs::symlink(&story, &alias).unwrap();
        assert!(resolve("test.story", &story, Some(&alias.join("saves"))).is_err());
        assert!(!story.join("saves").exists());
    }
}
