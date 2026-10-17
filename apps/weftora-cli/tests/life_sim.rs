use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

fn story() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/life-sim")
}
fn run(root: &Path, flags: &[&str], saves: Option<&Path>) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_weftora"));
    command.arg("run").arg(root).arg("--headless").args(flags);
    if let Some(saves) = saves {
        command.arg("--save-dir").arg(saves);
    }
    command.output().unwrap()
}
fn events(out: Output) -> Vec<Value> {
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}
fn copy_story(root: &Path) {
    fs::create_dir_all(root.join("ui")).unwrap();
    for file in [
        "manifest.jsonc",
        "main.yarn",
        "events.jsonc",
        "mechanics.rhai",
        "ui/dashboard.jsonc",
        "ui/journal.jsonc",
    ] {
        fs::copy(story().join(file), root.join(file)).unwrap();
    }
}
#[test]
fn a_day_combines_clock_schedule_resources_inventory_and_seeded_pay() {
    for (choices, hour, energy, coins, worked, items) in [
        ("0,0,0,0", 14, 3, 2, true, json!(["permit", "receipt"])),
        ("0,0,1,1", 13, 2, 4, true, json!(["permit", "receipt"])),
        ("0,1,1", 11, 4, 0, false, json!(["permit"])),
        ("1", 9, 6, 0, false, json!(["permit"])),
    ] {
        let trace = events(run(&story(), &["--choices", choices], None));
        assert!(
            trace
                .iter()
                .filter(|e| e["event"] == "line")
                .all(|e| e["character"].is_null()),
            "narration must not be parsed as a speaker"
        );
        let end = trace.last().unwrap();
        assert_eq!(end["event"], "end");
        let state = &end["state"];
        assert_eq!(state["location"], "Home");
        assert_eq!(state["hour"], hour);
        assert_eq!(state["energy"], energy);
        assert_eq!(state["coins"], coins);
        assert_eq!(state["worked"], worked);
        assert_eq!(state["inventory"], items);
        assert_eq!(state["shift_open"], false);
        assert_eq!(
            state["schedule"],
            json!({"market":"09:00-12:00","open":false})
        );
        assert_eq!(
            state["rng"],
            if worked { -7046029254386353131i64 } else { 0 }
        );
        if worked {
            assert!(
                end["changes"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|c| c["key"] == "shift_open" && c["after"] == true)
            );
            assert_eq!(state["earned"], 4);
        }
    }
}
#[test]
fn work_is_conditional_on_time_energy_and_the_permit() {
    for (key, default, available) in [
        ("hour", json!(7), false),
        ("hour", json!(10), true),
        ("hour", json!(11), false),
        ("hour", json!(12), false),
        ("energy", json!(2), false),
        ("energy", json!(3), true),
        ("inventory", json!([]), false),
    ] {
        let temp = tempfile::tempdir().unwrap();
        copy_story(temp.path());
        let path = temp.path().join("manifest.jsonc");
        let mut manifest: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        manifest["state"][key]["default"] = default;
        fs::write(path, manifest.to_string()).unwrap();
        let trace = events(run(temp.path(), &["--choices", "0,1,1"], None));
        let work = trace
            .iter()
            .filter(|e| e["event"] == "choices")
            .flat_map(|e| e["options"].as_array().unwrap())
            .find(|o| o["text"] == "Work the morning shift")
            .unwrap();
        assert_eq!(work["available"], available, "{key}");
        assert_eq!(trace.last().unwrap()["state"]["worked"], false);
        assert_eq!(trace.last().unwrap()["state"]["rng"], 0);
    }
}
#[test]
fn every_checkpoint_replays_actions_rng_and_collections_once() {
    let baseline = events(run(&story(), &["--choices", "0,0,0,0"], None))
        .last()
        .unwrap()["state"]
        .clone();
    for (checkpoint, remaining, hour, coins) in [
        ("day.home", "0,0,0,0", 8, 0),
        ("day.market", "0,0,0", 9, 0),
        ("day.shift", "0,0", 11, 4),
        ("day.return", "0", 13, 2),
        ("day.end", "", 14, 2),
    ] {
        let saves = tempfile::tempdir().unwrap();
        let saved = events(run(
            &story(),
            &["--choices", "0,0,0,0", "--save-at", checkpoint],
            Some(saves.path()),
        ));
        let point = saved.last().unwrap();
        assert_eq!(point["event"], "saved");
        assert_eq!(point["state"]["hour"], hour);
        assert_eq!(point["state"]["coins"], coins);
        if checkpoint == "day.market" {
            assert_eq!(point["state"]["schedule"]["open"], true);
            assert_eq!(point["state"]["shift_open"], true);
        }
        let slot = saves.path().join("save.json");
        let bytes = fs::read(&slot).unwrap();
        events(run(
            &story(),
            &["--load", "--save-at", checkpoint],
            Some(saves.path()),
        ));
        assert_eq!(fs::read(&slot).unwrap(), bytes);
        assert_eq!(fs::read(saves.path().join("save.bak")).unwrap(), bytes);
        let mut flags = vec!["--load"];
        if !remaining.is_empty() {
            flags.extend(["--choices", remaining]);
        }
        let restored = events(run(&story(), &flags, Some(saves.path())));
        assert_eq!(restored.last().unwrap()["state"], baseline, "{checkpoint}");
    }
}
#[test]
fn story_a_and_b_reject_each_others_saves_without_output_or_file_changes() {
    let story_a = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/story-a");
    for (source, checkpoint, target) in [
        (story_a.as_path(), "scene.menu", story()),
        (story().as_path(), "day.home", story_a.clone()),
    ] {
        let saves = tempfile::tempdir().unwrap();
        events(run(source, &["--save-at", checkpoint], Some(saves.path())));
        let slot = saves.path().join("save.json");
        let bytes = fs::read(&slot).unwrap();
        let failed = run(&target, &["--load"], Some(saves.path()));
        assert!(!failed.status.success());
        assert!(failed.stdout.is_empty());
        assert!(
            String::from_utf8_lossy(&failed.stderr).contains("save belongs to another Story"),
            "{}",
            String::from_utf8_lossy(&failed.stderr)
        );
        assert_eq!(fs::read(slot).unwrap(), bytes);
        assert!(!saves.path().join("save.bak").exists());
    }
}

