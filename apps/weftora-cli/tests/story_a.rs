use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

fn story() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/story-a")
}
fn run(root: &Path, flags: &[&str], saves: Option<&Path>) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_weftora"));
    command.arg("run").arg(root).arg("--headless").args(flags);
    if let Some(directory) = saves {
        command.arg("--save-dir").arg(directory);
    }
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    output
}
fn events(output: &Output) -> Vec<Value> {
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}
#[test]
fn both_story_a_branches_finish_with_distinct_routes_and_one_attempt() {
    for (choice, route) in [("0", "repair"), ("1", "roof")] {
        let output = run(&story(), &["--choices", choice], None);
        let trace = events(&output);
        let end = trace.last().unwrap();
        assert_eq!(end["event"], "end");
        assert_eq!(end["state"]["story.route"], route);
        assert_eq!(end["state"]["story.signal"], true);
        assert_eq!(end["state"]["story.attempts"], 1.0);
        for command in ["show", "music", "effect"] {
            assert!(
                trace
                    .iter()
                    .any(|event| event["event"] == "command" && event["name"] == command)
            );
        }
        assert!(
            trace
                .iter()
                .any(|event| event["name"] == "show" && event["args"][0] == "cg")
        );
    }
}
#[test]
fn real_story_branch_checkpoints_replay_the_opening_once_and_resave_identically() {
    for (choice, checkpoint, route) in
        [("0", "scene.repair", "repair"), ("1", "scene.roof", "roof")]
    {
        let saves = tempfile::tempdir().unwrap();
        let initial = run(
            &story(),
            &["--choices", choice, "--save-at", checkpoint],
            Some(saves.path()),
        );
        let initial = events(&initial);
        assert_eq!(initial.last().unwrap()["event"], "saved");
        assert_eq!(initial.last().unwrap()["state"]["story.attempts"], 1.0);
        let bytes = fs::read(saves.path().join("save.json")).unwrap();
        let resaved = run(
            &story(),
            &["--load", "--save-at", checkpoint],
            Some(saves.path()),
        );
        assert_eq!(
            events(&resaved).last().unwrap()["state"]["story.attempts"],
            1.0
        );
        assert_eq!(fs::read(saves.path().join("save.json")).unwrap(), bytes);
        assert_eq!(fs::read(saves.path().join("save.bak")).unwrap(), bytes);
        let loaded = run(&story(), &["--load"], Some(saves.path()));
        let loaded = events(&loaded);
        let end = loaded.last().unwrap();
        assert_eq!(end["event"], "end");
        assert_eq!(end["state"]["story.route"], route);
        assert_eq!(end["state"]["story.signal"], true);
        assert_eq!(end["state"]["story.attempts"], 1.0);
    }
}

#[test]
fn arrival_entrypoint_preserves_legacy_title_checkpoint_saves() {
    fn copy(source: &Path, destination: &Path) {
        fs::create_dir_all(destination).unwrap();
        for entry in fs::read_dir(source).unwrap() {
            let entry = entry.unwrap();
            let target = destination.join(entry.file_name());
            if entry.file_type().unwrap().is_dir() {
                copy(&entry.path(), &target);
            } else {
                fs::copy(entry.path(), target).unwrap();
            }
        }
    }
    let fresh = events(&run(
        &story(),
        &["--save-at", "scene.arrival"],
        Some(tempfile::tempdir().unwrap().path()),
    ));
    let first_line = fresh.iter().find(|event| event["event"] == "line").unwrap();
    assert!(
        first_line["text"]
            .as_str()
            .unwrap()
            .contains("The ferry leaves at dawn.")
    );
    assert!(!fresh.iter().any(|event| event["event"] == "choices"));

    let legacy = tempfile::tempdir().unwrap();
    copy(&story(), legacy.path());
    let manifest = legacy.path().join("manifest.jsonc");
    let mut document: Value = serde_json::from_slice(&fs::read(&manifest).unwrap()).unwrap();
    document["entrypoint"] = "Start".into();
    fs::write(manifest, document.to_string()).unwrap();
    let saves = tempfile::tempdir().unwrap();
    let original = events(&run(
        legacy.path(),
        &["--save-at", "scene.menu"],
        Some(saves.path()),
    ));
    assert!(original.iter().any(|event| event["event"] == "choices"));
    let bytes = fs::read(saves.path().join("save.json")).unwrap();
    let restored = events(&run(
        &story(),
        &["--load", "--save-at", "scene.menu"],
        Some(saves.path()),
    ));
    let choices = |trace: &[Value]| {
        trace
            .iter()
            .find(|event| event["event"] == "choices")
            .unwrap()
            .clone()
    };
    assert_eq!(choices(&restored), choices(&original));
    assert_eq!(fs::read(saves.path().join("save.json")).unwrap(), bytes);
    let completed = events(&run(
        &story(),
        &["--load", "--choices", "1,0,0,1"],
        Some(saves.path()),
    ));
    assert_eq!(completed.last().unwrap()["state"]["story.route"], "roof");
    assert_eq!(completed.last().unwrap()["state"]["story.attempts"], 1.0);
}
