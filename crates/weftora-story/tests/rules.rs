use serde_json::{Value, json};
use std::{fs, path::Path};
use weftora_story::Story;

fn fixture() -> tempfile::TempDir {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/rules-demo");
    let temp = tempfile::tempdir().unwrap();
    for file in ["manifest.jsonc", "main.yarn", "events.jsonc"] {
        fs::copy(root.join(file), temp.path().join(file)).unwrap();
    }
    temp
}
fn update(root: &Path, file: &str, edit: impl FnOnce(&mut Value)) {
    let mut value: Value =
        serde_json::from_str(&fs::read_to_string(root.join(file)).unwrap()).unwrap();
    edit(&mut value);
    fs::write(root.join(file), value.to_string()).unwrap();
}

#[test]
fn jsonc_rules_load_and_feedback_preserves_notifications() {
    let temp = fixture();
    let story = Story::load(temp.path()).unwrap();
    let mut state = story.initial_state().unwrap();
    state.set("input", weftora_api::Value::Bool(true)).unwrap();
    story.rules().settle(&mut state, 0).unwrap();
    assert_eq!(state.get("points"), Ok(&weftora_api::Value::Int(10)));
    assert_eq!(
        state
            .pending_changes()
            .iter()
            .map(|c| c.key.as_str())
            .collect::<Vec<_>>(),
        ["input", "points", "awarded"]
    );
    let before = state.clone();
    let cursor = state.pending_changes().len();
    story.rules().settle(&mut state, cursor).unwrap();
    assert_eq!(state, before);
}

#[test]
fn invalid_rules_have_source_locations_and_future_schema_is_probed_first() {
    for (field, value, expected) in [
        ("trigger", json!("state.changed.unknown"), "unknown"),
        ("id", json!("BAD"), "invalid state key"),
        (
            "actions",
            json!([{"do":"state.set","key":"points","value":true}]),
            "/actions/0",
        ),
        ("actions", json!([{"do":"unknown"}]), "unknown variant"),
        (
            "condition",
            json!({"op":"compare","operator":"==","left":{"state":"points"},"right":{"literal":true}}),
            "/condition",
        ),
        (
            "condition",
            json!({"op":"and","conditions":[]}),
            "/condition",
        ),
    ] {
        let temp = fixture();
        update(temp.path(), "events.jsonc", |v| {
            v["events"][0][field] = value
        });
        let errors = Story::load(temp.path()).unwrap_err();
        assert_eq!(errors[0].file, "events.jsonc");
        assert!(errors[0].message.contains(expected), "{:?}", errors);
    }
    let temp = fixture();
    fs::write(
        temp.path().join("events.jsonc"),
        "{\"rulesSchema\":2,\"futureField\":true}",
    )
    .unwrap();
    assert!(
        Story::load(temp.path()).unwrap_err()[0]
            .message
            .contains("unsupported rulesSchema 2")
    );
    fs::write(
        temp.path().join("events.jsonc"),
        "{\n // comment\n \"rulesSchema\":1,\n \"events\": ,\n}",
    )
    .unwrap();
    assert_eq!(Story::load(temp.path()).unwrap_err()[0].line, Some(4));
}

#[test]
fn sources_capabilities_and_save_scopes_are_checked() {
    for (file, edit, expected) in [
        (
            "manifest.jsonc",
            json!({"events":["events.jsonc","events.jsonc"]}),
            "duplicate rule source",
        ),
        (
            "manifest.jsonc",
            json!({"events":["../escape.jsonc"]}),
            "must not contain",
        ),
        (
            "manifest.jsonc",
            json!({"requires":{"engineApi":">=0.1.0 <0.2.0","capabilities":["save/1"]}}),
            "rules.events/1",
        ),
        (
            "events.jsonc",
            json!({"events":[{"id":"same","trigger":"start","actions":[]},{"id":"same","trigger":"end","actions":[]}]}),
            "duplicate catalog ID",
        ),
        (
            "events.jsonc",
            json!({"events":[{"id":"session","trigger":"state.changed.temporary","actions":[]}]}),
            "Persistent",
        ),
        (
            "events.jsonc",
            json!({"events":[{"id":"all","trigger":"state.changed","actions":[]}]}),
            "keyed",
        ),
        (
            "events.jsonc",
            json!({"events":[{"id":"write.session","trigger":"start","actions":[{"do":"state.set","key":"temporary","value":1}]}]}),
            "Persistent",
        ),
    ] {
        let temp = fixture();
        update(temp.path(), file, |v| {
            for (key, value) in edit.as_object().unwrap() {
                v[key] = value.clone();
            }
        });
        let errors = Story::load(temp.path()).unwrap_err();
        assert!(
            errors.iter().any(|e| e.message.contains(expected)),
            "{:?}",
            errors
        );
    }
}