#[test]
fn loaded_days_can_take_an_alternate_branch_and_skip_work() {
    for (checkpoint, before, after, hour, energy, coins, worked) in [
        ("day.shift", "0,0", "1,1", 13, 2, 4, true),
        ("day.return", "0,1", "1", 11, 4, 0, false),
    ] {
        let saves = tempfile::tempdir().unwrap();
        events(run(
            &story(),
            &["--choices", before, "--save-at", checkpoint],
            Some(saves.path()),
        ));
        let trace = events(run(
            &story(),
            &["--load", "--choices", after],
            Some(saves.path()),
        ));
        let state = &trace.last().unwrap()["state"];
        assert_eq!(state["hour"], hour);
        assert_eq!(state["energy"], energy);
        assert_eq!(state["coins"], coins);
        assert_eq!(state["worked"], worked);
        assert_eq!(
            state["rng"],
            if worked { -7046029254386353131i64 } else { 0 }
        );
    }
}
#[test]
fn pay_edit_with_schema_bump_refuses_old_save_and_preserves_both_files() {
    let saves = tempfile::tempdir().unwrap();
    for _ in 0..2 {
        events(run(
            &story(),
            &["--choices", "0,0", "--save-at", "day.shift"],
            Some(saves.path()),
        ));
    }
    let before = ["save.json", "save.bak"].map(|file| fs::read(saves.path().join(file)).unwrap());
    let edited = tempfile::tempdir().unwrap();
    copy_story(edited.path());
    let path = edited.path().join("manifest.jsonc");
    let mut manifest: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    manifest["saveSchema"] = json!(2);
    fs::write(path, manifest.to_string()).unwrap();
    let path = edited.path().join("mechanics.rhai");
    fs::write(
        &path,
        fs::read_to_string(&path)
            .unwrap()
            .replace("let base_pay = 3;", "let base_pay = 5;"),
    )
    .unwrap();
    let trace = events(run(edited.path(), &["--choices", "0,0,0,0"], None));
    assert_eq!(trace.last().unwrap()["state"]["earned"], 6);
    assert_eq!(trace.last().unwrap()["state"]["coins"], 4);
    let failed = run(edited.path(), &["--load"], Some(saves.path()));
    assert!(!failed.status.success());
    assert!(failed.stdout.is_empty());
    assert!(
        String::from_utf8_lossy(&failed.stderr).contains("no save migration from schema 1 to 2")
    );
    for (file, bytes) in ["save.json", "save.bak"].into_iter().zip(before) {
        assert_eq!(fs::read(saves.path().join(file)).unwrap(), bytes);
    }
}
