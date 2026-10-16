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

fn commands_fixture() -> tempfile::TempDir {
    let example = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/commands-demo");
    let temp = tempfile::tempdir().unwrap();
    fs::create_dir(temp.path().join("story")).unwrap();
    for file in [
        "manifest.jsonc",
        "main.yarn",
        "events.jsonc",
        "screen.jsonc",
    ] {
        fs::copy(example.join(file), temp.path().join("story").join(file)).unwrap();
    }
    temp
}
#[test]
fn yarn_commands_execute_once_and_replay_before_checkpoint_display() {
    let temp = commands_fixture();
    let saved = success(run(temp.path(), &["--save-at", "commands.start"]));
    assert_eq!(saved.last().unwrap()["state"]["points"], 2);
    let slot = temp.path().join("saves/save.json");
    let bytes = fs::read(&slot).unwrap();
    let replay = success(run(temp.path(), &["--load", "--save-at", "commands.start"]));
    let lines: Vec<_> = replay
        .iter()
        .filter(|v| v["event"] == "line")
        .map(|v| v["text"].as_str().unwrap())
        .collect();
    assert_eq!(lines, ["First points 2"]);
    let resumed = success(run(temp.path(), &["--load"]));
    assert_eq!(resumed.last().unwrap()["state"]["points"], 3);
    assert_eq!(fs::read(&slot).unwrap(), bytes);
    assert!(!replay.iter().any(|v| v["event"] == "command"));
    let rules_file = temp.path().join("story/events.jsonc");
    let mut rules: Value = serde_json::from_slice(&fs::read(&rules_file).unwrap()).unwrap();
    rules["commands"][1]["actions"] = json!([
        {"do":"state.set","key":"points","value":20},
        {"do":"state.add","key":"points","amount":i64::MAX}
    ]);
    fs::write(rules_file, rules.to_string()).unwrap();
    let failed = run(temp.path(), &["--load"]);
    assert!(!failed.status.success());
    assert!(failed.stdout.is_empty());
    let error = String::from_utf8_lossy(&failed.stderr);
    assert!(
        error.contains("/commands/1/actions/1") && error.contains("overflow"),
        "{error}"
    );
    assert_eq!(fs::read(slot).unwrap(), bytes);
}
#[test]
fn yarn_command_arguments_are_checked_statically_and_after_interpolation() {
    let temp = commands_fixture();
    let path = temp.path().join("story/main.yarn");
    let source = fs::read_to_string(&path).unwrap();
    fs::write(&path, source.replace("<<award 2>>", "<<award true>>")).unwrap();
    let failed = run(temp.path(), &[]);
    assert!(!failed.status.success());
    let error = String::from_utf8_lossy(&failed.stderr);
    assert!(
        error.contains("main.yarn") && error.contains("expected Int"),
        "{error}"
    );
    fs::write(&path, source.replace("<<award 2>>", "<<award {1 + 1}>>")).unwrap();
    let trace = success(run(temp.path(), &[]));
    assert_eq!(trace.last().unwrap()["state"]["points"], 3);
    fs::write(&path, source.replace("<<award 2>>", "<<award {1 / 2}>>")).unwrap();
    let failed = run(temp.path(), &[]);
    assert!(!failed.status.success());
    assert!(String::from_utf8_lossy(&failed.stderr).contains("expected Int"));
}

