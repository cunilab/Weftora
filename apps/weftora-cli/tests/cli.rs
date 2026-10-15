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
    minimal_story(&story, "dialogue/intro.yarn");
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
        err.contains(
            "dialogue/main.yarn:3: `background`: images reference: missing catalog ID `bg.missing`"
        ),
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
fn export_rejects_unused_symlink_before_creating_output() {
    let tmp = tempfile::tempdir().unwrap();
    let story = tmp.path().join("story");
    minimal_story(&story, "intro.yarn");
    // Whole-package inventory rejects even unreferenced links before any export writes.
    std::os::unix::fs::symlink(story.join("intro.yarn"), story.join("z-link")).unwrap();
    let checked = weftora().arg("check").arg(&story).output().unwrap();
    assert_eq!(checked.status.code(), Some(1));
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
            stderr(&out).contains("symlinks are not allowed in Stories"),
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
    assert_eq!(report["exportFormat"], 1);
    let artifact = out_dir.join(weftora_yarn::ARTIFACT_FILE);
    let bytes = fs::read(&artifact).unwrap();
    let mut compiled = weftora_yarn::CompiledDialogue::from_artifact_bytes(&bytes).unwrap();
    let exported_story = weftora_story::Story::load(&out_dir.join("story")).unwrap();
    compiled
        .verify_sources(&exported_story.dialogue_sources().unwrap())
        .unwrap();
    assert_eq!(report["dialogue"]["file"], weftora_yarn::ARTIFACT_FILE);
    assert_eq!(report["dialogue"]["dialogueArtifactFormat"], 1);
    assert_eq!(
        report["dialogue"]["compiler"]["version"],
        weftora_yarn::COMPILER_VERSION
    );
    assert_eq!(report["dialogue"]["inputHash"], compiled.input_hash());
    use sha2::Digest;
    assert_eq!(
        report["dialogue"]["sha256"],
        format!("{:x}", sha2::Sha256::digest(&bytes))
    );
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
            "assets/audio.jsonc",
            "assets/chime.wav",
            "assets/images.jsonc",
            "assets/room.png",
            "dialogue/intro.yarn",
            "manifest.jsonc",
            "ui/details.jsonc",
            "ui/dialogue.jsonc"
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

#[test]
fn headless_golden_traces_match_in_ten_fresh_runs_per_branch() {
    for (choice, golden) in [
        (
            "0",
            include_bytes!("fixtures/headless-yes.jsonl").as_slice(),
        ),
        ("1", include_bytes!("fixtures/headless-no.jsonl").as_slice()),
    ] {
        for _ in 0..10 {
            let out = weftora()
                .env("WEFTORA_PLAYER", "missing-player")
                .args(["run"])
                .arg(hello())
                .args(["--headless", "--choices", choice])
                .output()
                .unwrap();
            assert!(out.status.success(), "{}", stderr(&out));
            assert_eq!(out.stdout, golden);
        }
    }
}

#[test]
fn check_and_headless_run_reject_the_same_invalid_package() {
    let tmp = tempfile::tempdir().unwrap();
    minimal_story(tmp.path(), "main.yarn");
    let text = fs::read_to_string(tmp.path().join("manifest.jsonc")).unwrap();
    fs::write(
        tmp.path().join("manifest.jsonc"),
        text.replace("main.yarn", "C:/outside.yarn"),
    )
    .unwrap();
    let check = weftora().arg("check").arg(tmp.path()).output().unwrap();
    let run = weftora()
        .arg("run")
        .arg(tmp.path())
        .arg("--headless")
        .output()
        .unwrap();
    assert_eq!(check.status.code(), Some(1));
    assert_eq!(run.status.code(), Some(1));
    assert_eq!(check.stderr, run.stderr);
    assert!(run.stdout.is_empty());
}

#[test]
fn headless_choices_are_required_valid_and_exhausted() {
    let out = weftora()
        .arg("run")
        .arg(hello())
        .arg("--headless")
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(stderr(&out).contains("requires --choices"));
    for choices in ["999", "0,1", "oops", "-1", ""] {
        let out = weftora()
            .arg("run")
            .arg(hello())
            .args(["--headless", "--choices", choices])
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(1), "{choices}: {}", stderr(&out));
    }
}