#[test]
fn limits_and_notification_cycles_are_bounded() {
    let temp = fixture();
    update(
        temp.path(),
        "events.jsonc",
        |v| {
            v["events"][0]["actions"] =
                json!([{"do":"state.set","key":"awarded","value":"x".repeat(16385)}])
        },
    );
    assert!(
        Story::load(temp.path()).unwrap_err()[0]
            .message
            .contains("16384")
    );
    update(temp.path(), "events.jsonc", |v| {
        v["events"][0]["trigger"] = json!("state.changed.points");
        v["events"][0]["condition"] = Value::Null;
        v["events"][0]["actions"] = json!([{"do":"state.add","key":"points","amount":1}]);
    });
    let story = Story::load(temp.path()).unwrap();
    let mut state = story.initial_state().unwrap();
    state.set("points", weftora_api::Value::Int(1)).unwrap();
    let error = story.rules().settle(&mut state, 0).unwrap_err();
    assert_eq!(error.file, "events.jsonc");
    assert!(error.message.contains("budget (1024)"));
    assert!(state.pending_changes().len() <= 1024);
}

#[test]
fn recursive_input_action_counts_and_duplicate_fields_are_rejected() {
    let compare =
        json!({"op":"compare","operator":"==","left":{"state":"input"},"right":{"literal":true}});
    let mut deep = compare.clone();
    for _ in 0..32 {
        deep = json!({"op":"not","condition":deep});
    }
    for (field, value, expected) in [
        ("condition", deep, "depth exceeds 32"),
        (
            "condition",
            json!({"op":"and","conditions":vec![compare;256]}),
            "node count exceeds 256",
        ),
        (
            "actions",
            json!(vec![json!({"do":"state.reset","key":"points"}); 257]),
            "action count exceeds 256",
        ),
    ] {
        let temp = fixture();
        update(temp.path(), "events.jsonc", |v| {
            v["events"][0][field] = value
        });
        assert!(
            Story::load(temp.path()).unwrap_err()[0]
                .message
                .contains(expected)
        );
    }
    let temp = fixture();
    fs::write(
        temp.path().join("events.jsonc"),
        r#"{"rulesSchema":1,"events":[],"events":[]}"#,
    )
    .unwrap();
    assert!(
        Story::load(temp.path()).unwrap_err()[0]
            .message
            .contains("duplicate")
    );
}

#[test]
fn old_notifications_do_not_consume_the_current_boundary_budget() {
    let temp = fixture();
    let story = Story::load(temp.path()).unwrap();
    let mut state = story.initial_state().unwrap();
    for i in 1..4200 {
        state.set("points", weftora_api::Value::Int(i)).unwrap();
    }
    let from = state.pending_changes().len();
    state.set("input", weftora_api::Value::Bool(true)).unwrap();
    story.rules().settle(&mut state, from).unwrap();
    assert_eq!(state.get("points"), Ok(&weftora_api::Value::Int(4209)));
    let stale = state.pending_changes().len();
    state.drain_changes();
    assert!(
        story
            .rules()
            .settle(&mut state, stale)
            .unwrap_err()
            .message
            .contains("drained")
    );
    story.rules().settle(&mut state, 0).unwrap();
}

#[test]
fn explicit_signal_chains_and_reset_use_the_same_save_safe_rules() {
    let temp = fixture();
    update(temp.path(), "events.jsonc", |v| {
        v["events"] = json!([
            {"id":"emit","trigger":"state.changed.input","actions":[{"do":"signal.emit","id":"award.signal","value":true}]},
            {"id":"reset","trigger":"award.signal","actions":[{"do":"state.reset","key":"points"},{"do":"state.add","key":"points","amount":7}]}
        ])
    });
    let story = Story::load(temp.path()).unwrap();
    let mut state = story.initial_state().unwrap();
    state.set("points", weftora_api::Value::Int(99)).unwrap();
    let from = state.pending_changes().len();
    state.set("input", weftora_api::Value::Bool(true)).unwrap();
    story.rules().settle(&mut state, from).unwrap();
    assert_eq!(state.get("points"), Ok(&weftora_api::Value::Int(7)));
}