#[test]
fn yarn_typed_commands_preserve_quoted_strings_and_parse_interpolated_scalars() {
    let temp = commands_fixture();
    let path = temp.path().join("story/events.jsonc");
    let mut rules: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    rules["commands"].as_array_mut().unwrap().push(json!({"id":"typed","parameters":[{"name":"flag","type":"bool"},{"name":"ratio","type":"float"},{"name":"text","type":"string"}],"actions":[{"do":"state.set","key":"label","value":{"argument":"text"}}]}));
    fs::write(path, rules.to_string()).unwrap();
    let manifest_path = temp.path().join("story/manifest.jsonc");
    let mut manifest: Value = serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
    manifest["state"]["label"] = json!({"default":"initial"});
    fs::write(manifest_path, manifest.to_string()).unwrap();
    fs::write(
        temp.path().join("story/main.yarn"),
        r#"title: Start
---
<<declare $label = "initial">>
<<checkpoint commands.start>>
<<typed {true} {1.25} "hello world">>
Label {$label}
<<typed false 1.0 "">>
Empty "{$label}"
===
"#,
    )
    .unwrap();
    let trace = success(run(temp.path(), &[]));
    let lines: Vec<_> = trace
        .iter()
        .filter(|v| v["event"] == "line")
        .map(|v| v["text"].as_str().unwrap())
        .collect();
    assert_eq!(lines, ["Label hello world", "Empty \"\""]);
    assert_eq!(trace.last().unwrap()["state"]["label"], "");
}

#[test]
fn yarn_command_string_interpolation_refuses_before_activation() {
    let temp = commands_fixture();
    let path = temp.path().join("story/events.jsonc");
    let mut rules: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    rules["commands"]
        .as_array_mut()
        .unwrap()
        .push(json!({"id":"greet","parameters":[{"name":"name","type":"string"}],"actions":[]}));
    fs::write(path, rules.to_string()).unwrap();
    fs::write(
        temp.path().join("story/main.yarn"),
        "title: Start\n---\n<<checkpoint commands.start>>\n<<greet {\"Ann Lee\"}>>\nDone\n===\n",
    )
    .unwrap();
    let failed = run(temp.path(), &[]);
    assert!(!failed.status.success());
    assert!(failed.stdout.is_empty());
    assert!(String::from_utf8_lossy(&failed.stderr).contains("string arguments must be literal"));
}

#[test]
fn commands_run_before_vm_notification_feedback_and_later_yarn_reads() {
    let temp = commands_fixture();
    let path = temp.path().join("story/events.jsonc");
    let mut rules: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    rules["events"][0]["condition"] =
        json!({"op":"compare","operator":"==","left":{"state":"points"},"right":{"literal":2}});
    fs::write(path, rules.to_string()).unwrap();
    fs::write(temp.path().join("story/main.yarn"), "title: Start\n---\n<<declare $points = 0>>\n<<declare $input = false>>\n<<checkpoint commands.start>>\n<<set $input = true>>\n<<award 2>>\n<<if $points == 12>>\nObserved 12\n<<else>>\nWrong value {$points}\n<<endif>>\n===\n").unwrap();
    let trace = success(run(temp.path(), &[]));
    let lines: Vec<_> = trace
        .iter()
        .filter(|v| v["event"] == "line")
        .map(|v| v["text"].as_str().unwrap())
        .collect();
    assert_eq!(lines, ["Observed 12"]);
    assert_eq!(trace.last().unwrap()["state"]["points"], 12);
}