#[test]
fn real_startup_failure_emits_no_gameplay_trace() {
    let dir = tempfile::tempdir().unwrap();
    minimal_story(dir.path(), "main.yarn");
    fs::write(
        dir.path().join("main.yarn"),
        "title: Other\n---\nHello.\n===\n",
    )
    .unwrap();
    let output = weftora()
        .arg("run")
        .arg(dir.path())
        .arg("--headless")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(stderr(&output).contains("entrypoint `Start`"));
}

#[test]
fn scoped_replay_defaults_notifications_and_signals_match_ten_fresh_runs() {
    let story = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/scoped-story");
    let expected = include_bytes!("fixtures/scoped-replay.jsonl");
    for _ in 0..10 {
        let output = weftora()
            .arg("run")
            .arg(&story)
            .arg("--headless")
            .output()
            .unwrap();
        assert!(output.status.success(), "{}", stderr(&output));
        assert_eq!(output.stdout, expected);
    }
}

#[test]
fn check_and_run_reject_invalid_yarn_defaults_before_startup() {
    let dir = tempfile::tempdir().unwrap();
    minimal_story(dir.path(), "main.yarn");
    fs::write(
        dir.path().join("main.yarn"),
        "title: Start\n---\n<<declare $Bad = true>>\nHello.\n===\n",
    )
    .unwrap();
    let check = weftora().arg("check").arg(dir.path()).output().unwrap();
    let run = weftora()
        .arg("run")
        .arg(dir.path())
        .arg("--headless")
        .output()
        .unwrap();
    assert_eq!(check.status.code(), Some(1));
    assert_eq!(run.status.code(), Some(1));
    assert_eq!(check.stderr, run.stderr);
    assert!(run.stdout.is_empty());
    assert!(stderr(&check).contains("$Bad"));
}

#[test]
fn startup_preserves_multiline_diagnostics_byte_for_byte() {
    let dir = tempfile::tempdir().unwrap();
    minimal_story(dir.path(), "main.yarn");
    let manifest_path = dir.path().join("manifest.jsonc");
    let mut manifest: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&manifest_path).unwrap()).unwrap();
    manifest["initialState"] = serde_json::json!({"bad\r\nkey": 1});
    fs::write(manifest_path, manifest.to_string()).unwrap();
    let check = weftora().arg("check").arg(dir.path()).output().unwrap();
    let run = weftora()
        .arg("run")
        .arg(dir.path())
        .arg("--headless")
        .output()
        .unwrap();
    assert_eq!(check.status.code(), Some(1));
    assert_eq!(run.status.code(), Some(1));
    assert_eq!(check.stderr, run.stderr);
    assert!(check.stderr.windows(2).any(|pair| pair == b"\r\n"));
}

#[cfg(unix)]
#[test]
fn preview_passes_a_live_artifact_and_cleans_it_without_story_writes() {
    use std::os::unix::fs::PermissionsExt;
    let tmp = tempfile::tempdir().unwrap();
    let story = tmp.path().join("story");
    minimal_story(&story, "intro.yarn");
    let before: Vec<_> = fs::read_dir(&story)
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    let player = tmp.path().join("inspect-player");
    fs::write(&player, b"#!/bin/sh\ntest \"$2\" = --dialogue || exit 11\ntest -s \"$3\" || exit 12\nprintf '%s' \"$3\"\nexit 23\n").unwrap();
    fs::set_permissions(&player, fs::Permissions::from_mode(0o755)).unwrap();
    let output = weftora()
        .env("WEFTORA_PLAYER", &player)
        .arg("run")
        .arg(&story)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(23), "{}", stderr(&output));
    let path = std::path::PathBuf::from(String::from_utf8(output.stdout).unwrap());
    assert!(!path.exists(), "preview artifact was not cleaned up");
    let after: Vec<_> = fs::read_dir(&story)
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(before, after);
}

