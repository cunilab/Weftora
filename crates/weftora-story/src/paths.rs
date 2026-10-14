//! Bounded inventory and portable Windows path rules for every package entry.
use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use weftora_api::Diagnostic;

pub const MAX_FILES: usize = 4096;
pub const MAX_FILE_BYTES: u64 = 16 * 1024 * 1024;
pub const MAX_TOTAL_BYTES: u64 = 256 * 1024 * 1024;

pub fn validate_relative(relative: &str) -> Result<(), String> {
    if relative.is_empty() || relative.starts_with('/') || relative.contains(['\\', ':']) {
        return Err("path must be relative with `/` separators; absolute, UNC, drive and stream paths are forbidden".into());
    }
    for segment in relative.split('/').filter(|s| !s.is_empty()) {
        if matches!(segment, "." | "..")
            || segment.ends_with(['.', ' '])
            || segment.contains(['<', '>', '"', '|', '?', '*'])
            || segment.chars().any(|c| c.is_control())
        {
            return Err(format!("invalid Windows path segment `{segment}`"));
        }
        let stem = segment
            .split('.')
            .next()
            .unwrap_or("")
            .trim_end_matches(' ')
            .to_uppercase();
        let numbered = stem
            .strip_prefix("COM")
            .or_else(|| stem.strip_prefix("LPT"));
        if matches!(
            stem.as_str(),
            "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
        ) || numbered.is_some_and(|s| {
            matches!(
                s,
                "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
            )
        }) {
            return Err(format!("Windows reserved name `{segment}`"));
        }
    }
    Ok(())
}

pub fn inventory(root: &Path) -> Result<(), Diagnostic> {
    inventory_with_limits(root, MAX_FILES, MAX_FILE_BYTES, MAX_TOTAL_BYTES)
}

fn inventory_with_limits(
    root: &Path,
    max_files: usize,
    max_file: u64,
    max_total: u64,
) -> Result<(), Diagnostic> {
    let bad = |file: &str, message: String| Diagnostic::new(file, None, message);
    let root = root
        .canonicalize()
        .map_err(|e| bad("", format!("cannot access Story root: {e}")))?;
    let mut pending = vec![root.clone()];
    let mut names = BTreeMap::new();
    let mut files = 0;
    let mut entries_seen = 0;
    let mut total = 0u64;
    while let Some(directory) = pending.pop() {
        let mut entries = Vec::new();
        for entry in std::fs::read_dir(&directory).map_err(|e| bad("", e.to_string()))? {
            entries_seen += 1;
            // Bound empty-directory fan-out as well as file count.
            if entries_seen > max_files * 2 {
                return Err(bad(
                    "",
                    format!("Story entry count exceeds {}", max_files * 2),
                ));
            }
            entries.push(entry.map_err(|e| bad("", e.to_string()))?);
        }
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            let path = entry.path();
            let relative = path.strip_prefix(&root).expect("inventory remains in root");
            let mut components = Vec::new();
            for component in relative.components() {
                components.push(
                    component
                        .as_os_str()
                        .to_str()
                        .ok_or_else(|| bad("", "Story paths must be Unicode".into()))?,
                );
            }
            let relative = components.join("/");
            validate_relative(&relative).map_err(|e| bad(&relative, e))?;
            record_name(&mut names, &relative)?;
            let metadata =
                std::fs::symlink_metadata(&path).map_err(|e| bad(&relative, e.to_string()))?;
            if super::is_symlink_or_reparse_point(&metadata) {
                return Err(bad(&relative, "path passes through a symlink; symlinks are not allowed in Stories (including Windows junctions)".into()));
            }
            if metadata.is_dir() {
                pending.push(path);
            } else if metadata.is_file() {
                files += 1;
                total += metadata.len();
                if files > max_files {
                    return Err(bad(
                        &relative,
                        format!("Story file count exceeds {max_files}"),
                    ));
                }
                if metadata.len() > max_file {
                    return Err(bad(
                        &relative,
                        format!("file size exceeds {max_file} bytes"),
                    ));
                }
                if total > max_total {
                    return Err(bad(
                        &relative,
                        format!("Story total size exceeds {max_total} bytes"),
                    ));
                }
            } else {
                return Err(bad(&relative, "unsupported Story file type".into()));
            }
        }
    }
    Ok(())
}

fn record_name(names: &mut BTreeMap<String, String>, relative: &str) -> Result<(), Diagnostic> {
    if let Some(previous) = names.insert(relative.to_uppercase(), relative.into()) {
        return Err(Diagnostic::new(
            relative,
            None,
            format!("case-insensitive path collision with `{previous}`"),
        ));
    }
    Ok(())
}

/// Enforce the limit again during the read, even if a file grew after inventory.
pub fn read_text(path: PathBuf) -> Result<String, String> {
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    file.take(MAX_FILE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_FILE_BYTES {
        return Err(format!("file size exceeds {MAX_FILE_BYTES} bytes"));
    }
    String::from_utf8(bytes).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn windows_names_are_rejected_on_every_host() {
        for path in [
            "C:/a",
            "C:a",
            "//server/share",
            "\\\\?\\C:\\a",
            "a/../b",
            "NUL.txt",
            "CON .txt",
            "con",
            "COM1.wav",
            "LPT³.txt",
            "a.",
            "a ",
            "a/b:stream",
            "a?b",
        ] {
            assert!(validate_relative(path).is_err(), "{path}");
        }
        for path in ["a/b", "COM0.txt", "COM10.txt", "café/背景.png"] {
            assert!(validate_relative(path).is_ok(), "{path}");
        }
    }
    #[test]
    fn inventory_limits_fail_before_loading() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a"), "1234").unwrap();
        std::fs::write(dir.path().join("b"), "12").unwrap();
        assert!(
            inventory_with_limits(dir.path(), 1, 10, 10)
                .unwrap_err()
                .message
                .contains("file count")
        );
        assert!(
            inventory_with_limits(dir.path(), 2, 3, 10)
                .unwrap_err()
                .message
                .contains("file size")
        );
        assert!(
            inventory_with_limits(dir.path(), 2, 10, 5)
                .unwrap_err()
                .message
                .contains("total size")
        );
        assert!(inventory_with_limits(dir.path(), 2, 4, 6).is_ok());
    }
    #[test]
    fn case_collision_policy_covers_ascii_and_unicode_on_every_host() {
        for (first, second) in [
            ("assets/Image.png", "assets/image.png"),
            ("data/Σ.jsonc", "data/ς.jsonc"),
        ] {
            let mut names = BTreeMap::new();
            record_name(&mut names, first).unwrap();
            let diagnostic = record_name(&mut names, second).unwrap_err();
            assert_eq!(diagnostic.file, second);
            assert!(diagnostic.message.contains(first));
        }
    }
    #[cfg(not(windows))]
    #[test]
    fn inventory_rejects_case_collisions_and_unused_unsafe_names() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a"), "").unwrap();
        std::fs::write(dir.path().join("A"), "").unwrap();
        // A case-insensitive volume cannot create this collision; the logical fixture below runs everywhere.
        if std::fs::read_dir(dir.path()).unwrap().count() == 2 {
            assert!(
                inventory(dir.path())
                    .unwrap_err()
                    .message
                    .contains("collision")
            );
        }
        std::fs::write(dir.path().join("NUL.txt"), "").unwrap();
        assert!(
            inventory(dir.path())
                .unwrap_err()
                .message
                .contains("reserved")
                || inventory(dir.path())
                    .unwrap_err()
                    .message
                    .contains("collision")
        );
    }
}