fn collections_fixture() -> tempfile::TempDir {
    let example = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/collections-demo");
    let temp = tempfile::tempdir().unwrap();
    fs::create_dir(temp.path().join("story")).unwrap();
    for file in [
        "manifest.jsonc",
        "main.yarn",
        "events.jsonc",
        "screen.jsonc",
    ] {
        fs::copy(example.join(file), temp.path().join("story").join(file)).unwrap();
    }
    temp
}
#[test]
fn collection_checkpoint_replay_is_byte_identical_and_failed_candidate_publishes_nothing() {
    let temp = collections_fixture();
    success(run(temp.path(), &["--save-at", "collections.next"]));
    let slot = temp.path().join("saves/save.json");
    let bytes = fs::read(&slot).unwrap();
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["body"]["state"]["items"], json!(["key", "ticket"]));
    assert_eq!(body["body"]["state"]["schedule"]["energy"], 3);
    success(run(
        temp.path(),
        &["--load", "--save-at", "collections.next"],
    ));
    assert_eq!(fs::read(&slot).unwrap(), bytes);
    let backup = temp.path().join("saves/save.bak");
    let rules_file = temp.path().join("story/events.jsonc");
    let mut rules: Value = serde_json::from_slice(&fs::read(&rules_file).unwrap()).unwrap();
    rules["commands"][1]["actions"] = json!([
        {"do":"state.set","key":"items","value":{"literal":["changed"]}},
        {"do":"state.add","key":"item_count","amount":i64::MAX}
    ]);
    fs::write(rules_file, rules.to_string()).unwrap();
    let failed = run(temp.path(), &["--load"]);
    assert!(!failed.status.success());
    assert!(failed.stdout.is_empty());
    assert!(String::from_utf8_lossy(&failed.stderr).contains("overflow"));
    assert_eq!(fs::read(&slot).unwrap(), bytes);
    assert_eq!(fs::read(&backup).unwrap(), bytes);
}
#[test]
fn collections_are_not_mapped_into_yarn_or_text_bindings() {
    let temp = collections_fixture();
    fs::write(temp.path().join("story/main.yarn"),"title: Start\n---\n<<declare $items = 0>>\n<<checkpoint collections.start>>\nItems {$items}\n===\n").unwrap();
    let failed = run(temp.path(), &[]);
    assert!(!failed.status.success());
    let error = String::from_utf8_lossy(&failed.stderr);
    assert!(error.contains("items") && error.contains("list"), "{error}");
    let temp = collections_fixture();
    let path = temp.path().join("story/screen.jsonc");
    let mut screen: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    screen["root"]["children"][2]["bind"] = json!({"state":"items"});
    fs::write(path, screen.to_string()).unwrap();
    let failed = run(temp.path(), &[]);
    assert!(!failed.status.success());
    assert!(String::from_utf8_lossy(&failed.stderr).contains("scalar"));
}

fn scripts_fixture() -> tempfile::TempDir {
    let example = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/scripts-demo");
    let temp = tempfile::tempdir().unwrap();
    fs::create_dir(temp.path().join("story")).unwrap();
    for file in [
        "manifest.jsonc",
        "main.yarn",
        "events.jsonc",
        "screen.jsonc",
        "mechanics.rhai",
    ] {
        fs::copy(example.join(file), temp.path().join("story").join(file)).unwrap();
    }
    temp
}
#[test]
fn scripts_run_once_through_shared_commands_and_checkpoint_candidates() {
    let temp = scripts_fixture();
    let trace = success(run(temp.path(), &[]));
    let lines: Vec<_> = trace
        .iter()
        .filter(|v| v["event"] == "line")
        .map(|v| v["text"].as_str().unwrap())
        .collect();
    assert_eq!(lines, ["Packed 2 items", "Items after reset 0"]);
    let saved = success(run(temp.path(), &["--save-at", "scripts.start"]));
    assert_eq!(
        saved.last().unwrap()["state"]["items"],
        json!(["key", "ticket"])
    );
    assert_eq!(saved.last().unwrap()["state"]["schedule"]["energy"], 8);
    let slot = temp.path().join("saves/save.json");
    let bytes = fs::read(&slot).unwrap();
    success(run(temp.path(), &["--load", "--save-at", "scripts.start"]));
    assert_eq!(fs::read(&slot).unwrap(), bytes);
    let script = temp.path().join("story/mechanics.rhai");
    let source = fs::read_to_string(&script).unwrap().replace(
        "state.emit(\"kit.packed\", true);",
        "state.add(\"item_count\", 9223372036854775807);",
    );
    fs::write(script, source).unwrap();
    let failed = run(temp.path(), &["--load"]);
    assert!(!failed.status.success());
    assert!(failed.stdout.is_empty());
    let error = String::from_utf8_lossy(&failed.stderr);
    assert!(
        error.contains("mechanics.rhai:") && error.contains("overflow"),
        "{error}"
    );
    assert_eq!(fs::read(&slot).unwrap(), bytes);
    assert_eq!(fs::read(temp.path().join("saves/save.bak")).unwrap(), bytes);
}
#[test]
fn script_compile_errors_and_permission_failures_are_source_diagnostics() {
    for source in [
        "fn pack(state, amount) {",
        "let hidden=1; fn pack(state,amount) {}",
        "fn wrong(state,amount) {}",
    ] {
        let temp = scripts_fixture();
        fs::write(temp.path().join("story/mechanics.rhai"), source).unwrap();
        let failed = run(temp.path(), &[]);
        assert!(!failed.status.success());
        assert!(String::from_utf8_lossy(&failed.stderr).contains("mechanics.rhai"));
    }
    let temp = scripts_fixture();
    fs::write(
        temp.path().join("story/mechanics.rhai"),
        "fn pack(state,amount) {\n state.get(\"status\");\n}",
    )
    .unwrap();
    let failed = run(temp.path(), &[]);
    let error = String::from_utf8_lossy(&failed.stderr);
    assert!(!failed.status.success());
    assert!(
        error.contains("mechanics.rhai:2") && error.contains("permission denied"),
        "{error}"
    );
}
#[test]
fn script_feedback_shares_work_and_operation_limits() {
    let temp = scripts_fixture();
    fs::write(
        temp.path().join("story/mechanics.rhai"),
        "fn pack(state,amount) { state.add(\"item_count\",1); }",
    )
    .unwrap();
    let path = temp.path().join("story/events.jsonc");
    let mut rules: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    rules["events"] = json!([{"id":"cycle","trigger":"state.changed.item_count","actions":[{"do":"command.call","command":"pack","args":[2]}]}]);
    fs::write(path, rules.to_string()).unwrap();
    let failed = run(temp.path(), &[]);
    assert!(!failed.status.success());
    let error = String::from_utf8_lossy(&failed.stderr);
    assert!(
        error.contains("budget") || error.contains("operations"),
        "{error}"
    );
}