#[test]
fn typed_command_mismatch_preserves_yarn_context_and_never_activates() {
    let tmp = tempfile::tempdir().unwrap();
    let story = tmp.path().join("story");
    minimal_story(&story, "intro.yarn");
    let manifest_path = story.join("manifest.jsonc");
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
    manifest["manifestSchema"] = 1.into();
    manifest["storyVersion"] = "0.1.0".into();
    manifest["requires"] =
        serde_json::json!({"engineApi":">=0.1.0 <0.2.0","capabilities":["assets.catalog/1"]});
    manifest["catalogs"] = serde_json::json!({"images":["images.jsonc"]});
    fs::write(manifest_path, manifest.to_string()).unwrap();
    fs::write(story.join("image.bin"), b"typed lookup only").unwrap();
    fs::write(
        story.join("images.jsonc"),
        r#"[{"id":"bg.room","path":"image.bin"}]"#,
    )
    .unwrap();
    fs::write(
        story.join("intro.yarn"),
        "title: Start\n---\n<<sound bg.room>>\nHello.\n===\n",
    )
    .unwrap();
    let check = weftora().arg("check").arg(&story).output().unwrap();
    let run = weftora()
        .arg("run")
        .arg(&story)
        .arg("--headless")
        .output()
        .unwrap();
    assert_eq!(check.status.code(), Some(1));
    assert_eq!(run.status.code(), Some(1));
    assert_eq!(check.stderr, run.stderr);
    assert!(run.stdout.is_empty());
    assert!(stderr(&check).contains(
        "intro.yarn:3: `sound`: asset `bg.room` belongs to images, expected audio (node `Start`)"
    ));
    let out = tmp.path().join("export");
    let export = weftora()
        .arg("export")
        .arg(&story)
        .arg("--out")
        .arg(&out)
        .output()
        .unwrap();
    assert_eq!(export.status.code(), Some(1));
    assert!(!out.exists());
}

#[test]
fn screen_state_validation_uses_yarn_defaults_and_blocks_activation() {
    let tmp = tempfile::tempdir().unwrap();
    let story = tmp.path().join("story");
    minimal_story(&story, "intro.yarn");
    let manifest_path = story.join("manifest.jsonc");
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
    manifest["manifestSchema"] = 1.into();
    manifest["storyVersion"] = "0.1.0".into();
    manifest["requires"] =
        serde_json::json!({"engineApi":">=0.1.0 <0.2.0","capabilities":["ui.screens/1"]});
    manifest["screens"] = serde_json::json!(["screen.jsonc"]);
    manifest["initialScreen"] = "dialogue".into();
    fs::write(manifest_path, manifest.to_string()).unwrap();
    fs::write(
        story.join("intro.yarn"),
        "title: Start\n---\n<<declare $world__day = 1>>\nHello.\n===\n",
    )
    .unwrap();
    let mut screen = serde_json::json!({"screenSchema":1,"root":{"type":"screen","id":"dialogue","children":[{"type":"text","bind":{"state":"world.day"}},{"type":"choiceList","id":"choices"},{"type":"button","id":"set","text":"Set","action":{"do":"state.set","key":"world.day","value":2.0}}]}});
    let path = story.join("screen.jsonc");
    fs::write(&path, screen.to_string()).unwrap();
    let check = weftora().arg("check").arg(&story).output().unwrap();
    assert!(check.status.success(), "{}", stderr(&check));
    for value in [serde_json::json!(true), serde_json::json!("two")] {
        screen["root"]["children"][2]["action"]["value"] = value;
        fs::write(&path, screen.to_string()).unwrap();
        let check = weftora().arg("check").arg(&story).output().unwrap();
        let run = weftora()
            .arg("run")
            .arg(&story)
            .arg("--headless")
            .output()
            .unwrap();
        assert_eq!(check.status.code(), Some(1));
        assert_eq!(run.status.code(), Some(1));
        assert_eq!(check.stderr, run.stderr);
        assert!(run.stdout.is_empty());
        assert!(
            stderr(&check)
                .contains("screen.jsonc: /root/children/2/action/value: state `world.day` expects")
        );
    }
    screen["root"]["children"][2]["action"]["value"] = serde_json::json!(1e100);
    fs::write(&path, screen.to_string()).unwrap();
    let check = weftora().arg("check").arg(&story).output().unwrap();
    assert_eq!(check.status.code(), Some(1));
    assert!(stderr(&check).contains("does not fit Yarn's f32"));
    let manifest_path = story.join("manifest.jsonc");
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
    manifest["initialState"] = serde_json::json!({"world.day":1});
    fs::write(manifest_path, manifest.to_string()).unwrap();
    screen["root"]["children"][2]["action"]["value"] = serde_json::json!(16777217);
    fs::write(&path, screen.to_string()).unwrap();
    let check = weftora().arg("check").arg(&story).output().unwrap();
    assert_eq!(check.status.code(), Some(1));
    assert!(stderr(&check).contains("/root/children/2/action/value:"));
    assert!(stderr(&check).contains("outside ±16777216"));
}

