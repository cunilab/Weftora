use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn weftora() -> Command {
    Command::new(env!("CARGO_BIN_EXE_weftora"))
}

fn hello() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/hello-story")
}

fn broken() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/broken")
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

fn minimal_story(root: &Path, dialogue: &str) {
    fs::create_dir_all(root).unwrap();
    let yarn = root.join(dialogue);
    fs::create_dir_all(yarn.parent().unwrap()).unwrap();
    fs::write(yarn, "title: Start\n---\nHello.\n===\n").unwrap();
    let manifest = serde_json::json!({
        "manifestSchema": 0, "id": "test.story", "storyVersion": "1",
        "title": "Test", "entrypoint": "Start", "dialogue": [dialogue]
    });
    fs::write(root.join("manifest.jsonc"), manifest.to_string()).unwrap();
}

fn assert_overlap_rejected(story: &Path, out_dir: &Path, player: &Path) {
    let out = weftora()
        .env("WEFTORA_PLAYER", player)
        .arg("export")
        .arg(story)
        .arg("--out")
        .arg(out_dir)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(
        stderr(&out).contains("must not overlap")
            || stderr(&out).contains("output directory must not be a symlink or junction"),
        "{}",
        stderr(&out)
    );
}

#[test]
fn export_rejects_overlapping_directories_without_writes() {
    let tmp = tempfile::tempdir().unwrap();
    let story = tmp.path().join("story");
    minimal_story(&story, "intro.yarn");
    let player = tmp.path().join("fake-player");
    fs::write(&player, "player").unwrap();
    for out in [
        story.clone(),
        story.join("export"),
        story.join("missing/nested/export"),
        tmp.path().to_path_buf(),
        story.join("missing/../export"),
    ] {
        assert_overlap_rejected(&story, &out, &player);
    }
    assert_eq!(fs::read_dir(&story).unwrap().count(), 2);
    assert_eq!(fs::read_dir(tmp.path()).unwrap().count(), 2);
}

#[cfg(unix)]
#[test]
fn export_rejects_overlap_through_symlink_aliases() {
    let tmp = tempfile::tempdir().unwrap();
    let story = tmp.path().join("story");
    minimal_story(&story, "intro.yarn");
    let alias = tmp.path().join("alias");
    let parent_alias = tmp.path().join("parent-alias");
    std::os::unix::fs::symlink(&story, &alias).unwrap();
    std::os::unix::fs::symlink(tmp.path(), &parent_alias).unwrap();
    let player = tmp.path().join("fake-player");
    fs::write(&player, "player").unwrap();
    for out in [
        alias.clone(),
        alias.join("missing/nested/export"),
        parent_alias.clone(),
        parent_alias.join("story/export"),
        tmp.path().join("missing/../alias/export"),
    ] {
        assert_overlap_rejected(&story, &out, &player);
    }
    assert_overlap_rejected(&alias, &story.join("export"), &player);
    assert_eq!(fs::read_dir(&story).unwrap().count(), 2);
    assert_eq!(fs::read_dir(tmp.path()).unwrap().count(), 4);
}

#[test]
fn hidden_dialogue_is_rejected_by_check_and_export() {
    let tmp = tempfile::tempdir().unwrap();
    let story = tmp.path().join("story");
    minimal_story(&story, ".dialogue/intro.yarn");
    let player = tmp.path().join("fake-player");
    fs::write(&player, "player").unwrap();
    let export = tmp.path().join("export");
    for command in ["check", "export"] {
        let mut cmd = weftora();
        cmd.env("WEFTORA_PLAYER", &player).arg(command).arg(&story);
        if command == "export" {
            cmd.arg("--out").arg(&export);
        }
        let out = cmd.output().unwrap();
        assert_eq!(out.status.code(), Some(1));
        assert!(
            stderr(&out).contains("hidden files and folders are not allowed in Story references"),
            "{}",
            stderr(&out)
        );
    }
    assert!(!export.exists());
}