fn random_fixture() -> tempfile::TempDir {
    let example = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/random-demo");
    let temp = tempfile::tempdir().unwrap();
    fs::create_dir(temp.path().join("story")).unwrap();
    for file in [
        "manifest.jsonc",
        "main.yarn",
        "events.jsonc",
        "screen.jsonc",
        "roll.rhai",
    ] {
        fs::copy(example.join(file), temp.path().join("story").join(file)).unwrap();
    }
    temp
}
#[test]
fn seeded_random_sequence_continues_identically_after_each_checkpoint_load() {
    let temp = random_fixture();
    let uninterrupted = success(run(temp.path(), &[]));
    let end = uninterrupted.last().unwrap();
    assert_eq!(end["state"]["rolls"], json!([2, 1, 2]));
    assert_eq!(end["state"]["rng"], -2691343689449507777i64);
    for checkpoint in ["random.start", "random.second", "random.third"] {
        let saved = success(run(temp.path(), &["--save-at", checkpoint]));
        let slot = temp.path().join("saves/save.json");
        let bytes = fs::read(&slot).unwrap();
        for _ in 0..3 {
            let resaved = success(run(temp.path(), &["--load", "--save-at", checkpoint]));
            assert_eq!(
                resaved.last().unwrap()["state"],
                saved.last().unwrap()["state"]
            );
            assert_eq!(fs::read(&slot).unwrap(), bytes);
            let restored = success(run(temp.path(), &["--load"]));
            assert_eq!(restored.last().unwrap()["state"], end["state"]);
        }
    }
}
#[test]
fn failed_random_replay_never_publishes_candidate_output_or_changes_save_files() {
    let temp = random_fixture();
    success(run(temp.path(), &["--save-at", "random.start"]));
    success(run(temp.path(), &["--load", "--save-at", "random.start"]));
    let slot = temp.path().join("saves/save.json");
    let backup = temp.path().join("saves/save.bak");
    let bytes = fs::read(&slot).unwrap();
    let prior_backup = fs::read(&backup).unwrap();
    let manifest = temp.path().join("story/manifest.jsonc");
    let mut document: Value = serde_json::from_slice(&fs::read(&manifest).unwrap()).unwrap();
    // Retain schema 1 deliberately: fresh launch succeeds, but the old checkpoint replay fails.
    document["state"]["rng"]["default"] = json!(1);
    fs::write(manifest, document.to_string()).unwrap();
    let script = temp.path().join("story/roll.rhai");
    let source = fs::read_to_string(&script).unwrap().replace(
        "let rolls =",
        "if state.get(\"rng\") == -7046029254386353131 { state.get(\"denied\"); }\n    let rolls =",
    );
    fs::write(script, source).unwrap();
    let fresh = success(run(temp.path(), &[]));
    assert_eq!(fresh.last().unwrap()["state"]["rolls"], json!([6, 2, 1]));
    let failed = run(temp.path(), &["--load"]);
    assert!(!failed.status.success());
    assert!(failed.stdout.is_empty());
    assert!(String::from_utf8_lossy(&failed.stderr).contains("roll.rhai:3"));
    assert_eq!(fs::read(slot).unwrap(), bytes);
    assert_eq!(fs::read(backup).unwrap(), prior_backup);
}