#[test]
fn media_parser_check_and_headless_share_author_diagnostics() {
    let tmp = tempfile::tempdir().unwrap();
    let story = tmp.path().join("story");
    minimal_story(&story, "intro.yarn");
    let path = story.join("manifest.jsonc");
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    manifest["manifestSchema"] = 1.into();
    manifest["storyVersion"] = "0.1.0".into();
    manifest["requires"] = serde_json::json!({"engineApi":">=0.1.0 <0.2.0","capabilities":["assets.catalog/1","presentation.media/1"]});
    manifest["catalogs"] = serde_json::json!({"images":["images.jsonc"],"audio":["audio.jsonc"]});
    fs::write(&path, manifest.to_string()).unwrap();
    fs::write(
        story.join("media.bin"),
        b"lookup fixture, not decoder evidence",
    )
    .unwrap();
    fs::write(
        story.join("images.jsonc"),
        r#"[{"id":"image","path":"media.bin"}]"#,
    )
    .unwrap();
    fs::write(
        story.join("audio.jsonc"),
        r#"[{"id":"audio","path":"media.bin"}]"#,
    )
    .unwrap();
    for (command, needle) in [
        ("show left image 10001", "duration must be"),
        ("move left 0.5 0.5 0 1 0", "scale must be greater"),
        ("show missing image 0", "unknown view slot"),
        ("music image 0", "belongs to images, expected audio"),
        ("show left audio 0", "belongs to audio, expected images"),
        ("voice absent 100", "missing catalog ID"),
    ] {
        fs::write(
            story.join("intro.yarn"),
            format!("title: Start\n---\n<<{command}>>\nHello.\n===\n"),
        )
        .unwrap();
        let check = weftora().arg("check").arg(&story).output().unwrap();
        let run = weftora()
            .arg("run")
            .arg(&story)
            .arg("--headless")
            .output()
            .unwrap();
        assert_eq!(check.status.code(), Some(1));
        assert_eq!(check.stderr, run.stderr);
        assert!(run.stdout.is_empty());
        let error = stderr(&check);
        assert!(error.contains("intro.yarn:3:"), "{error}");
        assert!(error.contains("node `Start`"), "{error}");
        assert!(error.contains(needle), "{error}");
    }
    fs::write(
        story.join("intro.yarn"),
        "title: Start\n---\n<<show left image 0>>\nHello.\n===\n",
    )
    .unwrap();
    manifest["requires"]["capabilities"] = serde_json::json!(["assets.catalog/1"]);
    fs::write(path, manifest.to_string()).unwrap();
    let check = weftora().arg("check").arg(&story).output().unwrap();
    assert_eq!(check.status.code(), Some(1));
    assert!(stderr(&check).contains("requires `presentation.media/1`"));
}

