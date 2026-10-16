use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

fn source() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/screens-demo")
}
fn copy_story(root: &Path) {
    fs::create_dir_all(root.join("ui")).unwrap();
    for file in [
        "manifest.jsonc",
        "main.yarn",
        "ui/menu.jsonc",
        "ui/activity.jsonc",
        "ui/dialogue.jsonc",
        "ui/pause.jsonc",
    ] {
        fs::copy(source().join(file), root.join(file)).unwrap();
    }
}
#[test]
fn themed_screens_validate_and_both_dialogue_branches_complete() {
    let out = Command::new(env!("CARGO_BIN_EXE_weftora"))
        .arg("check")
        .arg(source())
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    for (choice, task) in [("0", "Station shift"), ("1", "Rest day")] {
        let out = Command::new(env!("CARGO_BIN_EXE_weftora"))
            .arg("run")
            .arg(source())
            .args(["--headless", "--choices", choice])
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        let events: Vec<Value> = String::from_utf8_lossy(&out.stdout)
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        let end = events.last().unwrap();
        assert_eq!(end["event"], "end");
        assert_eq!(end["state"]["task"], task);
        assert_eq!(end["state"]["prepared"], false);
    }
}
#[test]
fn invalid_authored_screens_block_check_run_and_export_before_output() {
    let fixtures: Vec<Value> =
        serde_json::from_str(include_str!("fixtures/screens-invalid.json")).unwrap();
    for fixture in fixtures {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("story");
        copy_story(&root);
        let path = root.join(fixture["file"].as_str().unwrap());
        let mut document: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        *document
            .pointer_mut(fixture["pointer"].as_str().unwrap())
            .unwrap() = fixture["value"].clone();
        fs::write(&path, document.to_string()).unwrap();
        let destination = temp.path().join("export");
        for verb in ["check", "run", "export"] {
            let mut command = Command::new(env!("CARGO_BIN_EXE_weftora"));
            command.arg(verb).arg(&root);
            if verb == "run" {
                command.arg("--headless");
            }
            if verb == "export" {
                command.arg("--out").arg(&destination);
            }
            let out = command.output().unwrap();
            let message = String::from_utf8_lossy(&out.stderr);
            assert!(!out.status.success(), "{} / {verb}", fixture["name"]);
            assert!(
                message.contains(fixture["file"].as_str().unwrap()),
                "{message}"
            );
            for expected in fixture["diagnostics"].as_array().unwrap() {
                assert!(message.contains(expected.as_str().unwrap()), "{message}");
            }
            assert!(out.stdout.is_empty(), "invalid package must not activate");
            assert!(!destination.exists(), "invalid package must not publish");
        }
    }
}
