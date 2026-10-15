use serde_json::{Value, json};
use std::{
    fs,
    path::Path,
    process::{Command, Output},
};

fn fixture() -> tempfile::TempDir {
    let example = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/rules-demo");
    let temp = tempfile::tempdir().unwrap();
    fs::create_dir(temp.path().join("story")).unwrap();
    for file in ["manifest.jsonc", "main.yarn", "events.jsonc"] {
        fs::copy(example.join(file), temp.path().join("story").join(file)).unwrap();
    }
    temp
}
fn run(temp: &Path, flags: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_weftora"));
    command
        .arg("run")
        .arg(temp.join("story"))
        .arg("--headless")
        .args(flags);
    if flags.contains(&"--load") || flags.contains(&"--save-at") {
        command.arg("--save-dir").arg(temp.join("saves"));
    }
    command.output().unwrap()
}
fn success(output: Output) -> Vec<Value> {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|s| serde_json::from_str(s).unwrap())
        .collect()
}

#[test]
fn real_story_rules_run_at_the_documented_boundary_and_replay_deterministically() {
    let temp = fixture();
    let checked = Command::new(env!("CARGO_BIN_EXE_weftora"))
        .arg("check")
        .arg(temp.path().join("story"))
        .output()
        .unwrap();
    assert!(
        checked.status.success(),
        "{}",
        String::from_utf8_lossy(&checked.stderr)
    );
    let mut golden = None;
    for _ in 0..10 {
        let output = run(temp.path(), &[]);
        let bytes = output.stdout.clone();
        let trace = success(output);
        let lines: Vec<_> = trace
            .iter()
            .filter(|v| v["event"] == "line")
            .map(|v| v["text"].as_str().unwrap())
            .collect();
        assert_eq!(lines, ["First points 0", "After rules 10"]);
        let end = trace.last().unwrap();
        assert_eq!(end["state"]["points"], 10);
        assert_eq!(end["state"]["awarded"], true);
        assert_eq!(end["changes"].as_array().unwrap().len(), 4);
        assert_eq!(end["signals"].as_array().unwrap().len(), 4);
        if let Some(golden) = &golden {
            assert_eq!(&bytes, golden);
        } else {
            golden = Some(bytes);
        }
    }
}

#[test]
fn checkpoint_rules_execute_once_and_failed_restore_publishes_nothing() {
    let temp = fixture();
    let saved = success(run(temp.path(), &["--save-at", "rules.start"]));
    assert_eq!(saved.last().unwrap()["state"]["points"], 10);
    let slot = temp.path().join("saves/save.json");
    let bytes = fs::read(&slot).unwrap();
    let restored = success(run(temp.path(), &["--load", "--save-at", "rules.start"]));
    assert_eq!(restored.last().unwrap()["state"]["points"], 10);
    assert_eq!(fs::read(&slot).unwrap(), bytes);
    let backup = temp.path().join("saves/save.bak");
    assert_eq!(fs::read(&backup).unwrap(), bytes);
    let rules_file = temp.path().join("story/events.jsonc");
    let mut rules: Value = serde_json::from_slice(&fs::read(&rules_file).unwrap()).unwrap();
    rules["events"][0]["actions"] = json!([
        {"do":"state.set","key":"points","value":20},
        {"do":"state.add","key":"points","amount":i64::MAX}
    ]);
    fs::write(rules_file, rules.to_string()).unwrap();
    let failed = run(temp.path(), &["--load"]);
    assert!(!failed.status.success());
    assert!(
        failed.stdout.is_empty(),
        "candidate output leaked: {}",
        String::from_utf8_lossy(&failed.stdout)
    );
    let error = String::from_utf8_lossy(&failed.stderr);
    assert!(
        error.contains("events.jsonc")
            && error.contains("/events/0/actions/1")
            && error.contains("overflow"),
        "{error}"
    );
    assert_eq!(fs::read(slot).unwrap(), bytes);
    assert_eq!(fs::read(backup).unwrap(), bytes);
}

#[test]
fn writes_before_the_checkpoint_are_settled_before_capture() {
    let temp = fixture();
    fs::write(temp.path().join("story/main.yarn"), "title: Start\n---\n<<declare $input = false>>\n<<declare $points = 0>>\n<<set $input = true>>\n<<jump Saved>>\n===\ntitle: Saved\n---\n<<checkpoint rules.start>>\n<<set $input = true>>\nPoints {$points}\n===\n").unwrap();
    let saved = success(run(temp.path(), &["--save-at", "rules.start"]));
    let loaded = success(run(temp.path(), &["--load", "--save-at", "rules.start"]));
    assert_eq!(saved.last().unwrap()["state"]["points"], 10);
    assert_eq!(loaded.last().unwrap()["state"]["points"], 10);
    assert_eq!(saved[0]["text"], "Points 10");
    assert_eq!(loaded[0]["text"], "Points 10");
}

#[test]
fn rule_cycles_fail_before_returning_a_display() {
    let temp = fixture();
    fs::write(temp.path().join("story/events.jsonc"), r#"{"rulesSchema":1,"events":[{"id":"cycle","trigger":"state.changed.input","actions":[{"do":"state.set","key":"input","value":false},{"do":"state.set","key":"input","value":true}]}]}"#).unwrap();
    let output = run(temp.path(), &[]);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(
        error.contains("budget (1024)") && error.contains("events.jsonc"),
        "{error}"
    );
}

#[test]
fn checkpoint_loops_do_not_reset_the_shared_state_write_budget() {
    let temp = fixture();
    fs::write(temp.path().join("story/main.yarn"), "title: Start\n---\n<<declare $input = false>>\n<<checkpoint rules.start>>\n<<set $input = not $input>>\n<<jump Start>>\n===\n").unwrap();
    fs::write(
        temp.path().join("story/events.jsonc"),
        json!({"rulesSchema":1,"events":[{
            "id":"writes","trigger":"state.changed.input",
            "actions":vec![json!({"do":"state.add","key":"points","amount":1});100]
        }]})
        .to_string(),
    )
    .unwrap();
    let output = run(temp.path(), &[]);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(error.contains("state write budget (4096)"), "{error}");
}