#[test]
fn media_demo_replays_identically_and_exported_artifact_validates() {
    let demo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/presentation-demo");
    let mut expected = None;
    for _ in 0..3 {
        let output = weftora()
            .arg("run")
            .arg(&demo)
            .arg("--headless")
            .output()
            .unwrap();
        assert!(output.status.success(), "{}", stderr(&output));
        if let Some(expected) = &expected {
            assert_eq!(&output.stdout, expected);
        } else {
            expected = Some(output.stdout.clone());
        }
    }
    let tmp = tempfile::tempdir().unwrap();
    let player = tmp.path().join("player");
    fs::write(&player, b"export fixture only").unwrap();
    let export = tmp.path().join("export");
    let output = weftora()
        .env("WEFTORA_PLAYER", player)
        .arg("export")
        .arg(&demo)
        .arg("--out")
        .arg(&export)
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", stderr(&output));
    let story = weftora_story::Story::load(&export.join("story")).unwrap();
    let mut compiled =
        weftora_yarn::CompiledDialogue::load_artifact(&export.join(weftora_yarn::ARTIFACT_FILE))
            .unwrap();
    compiled
        .verify_sources(&story.dialogue_sources().unwrap())
        .unwrap();
    assert!(weftora_yarn::check_story(&story, &compiled).is_empty());
}

#[test]
fn invalid_user_volume_scope_type_range_and_ui_literals_fail_before_activation() {
    let tmp = tempfile::tempdir().unwrap();
    let story = tmp.path().join("story");
    minimal_story(&story, "intro.yarn");
    let path = story.join("manifest.jsonc");
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    manifest["manifestSchema"] = 1.into();
    manifest["storyVersion"] = "0.1.0".into();
    manifest["requires"] = serde_json::json!({"engineApi":">=0.1.0 <0.2.0","capabilities":["presentation.media/1","ui.screens/1"]});
    manifest["screens"] = serde_json::json!(["ui.jsonc"]);
    manifest["initialScreen"] = "dialogue".into();
    let mut screen = serde_json::json!({"screenSchema":1,"root":{"type":"screen","id":"dialogue","children":[{"type":"choiceList","id":"choices"},{"type":"button","id":"volume","text":"Set","action":{"do":"state.set","key":"settings.audio.master","value":0.5}}]}});
    for (default, scope, needle) in [
        (serde_json::json!(1.1), "settings", "float in 0..=1"),
        (serde_json::json!(1), "settings", "float in 0..=1"),
        (serde_json::json!(1.0), "persistent", "settings scope"),
    ] {
        manifest["state"] =
            serde_json::json!({"settings.audio.master":{"default":default,"scope":scope}});
        fs::write(&path, manifest.to_string()).unwrap();
        fs::write(story.join("ui.jsonc"), screen.to_string()).unwrap();
        let check = weftora().arg("check").arg(&story).output().unwrap();
        assert_eq!(check.status.code(), Some(1));
        assert!(stderr(&check).contains(needle), "{}", stderr(&check));
    }
    manifest["state"] =
        serde_json::json!({"settings.audio.master":{"default":1.0,"scope":"settings"}});
    fs::write(path, manifest.to_string()).unwrap();
    screen["root"]["children"][1]["action"]["value"] = serde_json::json!(-0.5);
    fs::write(story.join("ui.jsonc"), screen.to_string()).unwrap();
    let check = weftora().arg("check").arg(&story).output().unwrap();
    let run = weftora()
        .arg("run")
        .arg(&story)
        .arg("--headless")
        .output()
        .unwrap();
    assert_eq!(check.status.code(), Some(1));
    assert_eq!(check.stderr, run.stderr);
    assert!(run.stdout.is_empty());
    assert!(stderr(&check).contains("ui.jsonc: /root/children/1/action/value:"));
}