#[test]
fn export_skips_unreferenced_hidden_content_and_keeps_references() {
    let tmp = tempfile::tempdir().unwrap();
    let story = tmp.path().join("story");
    minimal_story(&story, "dialogue//intro.yarn");
    fs::write(story.join(".notes"), "private notes").unwrap();
    fs::create_dir(story.join(".cache")).unwrap();
    fs::write(story.join(".cache/file"), "cached content").unwrap();
    let player = tmp.path().join("fake-player");
    fs::write(&player, "player").unwrap();
    let export = tmp.path().join("export");
    let out = weftora()
        .env("WEFTORA_PLAYER", &player)
        .arg("export")
        .arg(&story)
        .arg("--out")
        .arg(&export)
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(!export.join("story/.notes").exists());
    assert!(!export.join("story/.cache").exists());
    assert!(export.join("story/dialogue/intro.yarn").is_file());
    let checked = weftora()
        .arg("check")
        .arg(export.join("story"))
        .output()
        .unwrap();
    assert!(checked.status.success(), "{}", stderr(&checked));
}

#[test]
fn version_prints_hash() {
    let out = weftora().arg("--version").output().unwrap();
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        text.starts_with(&format!("weftora {} (sha256 ", env!("CARGO_PKG_VERSION"))),
        "{text}"
    );
    let hex = text
        .split("sha256 ")
        .nth(1)
        .unwrap()
        .trim_end_matches([')', '\n']);
    assert_eq!(hex.len(), 64);
}

#[test]
fn check_ok_on_hello_story() {
    let out = weftora().arg("check").arg(hello()).output().unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.starts_with("OK: dev.weftora.hello-story"), "{text}");
}

#[test]
fn check_fails_on_broken_story() {
    let out = weftora().arg("check").arg(broken()).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    let err = stderr(&out);
    assert!(
        err.contains("dialogue/main.yarn:3: `background` uses undeclared asset ID `bg.missing`"),
        "{err}"
    );
    assert!(
        err.contains("dialogue/main.yarn:4: unknown command `teleport`"),
        "{err}"
    );
}

#[test]
fn check_fails_on_missing_story() {
    let out = weftora()
        .arg("check")
        .arg("/nonexistent/story")
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(stderr(&out).contains("manifest.jsonc"));
    assert!(stderr(&out).contains("/nonexistent/story"));
}

#[test]
fn missing_manifest_diagnostic_names_story_root() {
    let tmp = tempfile::tempdir().unwrap();
    let out = weftora().arg("check").arg(tmp.path()).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(stderr(&out).contains("manifest.jsonc"));
    assert!(stderr(&out).contains(&tmp.path().display().to_string()));
}

#[test]
fn check_rejects_case_mismatched_dialogue_on_case_insensitive_filesystems() {
    let tmp = tempfile::tempdir().unwrap();
    let story = tmp.path().join("story");
    minimal_story(&story, "dialogue/intro.yarn");
    if !story.join("Dialogue/Intro.yarn").is_file() {
        return;
    }
    let manifest = story.join("manifest.jsonc");
    let text = fs::read_to_string(&manifest).unwrap();
    fs::write(
        manifest,
        text.replace("dialogue/intro.yarn", "Dialogue/Intro.yarn"),
    )
    .unwrap();
    let out = weftora().arg("check").arg(&story).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    let error = stderr(&out);
    assert!(
        error.contains("`Dialogue/Intro.yarn` differs in case from `dialogue/intro.yarn` on disk"),
        "{error}"
    );
}

#[cfg(unix)]
#[test]
fn export_copy_failure_removes_partial_output() {
    let tmp = tempfile::tempdir().unwrap();
    let story = tmp.path().join("story");
    minimal_story(&story, "intro.yarn");
    // Sorts after the manifest and dialogue so copying fails after files were written.
    std::os::unix::fs::symlink(story.join("intro.yarn"), story.join("z-link")).unwrap();
    let checked = weftora().arg("check").arg(&story).output().unwrap();
    assert!(checked.status.success(), "{}", stderr(&checked));
    let player = tmp.path().join("fake-player");
    fs::write(&player, "player").unwrap();
    for (export, existing_empty_directory, new_root) in [
        (tmp.path().join("export"), false, tmp.path().join("export")),
        (
            tmp.path().join("existing"),
            true,
            tmp.path().join("existing"),
        ),
        (
            tmp.path().join("newa/newb/out2"),
            false,
            tmp.path().join("newa"),
        ),
    ] {
        if existing_empty_directory {
            fs::create_dir(&export).unwrap();
        }
        let out = weftora()
            .env("WEFTORA_PLAYER", &player)
            .arg("export")
            .arg(&story)
            .arg("--out")
            .arg(&export)
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(1));
        assert!(
            stderr(&out).contains("symlinks are not exported"),
            "{}",
            stderr(&out)
        );
        if existing_empty_directory {
            assert!(export.is_dir(), "pre-existing output directory was deleted");
            assert_eq!(fs::read_dir(&export).unwrap().count(), 0);
        } else {
            assert!(!new_root.exists(), "newly created export ancestors remain");
        }
    }
}

