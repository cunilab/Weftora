mod support;

use serde_json::Value;
use std::{
    fs,
    path::Path,
    process::{Command, Output},
};

fn cli() -> Command {
    Command::new(env!("CARGO_BIN_EXE_weftora"))
}
fn success(output: Output) -> Output {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    output
}
fn refused(root: &Path) {
    let out = cli().arg("new").arg(root).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stdout.is_empty());
    assert!(String::from_utf8_lossy(&out.stderr).contains("already exists"));
}

#[test]
fn new_story_checks_runs_both_branches_and_exports_without_example_dependencies() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("My Story ü");
    success(cli().arg("new").arg(&root).output().unwrap());
    assert_eq!(fs::read_dir(&root).unwrap().count(), 3);
    success(cli().arg("check").arg(&root).output().unwrap());
    for (choice, ready) in [("0", true), ("1", false)] {
        let out = success(
            cli()
                .arg("run")
                .arg(&root)
                .args(["--headless", "--choices", choice])
                .output()
                .unwrap(),
        );
        let trace: Vec<Value> = String::from_utf8(out.stdout)
            .unwrap()
            .lines()
            .map(|s| serde_json::from_str(s).unwrap())
            .collect();
        assert!(
            trace.iter().any(|event| event["event"] == "choices"
                && event["options"].as_array().unwrap().len() == 2)
        );
        assert_eq!(trace.last().unwrap()["event"], "end");
        assert_eq!(trace.last().unwrap()["state"]["ready"], ready);
    }
    // Export plumbing only; the real-player validator runs in the generated-Story proof.
    let player = support::player(temp.path());
    let exported = temp.path().join("game");
    success(
        cli()
            .env("WEFTORA_PLAYER", &player)
            .arg("export")
            .arg(&root)
            .arg("--out")
            .arg(&exported)
            .output()
            .unwrap(),
    );
    assert!(exported.join("compiled/dialogue.json").is_file());
    assert!(exported.join("weftora-export.json").is_file());
    assert_eq!(
        fs::read(exported.join("story/main.yarn")).unwrap(),
        fs::read(root.join("main.yarn")).unwrap()
    );
}

#[test]
fn new_refuses_existing_files_and_empty_or_populated_directories() {
    let temp = tempfile::tempdir().unwrap();
    let file = temp.path().join("file");
    fs::write(&file, b"keep file").unwrap();
    refused(&file);
    assert_eq!(fs::read(&file).unwrap(), b"keep file");
    let empty = temp.path().join("empty");
    fs::create_dir(&empty).unwrap();
    refused(&empty);
    assert_eq!(fs::read_dir(&empty).unwrap().count(), 0);
    let used = temp.path().join("used");
    fs::create_dir(&used).unwrap();
    fs::write(used.join("manifest.jsonc"), b"my existing Story").unwrap();
    refused(&used);
    assert_eq!(
        fs::read(used.join("manifest.jsonc")).unwrap(),
        b"my existing Story"
    );
    assert_eq!(fs::read_dir(&used).unwrap().count(), 1);
}

#[test]
fn new_reports_missing_parent_without_creating_ancestors() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("missing/story");
    let out = cli().arg("new").arg(&root).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stdout.is_empty());
    assert!(String::from_utf8_lossy(&out.stderr).contains("existing writable parent"));
    assert!(!temp.path().join("missing").exists());
}

#[test]
fn new_usage_errors_do_not_create_folders() {
    let temp = tempfile::tempdir().unwrap();
    for args in [
        vec!["new"],
        vec!["new", "--force"],
        vec!["new", "story", "--force"],
    ] {
        let out = cli().current_dir(temp.path()).args(args).output().unwrap();
        assert_eq!(out.status.code(), Some(2));
        assert!(out.stdout.is_empty());
        assert!(String::from_utf8_lossy(&out.stderr).contains("weftora new <story-dir>"));
        assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 0);
    }
}

#[test]
fn concurrent_new_commands_choose_one_winner_and_preserve_a_valid_story() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("story");
    let mut first = cli()
        .arg("new")
        .arg(&root)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();
    let mut second = cli()
        .arg("new")
        .arg(&root)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();
    let mut codes = [
        first.wait().unwrap().code().unwrap(),
        second.wait().unwrap().code().unwrap(),
    ];
    codes.sort();
    assert_eq!(codes, [0, 1]);
    success(cli().arg("check").arg(&root).output().unwrap());
    assert_eq!(fs::read_dir(&root).unwrap().count(), 3);
}

#[cfg(unix)]
#[test]
fn new_refuses_existing_and_dangling_symlinks_without_target_writes() {
    let temp = tempfile::tempdir().unwrap();
    let target = temp.path().join("target");
    fs::create_dir(&target).unwrap();
    let link = temp.path().join("link");
    std::os::unix::fs::symlink(&target, &link).unwrap();
    refused(&link);
    assert_eq!(fs::read_dir(&target).unwrap().count(), 0);
    let dangling = temp.path().join("dangling");
    std::os::unix::fs::symlink(temp.path().join("absent"), &dangling).unwrap();
    refused(&dangling);
    assert!(!temp.path().join("absent").exists());
}

#[cfg(windows)]
#[test]
fn new_refuses_junction_without_target_writes() {
    let temp = tempfile::tempdir().unwrap();
    let target = temp.path().join("target");
    fs::create_dir(&target).unwrap();
    let junction = temp.path().join("junction");
    let result = Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(&junction)
        .arg(&target)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    refused(&junction);
    assert_eq!(fs::read_dir(&target).unwrap().count(), 0);
    fs::remove_dir(&target).unwrap();
    refused(&junction);
    assert!(!target.exists());
    fs::remove_dir(&junction).unwrap();
}