#[test]
fn yarn_created_volume_settings_are_rejected_without_screens_and_live_values_fail_headless() {
    let tmp = tempfile::tempdir().unwrap();
    let story = tmp.path().join("story");
    minimal_story(&story, "intro.yarn");
    let path = story.join("manifest.jsonc");
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    manifest["manifestSchema"] = 1.into();
    manifest["storyVersion"] = "0.1.0".into();
    manifest["requires"] =
        serde_json::json!({"engineApi":">=0.1.0 <0.2.0","capabilities":["presentation.media/1"]});
    fs::write(&path, manifest.to_string()).unwrap();
    for statement in [
        "declare $settings__audio__master = 0.5",
        "set $settings__audio__music = 0.5",
    ] {
        fs::write(
            story.join("intro.yarn"),
            format!("title: Start\n---\n<<{statement}>>\nHello.\n===\n"),
        )
        .unwrap();
        let check = weftora().arg("check").arg(&story).output().unwrap();
        let run = weftora()
            .arg("run")
            .arg(&story)
            .arg("--headless")
            .output()
            .unwrap();
        assert_eq!(check.status.code(), Some(1));
        assert_eq!(run.status.code(), Some(1));
        assert_eq!(check.stderr, run.stderr);
        assert!(run.stdout.is_empty());
        assert!(
            stderr(&check).contains("must use settings scope"),
            "{}",
            stderr(&check)
        );
    }
    manifest["state"] =
        serde_json::json!({"settings.audio.master":{"default":1.0,"scope":"settings"}});
    fs::write(path, manifest.to_string()).unwrap();
    fs::write(
        story.join("intro.yarn"),
        "title: Start\n---\nReady.\n<<set $settings__audio__master = 2>>\nAfter.\n===\n",
    )
    .unwrap();
    let check = weftora().arg("check").arg(&story).output().unwrap();
    assert!(check.status.success(), "{}", stderr(&check));
    let run = weftora()
        .arg("run")
        .arg(&story)
        .arg("--headless")
        .output()
        .unwrap();
    assert_eq!(run.status.code(), Some(1));
    assert!(stderr(&run).contains("float in 0..=1"));
    assert!(String::from_utf8_lossy(&run.stdout).contains("Ready."));
    assert!(!String::from_utf8_lossy(&run.stdout).contains("After."));
}

#[test]
fn yarn_syntax_missing_node_and_unknown_command_have_matching_file_node_errors() {
    for (source, expected) in [
        (
            "title: Start\n---\n<<if true>>\nHello\n===\n",
            "node 'Start'",
        ),
        (
            "title: Start\n---\n<<jump Missing>>\n===\n",
            "referenced by node 'Start'",
        ),
        ("title: Start\n---\n<<unknown_command>>\n===\n", "node"),
    ] {
        let dir = tempfile::tempdir().unwrap();
        minimal_story(dir.path(), "dialogue/main.yarn");
        fs::write(dir.path().join("dialogue/main.yarn"), source).unwrap();
        let check = weftora().arg("check").arg(dir.path()).output().unwrap();
        let run = weftora()
            .arg("run")
            .arg(dir.path())
            .arg("--headless")
            .output()
            .unwrap();
        assert_eq!(check.status.code(), Some(1));
        assert_eq!(run.status.code(), Some(1));
        assert_eq!(check.stderr, run.stderr);
        assert!(run.stdout.is_empty());
        let message = stderr(&check);
        assert!(message.contains("dialogue/main.yarn"), "{message}");
        assert!(message.contains(expected), "{message}");
        assert!(!message.contains("panicked at"), "{message}");
    }
}

#[test]
fn oversized_initial_string_is_rejected_before_activation() {
    let dir = tempfile::tempdir().unwrap();
    minimal_story(dir.path(), "main.yarn");
    fs::write(
        dir.path().join("main.yarn"),
        "title: Start\n---\n<<declare $text = \"x\">>\n{$text}\n===\n",
    )
    .unwrap();
    let path = dir.path().join("manifest.jsonc");
    let mut manifest: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    manifest["initialState"] = serde_json::json!({"text": "x".repeat(16385)});
    fs::write(path, manifest.to_string()).unwrap();
    let check = weftora().arg("check").arg(dir.path()).output().unwrap();
    let run = weftora()
        .arg("run")
        .arg(dir.path())
        .arg("--headless")
        .output()
        .unwrap();
    assert_eq!(check.status.code(), Some(1));
    assert_eq!(check.stderr, run.stderr);
    assert!(stderr(&check).contains("string budget"));
    assert!(run.stdout.is_empty());
}