fn queries_fixture() -> tempfile::TempDir {
    let example = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/queries-demo");
    let temp = tempfile::tempdir().unwrap();
    fs::create_dir(temp.path().join("story")).unwrap();
    for file in [
        "manifest.jsonc",
        "main.yarn",
        "events.jsonc",
        "screen.jsonc",
    ] {
        fs::copy(example.join(file), temp.path().join("story").join(file)).unwrap();
    }
    temp
}
#[test]
fn collection_queries_replay_without_derived_scalar_state_or_save_changes() {
    let temp = queries_fixture();
    let fresh = success(run(temp.path(), &[]));
    let lines: Vec<_> = fresh
        .iter()
        .filter(|v| v["event"] == "line")
        .map(|v| v["text"].as_str().unwrap())
        .collect();
    assert_eq!(
        lines,
        [
            "Packed 2 items",
            "Now 2, key true, schedule 3, morning true",
            "Items after reset 0"
        ]
    );
    assert!(fresh.last().unwrap()["state"].get("item_count").is_none());
    for checkpoint in ["queries.start", "queries.next"] {
        let saved = success(run(temp.path(), &["--save-at", checkpoint]));
        let slot = temp.path().join("saves/save.json");
        let bytes = fs::read(&slot).unwrap();
        let restored = success(run(temp.path(), &["--load", "--save-at", checkpoint]));
        assert_eq!(
            saved.last().unwrap()["state"],
            restored.last().unwrap()["state"]
        );
        assert_eq!(fs::read(slot).unwrap(), bytes);
        assert_eq!(
            success(run(temp.path(), &["--load"])).last().unwrap()["state"],
            fresh.last().unwrap()["state"]
        );
    }
}
#[test]
fn collection_query_capabilities_literal_targets_and_scopes_validate_before_activation() {
    for variant in [
        "capability",
        "missing",
        "type",
        "scope",
        "dynamic",
        "dynamic_scope",
    ] {
        let temp = queries_fixture();
        let root = temp.path().join("story");
        let manifest = root.join("manifest.jsonc");
        let mut m: Value = serde_json::from_slice(&fs::read(&manifest).unwrap()).unwrap();
        let source = fs::read_to_string(root.join("main.yarn")).unwrap();
        match variant {
            "capability" => m["requires"]["capabilities"]
                .as_array_mut()
                .unwrap()
                .retain(|v| v != "dialogue.collections/1"),
            "scope" => m["state"]["items"]["scope"] = json!("session"),
            "missing" => fs::write(
                root.join("main.yarn"),
                source.replace("state_list_len(\"items\")", "state_list_len(\"missing\")"),
            )
            .unwrap(),
            "type" => fs::write(
                root.join("main.yarn"),
                source.replace("state_list_len(\"items\")", "state_list_len(\"status\")"),
            )
            .unwrap(),
            "dynamic_scope" => {
                m["state"]["temporary"] = json!({"scope":"session","default":[]});
                fs::write(
                    root.join("main.yarn"),
                    source
                        .replace(
                            "<<checkpoint queries.start>>",
                            "<<declare $target = \"temporary\">>\n<<checkpoint queries.start>>",
                        )
                        .replace("state_list_len(\"items\")", "state_list_len($target)"),
                )
                .unwrap();
            }
            "dynamic" => fs::write(
                root.join("main.yarn"),
                source
                    .replace(
                        "<<checkpoint queries.start>>",
                        "<<declare $target = \"items\">>\n<<checkpoint queries.start>>",
                    )
                    .replace("state_list_len(\"items\")", "state_list_len($target)"),
            )
            .unwrap(),
            _ => unreachable!(),
        }
        fs::write(&manifest, m.to_string()).unwrap();
        let checked = Command::new(env!("CARGO_BIN_EXE_weftora"))
            .arg("check")
            .arg(&root)
            .output()
            .unwrap();
        if variant == "dynamic_scope" {
            assert!(
                checked.status.success(),
                "{}",
                String::from_utf8_lossy(&checked.stderr)
            );
            let failed = run(temp.path(), &[]);
            assert!(!failed.status.success());
            assert!(failed.stdout.is_empty());
            assert!(String::from_utf8_lossy(&failed.stderr).contains("Persistent"));
        } else if variant == "dynamic" {
            assert!(
                checked.status.success(),
                "{}",
                String::from_utf8_lossy(&checked.stderr)
            );
            success(run(temp.path(), &[]));
        } else {
            assert!(!checked.status.success(), "{variant}");
            let error = String::from_utf8_lossy(&checked.stderr);
            assert!(
                error.contains(if variant == "scope" {
                    "Persistent"
                } else if variant == "capability" {
                    "dialogue.collections/1"
                } else {
                    "main.yarn"
                }),
                "{variant}: {error}"
            );
        }
    }
}
#[test]
fn removed_collection_query_during_load_preserves_slot_and_publishes_no_output() {
    let temp = queries_fixture();
    success(run(temp.path(), &["--save-at", "queries.start"]));
    success(run(temp.path(), &["--load", "--save-at", "queries.start"]));
    let slot = temp.path().join("saves/save.json");
    let bytes = fs::read(&slot).unwrap();
    let backup = temp.path().join("saves/save.bak");
    let prior_backup = fs::read(&backup).unwrap();
    let rules = temp.path().join("story/events.jsonc");
    let mut document: Value = serde_json::from_slice(&fs::read(&rules).unwrap()).unwrap();
    document["commands"]
        .as_array_mut()
        .unwrap()
        .push(json!({"id":"break_queries","actions":[{"do":"state.remove","key":"items"}]}));
    fs::write(rules, document.to_string()).unwrap();
    // Deliberately retain schema 1: a fresh launch is valid, but an old node-entry value selects a failing replay.
    let manifest = temp.path().join("story/manifest.jsonc");
    let mut metadata: Value = serde_json::from_slice(&fs::read(&manifest).unwrap()).unwrap();
    metadata["state"]["status"]["default"] = json!("Fresh");
    fs::write(manifest, metadata.to_string()).unwrap();
    let yarn = temp.path().join("story/main.yarn");
    let source=fs::read_to_string(&yarn).unwrap().replace("<<checkpoint queries.start>>","<<declare $status = \"Fresh\">>\n<<checkpoint queries.start>>").replace("<<pack>>","<<if $status == \"Empty kit\">>\n<<pack>>\n<<break_queries>>\n<<else>>\n<<pack>>\n<<endif>>");
    fs::write(yarn, source).unwrap();
    let fresh = success(run(temp.path(), &[]));
    assert_eq!(
        fresh.iter().find(|v| v["event"] == "line").unwrap()["text"],
        "Packed 2 items"
    );
    let failed = run(temp.path(), &["--load"]);
    assert!(!failed.status.success());
    assert!(failed.stdout.is_empty());
    let error = String::from_utf8_lossy(&failed.stderr);
    assert!(
        error.contains("main.yarn") && error.contains("state_list_len") && error.contains("items"),
        "{error}"
    );
    assert_eq!(fs::read(slot).unwrap(), bytes);
    assert_eq!(fs::read(backup).unwrap(), prior_backup);
}