#[cfg(unix)]
#[test]
fn export_refuses_symlinked_output_without_touching_target() {
    let tmp = tempfile::tempdir().unwrap();
    let story = tmp.path().join("story");
    minimal_story(&story, "intro.yarn");
    let player = tmp.path().join("fake-player");
    fs::write(&player, "player").unwrap();
    let export = tmp.path().join("export");
    fs::create_dir(&export).unwrap();
    let alias = tmp.path().join("export-alias");
    std::os::unix::fs::symlink(&export, &alias).unwrap();
    for out_path in [alias.clone(), alias.join(".")] {
        let out = weftora()
            .env("WEFTORA_PLAYER", &player)
            .arg("export")
            .arg(&story)
            .arg("--out")
            .arg(out_path)
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(1));
        assert!(stderr(&out).contains("output directory must not be a symlink or junction"));
        assert!(export.is_dir(), "output alias target was deleted");
        assert_eq!(fs::read_dir(&export).unwrap().count(), 0);
        assert!(
            fs::symlink_metadata(&alias)
                .unwrap()
                .file_type()
                .is_symlink()
        );
    }
}

#[test]
fn export_copies_player_and_story() {
    let tmp = tempfile::tempdir().unwrap();
    let fake_player = tmp.path().join("fake-player");
    fs::write(&fake_player, b"not a real player").unwrap();
    let out_dir = tmp.path().join("missing/nested/export");

    let out = weftora()
        .env("WEFTORA_PLAYER", &fake_player)
        .arg("export")
        .arg(hello())
        .arg("--out")
        .arg(&out_dir)
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", stderr(&out));

    let player_name = format!("weftora-player{}", std::env::consts::EXE_SUFFIX);
    assert_eq!(
        fs::read(out_dir.join(player_name)).unwrap(),
        b"not a real player"
    );
    assert!(out_dir.join("story/manifest.jsonc").is_file());
    assert!(out_dir.join("story/dialogue/intro.yarn").is_file());
    assert!(out_dir.join("story/assets/room.png").is_file());

    let report: serde_json::Value =
        serde_json::from_slice(&fs::read(out_dir.join("weftora-export.json")).unwrap()).unwrap();
    assert_eq!(report["exportFormat"], 0);
    assert_eq!(report["story"]["id"], "dev.weftora.hello-story");
    assert_eq!(report["story"]["storyVersion"], "0.1.0");
    assert_eq!(report["cli"]["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(report["cli"]["sha256"].as_str().unwrap().len(), 64);
    assert_eq!(report["player"]["sha256"].as_str().unwrap().len(), 64);
    let paths: Vec<&str> = report["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["path"].as_str().unwrap())
        .collect();
    assert_eq!(
        paths,
        [
            "README.md",
            "assets/chime.wav",
            "assets/room.png",
            "dialogue/intro.yarn",
            "manifest.jsonc"
        ]
    );

    // A second export into the now non-empty directory is refused.
    let again = weftora()
        .env("WEFTORA_PLAYER", &fake_player)
        .arg("export")
        .arg(hello())
        .arg("--out")
        .arg(&out_dir)
        .output()
        .unwrap();
    assert_eq!(again.status.code(), Some(1));
    assert!(stderr(&again).contains("not empty"));
}

#[test]
fn run_without_player_reports_clear_error() {
    let out = weftora()
        .env("WEFTORA_PLAYER", "/nonexistent/player")
        .arg("run")
        .arg(hello())
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(stderr(&out).contains("player executable not found"));
}