#[test]
fn headless_change_history_is_bounded_across_blocking_lines() {
    let dir = tempfile::tempdir().unwrap();
    minimal_story(dir.path(), "main.yarn");
    // Even without the guard this finite fixture retains less than 40 MiB of change payloads.
    let source = format!(
        "title: Start\n---\n<<declare $text = \"\">>\n<<declare $n = 0>>\n<<set $text = \"{}\">>\n<<set $text = \"{}\">>\n<<set $n = $n + 1>>\nTick\n<<if $n < 600>>\n<<jump Start>>\n<<endif>>\n===\n",
        "a".repeat(16384),
        "b".repeat(16384)
    );
    fs::write(dir.path().join("main.yarn"), source).unwrap();
    let result = weftora()
        .arg("run")
        .arg(dir.path())
        .arg("--headless")
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(1));
    assert!(
        stderr(&result).contains("headless change budget exceeded"),
        "{}",
        stderr(&result)
    );
    assert!(!stderr(&result).contains("panicked at"));
    assert!(String::from_utf8(result.stdout).unwrap().lines().count() < 600);
}

fn checkpoint_story(root: &Path) {
    fs::create_dir_all(root).unwrap();
    fs::write(
        root.join("manifest.jsonc"),
        r#"{
        "manifestSchema":1,"id":"test.checkpoint","storyVersion":"0.1.0",
        "saveSchema":1,"title":"Checkpoint","entrypoint":"Start","dialogue":["main.yarn"],
        "requires":{"engineApi":">=0.1.0 <0.2.0","capabilities":["dialogue.yarn/1","save/1"]}
    }"#,
    )
    .unwrap();
    fs::write(root.join("main.yarn"), "title: Start\n---\n<<declare $n = 0>>\n<<checkpoint scene.start>>\n<<set $n = $n + 1>>\nFirst {$n}\n-> Left\n    <<set $n = $n + 10>>\n-> Right\n    <<set $n = $n + 20>>\nFinal {$n}\n===\n").unwrap();
}

fn checkpoint_run(story: &Path, saves: &Path, flags: &[&str]) -> Output {
    weftora()
        .arg("run")
        .arg(story)
        .arg("--headless")
        .args(flags)
        .arg("--save-dir")
        .arg(saves)
        .output()
        .unwrap()
}

#[test]
fn checkpoint_save_load_replays_prefix_once_and_continues_both_branches() {
    let temp = tempfile::tempdir().unwrap();
    let story = temp.path().join("story");
    let saves = temp.path().join("data");
    checkpoint_story(&story);
    let saved = checkpoint_run(&story, &saves, &["--save-at", "scene.start"]);
    assert!(saved.status.success(), "{}", stderr(&saved));
    let saved_events: Vec<serde_json::Value> = String::from_utf8(saved.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(saved_events[0]["text"], "First 1");
    assert_eq!(saved_events.last().unwrap()["event"], "saved");
    let bytes = fs::read(saves.join("save.json")).unwrap();
    let resaved = checkpoint_run(&story, &saves, &["--load", "--save-at", "scene.start"]);
    assert!(resaved.status.success(), "{}", stderr(&resaved));
    assert_eq!(fs::read(saves.join("save.json")).unwrap(), bytes);
    assert_eq!(fs::read(saves.join("save.bak")).unwrap(), bytes);
    for choice in ["0", "1"] {
        let normal = weftora()
            .arg("run")
            .arg(&story)
            .args(["--headless", "--choices", choice])
            .output()
            .unwrap();
        let loaded = checkpoint_run(&story, &saves, &["--load", "--choices", choice]);
        assert!(
            normal.status.success() && loaded.status.success(),
            "{}",
            stderr(&loaded)
        );
        assert_eq!(loaded.stdout, normal.stdout);
    }
}

#[test]
fn rejected_load_preserves_slot_and_publishes_no_gameplay_trace() {
    let temp = tempfile::tempdir().unwrap();
    let story = temp.path().join("story");
    let saves = temp.path().join("data");
    checkpoint_story(&story);
    let missing = checkpoint_run(&story, &saves, &["--load"]);
    assert!(!missing.status.success());
    assert!(missing.stdout.is_empty());
    assert!(!saves.exists());
    let saved = checkpoint_run(&story, &saves, &["--save-at", "scene.start"]);
    assert!(saved.status.success(), "{}", stderr(&saved));
    let bytes = fs::read(saves.join("save.json")).unwrap();
    let path = story.join("main.yarn");
    let original = fs::read_to_string(&path).unwrap();
    // Current content with the same schema/checkpoint is accepted and uses current text.
    fs::write(&path, original.replace("First {$n}", "Edited {$n}")).unwrap();
    let patched = checkpoint_run(&story, &saves, &["--load", "--choices", "0"]);
    assert!(patched.status.success(), "{}", stderr(&patched));
    assert!(String::from_utf8_lossy(&patched.stdout).contains("Edited 1"));
    for broken in [
        original.replace("scene.start", "scene.missing"),
        original.replace("First {$n}", "<<jump Other>>").to_string()
            + "\ntitle: Other\n---\nElsewhere\n===\n",
        original.replace("<<set $n = $n + 1>>", "<<set $n = 1 / 0>>"),
    ] {
        fs::write(&path, broken).unwrap();
        let loaded = checkpoint_run(&story, &saves, &["--load", "--choices", "0"]);
        assert!(
            !loaded.status.success(),
            "{}",
            String::from_utf8_lossy(&loaded.stdout)
        );
        assert!(loaded.stdout.is_empty());
        assert_eq!(fs::read(saves.join("save.json")).unwrap(), bytes);
    }
    fs::write(&path, original).unwrap();
    let manifest = story.join("manifest.jsonc");
    fs::write(
        &manifest,
        fs::read_to_string(&manifest)
            .unwrap()
            .replace("\"saveSchema\":1", "\"saveSchema\":2"),
    )
    .unwrap();
    let loaded = checkpoint_run(&story, &saves, &["--load"]);
    assert!(!loaded.status.success());
    assert!(stderr(&loaded).contains("no save migration"));
    assert!(loaded.stdout.is_empty());
    assert_eq!(fs::read(saves.join("save.json")).unwrap(), bytes);
}

#[test]
fn headless_save_options_reject_duplicates_and_story_internal_directories() {
    let temp = tempfile::tempdir().unwrap();
    let story = temp.path().join("story");
    checkpoint_story(&story);
    let internal = story.join("saves");
    let out = checkpoint_run(&story, &internal, &["--save-at", "scene.start"]);
    assert!(!out.status.success());
    assert!(stderr(&out).contains("outside the Story"));
    assert!(!internal.exists());
    let out = weftora()
        .arg("run")
        .arg(&story)
        .args(["--headless", "--load", "--load"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    let out = checkpoint_run(&story, &temp.path().join("data"), &["--save-at", "missing"]);
    assert!(!out.status.success());
    assert!(out.stdout.is_empty());
}

#[test]
fn checkpoint_nodes_refuse_session_reads_but_allow_deterministic_session_writes() {
    let temp = tempfile::tempdir().unwrap();
    checkpoint_story(temp.path());
    let manifest = temp.path().join("manifest.jsonc");
    let mut document: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&manifest).unwrap()).unwrap();
    document["state"] = serde_json::json!({"ui.flag":{"scope":"session","default":false}});
    fs::write(manifest, document.to_string()).unwrap();
    let yarn = temp.path().join("main.yarn");
    fs::write(&yarn, "title: Start\n---\n<<declare $ui__flag = false>>\n<<checkpoint scene.start>>\n{$ui__flag}\n===\n").unwrap();
    let out = weftora().arg("check").arg(temp.path()).output().unwrap();
    assert!(!out.status.success());
    assert!(stderr(&out).contains("cannot read Session variable $ui__flag"));
    fs::write(&yarn, "title: Start\n---\n<<declare $ui__flag = false>>\n<<checkpoint scene.start>>\n<<set $ui__flag = true>>\nReady\n===\n").unwrap();
    let out = weftora().arg("check").arg(temp.path()).output().unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
}
