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

fn command_fixture(commands: Value) -> tempfile::TempDir {
    let temp = fixture();
    update(temp.path(), "manifest.jsonc", |v| {
        v["requires"]["capabilities"]
            .as_array_mut()
            .unwrap()
            .push(json!("rules.commands/1"));
    });
    update(temp.path(), "events.jsonc", |v| v["commands"] = commands);
    temp
}
fn award_commands() -> Value {
    json!([
        {"id":"award","parameters":[{"name":"amount","type":"int"}],"actions":[
            {"do":"command.call","command":"increment","args":[{"argument":"amount"}]},
            {"do":"state.set","key":"awarded","value":true}
        ]},
        {"id":"increment","parameters":[{"name":"n","type":"int"}],"actions":[
            {"do":"state.add","key":"points","amount":{"argument":"n"}}
        ]}
    ])
}
#[test]
fn typed_commands_share_forward_references_events_and_notification_feedback() {
    use weftora_api::Value as V;
    let temp = command_fixture(award_commands());
    update(temp.path(), "events.jsonc", |v| {
        v["events"][0]["actions"] = json!([{"do":"command.call","command":"award","args":[10]}]);
    });
    let story = Story::load(temp.path()).unwrap();
    let rules = story.rules();
    assert!(rules.has_command("award"));
    assert_eq!(
        rules.parse_command("award", &["3".into()], false).unwrap(),
        [V::Int(3)]
    );
    for text in ["3.5", "1e3", "9223372036854775808", "true"] {
        assert!(rules.parse_command("award", &[text.into()], false).is_err());
    }
    let mut state = story.initial_state().unwrap();
    rules.call("award", &[V::Int(3)], &mut state, 0).unwrap();
    assert_eq!(state.get("points"), Ok(&V::Int(3)));
    state.set("awarded", V::Bool(false)).unwrap();
    let cursor = state.pending_changes().len();
    state.set("input", V::Bool(true)).unwrap();
    rules.settle(&mut state, cursor).unwrap();
    assert_eq!(state.get("points"), Ok(&V::Int(13)));
    let before = state.clone();
    assert!(
        rules
            .call("award", &[V::Bool(true)], &mut state, cursor)
            .is_err()
    );
    assert_eq!(state, before);
}
#[test]
fn command_errors_keep_nested_source_and_completed_writes() {
    use weftora_api::Value as V;
    let temp = command_fixture(award_commands());
    let story = Story::load(temp.path()).unwrap();
    let mut state = story.initial_state().unwrap();
    state.set("points", V::Int(1)).unwrap();
    let cursor = state.pending_changes().len();
    let error = story
        .rules()
        .call("award", &[V::Int(i64::MAX)], &mut state, cursor)
        .unwrap_err();
    assert_eq!(error.file, "events.jsonc");
    assert!(
        error.message.contains("/commands/1/actions/0") && error.message.contains("overflow"),
        "{error:?}"
    );
    assert_eq!(state.get("awarded"), Ok(&V::Bool(false)));
    update(temp.path(), "events.jsonc", |v| {
        v["commands"][0]["actions"]
            .as_array_mut()
            .unwrap()
            .insert(0, json!({"do":"state.set","key":"points","value":5}));
    });
    let story = Story::load(temp.path()).unwrap();
    assert!(
        story
            .rules()
            .call("award", &[V::Int(i64::MAX)], &mut state, cursor)
            .is_err()
    );
    assert_eq!(state.get("points"), Ok(&V::Int(5)));
    assert_eq!(state.get("awarded"), Ok(&V::Bool(false)));
}
#[test]
fn commands_reject_cycles_types_reserved_names_scopes_and_unknown_fields() {
    for (commands, expected) in [
        (
            json!([{"id":"loop","actions":[{"do":"command.call","command":"loop"}]}]),
            "recursive",
        ),
        (json!([{"id":"background","actions":[]}]), "reserved"),
        (
            json!([{"id":"bad","actions":[{"do":"command.call","command":"absent"}]}]),
            "unknown command",
        ),
        (
            json!([{"id":"bad","actions":[{"do":"state.set","key":"temporary","value":1}]}]),
            "Persistent",
        ),
        (
            json!([{"id":"bad","parameters":[{"name":"a","type":"bool"}],"actions":[{"do":"state.add","key":"points","amount":{"argument":"a"}}]}]),
            "expected int",
        ),
        (
            json!([{"id":"bad","actions":[{"do":"state.set","key":"points","value":{"argument":"missing"}}]}]),
            "unknown argument",
        ),
        (
            json!([{"id":"bad","parameters":[{"name":"a","type":"int"}],"actions":[{"do":"state.set","key":"points","value":{"argument":"a","ignored":true}}]}]),
            "collection action values require",
        ),
    ] {
        let temp = command_fixture(commands);
        let errors = Story::load(temp.path()).unwrap_err();
        assert!(errors[0].message.contains(expected), "{errors:?}");
    }
    let temp = command_fixture(award_commands());
    update(temp.path(), "events.jsonc", |v| {
        v["commands"][0]["actions"][0]["args"] = json!([true]);
    });
    assert!(
        Story::load(temp.path()).unwrap_err()[0]
            .message
            .contains("expected Int")
    );
    update(temp.path(), "manifest.jsonc", |v| {
        v["requires"]["capabilities"]
            .as_array_mut()
            .unwrap()
            .retain(|c| c != "rules.commands/1");
    });
    assert!(
        Story::load(temp.path()).unwrap_err()[0]
            .message
            .contains("rules.commands/1")
    );
}

#[test]
fn command_and_feedback_share_work_budget_even_for_no_op_cycles() {
    let temp = command_fixture(
        json!([{"id":"start","actions":[{"do":"signal.emit","id":"again","value":true}]}]),
    );
    update(temp.path(), "events.jsonc", |v| {
        v["events"] = json!([{"id":"cycle","trigger":"again","actions":[{"do":"command.call","command":"start"}]}]);
    });
    let story = Story::load(temp.path()).unwrap();
    let mut state = story.initial_state().unwrap();
    let error = story.rules().call("start", &[], &mut state, 0).unwrap_err();
    assert!(
        error.message.contains("budget") && error.message.contains("/commands/0/actions/0"),
        "{error:?}"
    );
}

#[test]
fn command_expansion_and_parameter_limits_are_bounded_before_execution() {
    let primitives = vec![json!({"do":"state.add","key":"points","amount":1}); 256];
    let commands = vec![
        json!({"id":"leaf","actions":primitives}),
        json!({"id":"root","actions":[{"do":"command.call","command":"leaf"},{"do":"command.call","command":"leaf"}]}),
    ];
    let temp = command_fixture(json!(commands));
    assert!(
        Story::load(temp.path()).unwrap_err()[0]
            .message
            .contains("expanded action count")
    );
    let commands: Vec<_> = (0..17).map(|index| json!({"id":format!("chain.n{index}"),"actions":if index == 16 {json!([])} else {json!([{"do":"command.call","command":format!("chain.n{}",index+1)}])}})).collect();
    let temp = command_fixture(json!(commands));
    assert!(
        Story::load(temp.path()).unwrap_err()[0]
            .message
            .contains("depth")
    );
    let parameters: Vec<_> = (0..17)
        .map(|i| json!({"name":format!("p.n{i}"),"type":"int"}))
        .collect();
    let temp = command_fixture(json!([{"id":"large","parameters":parameters,"actions":[]}]));
    assert!(
        Story::load(temp.path()).unwrap_err()[0]
            .message
            .contains("parameter count")
    );
    let calls = vec![json!({"do":"command.call","command":"empty"}); 256];
    let temp = command_fixture(json!([
        {"id":"root","actions":[{"do":"command.call","command":"wide"},{"do":"command.call","command":"wide"},{"do":"command.call","command":"wide"},{"do":"command.call","command":"wide"}]},
        {"id":"wide","actions":calls},{"id":"empty","actions":[]}
    ]));
    assert!(
        Story::load(temp.path()).unwrap_err()[0]
            .message
            .contains("expansion work")
    );
}
#[test]
fn typed_text_calls_handle_empty_quoted_strings_finite_floats_and_bool() {
    use weftora_api::Value as V;
    let temp = command_fixture(
        json!([{"id":"typed","parameters":[{"name":"flag","type":"bool"},{"name":"ratio","type":"float"},{"name":"text","type":"string"}],"actions":[]}]),
    );
    let story = Story::load(temp.path()).unwrap();
    assert_eq!(
        story
            .rules()
            .parse_command("typed", &["true".into(), "1.25".into(), "".into()], false)
            .unwrap(),
        [V::Bool(true), V::Float(1.25), V::String("".into())]
    );
    for float in ["NaN", "inf", "1e400"] {
        assert!(
            story
                .rules()
                .parse_command(
                    "typed",
                    &["true".into(), float.into(), "text".into()],
                    false
                )
                .is_err()
        );
    }
    for args in [
        vec![V::Bool(true)],
        vec![V::Bool(true), V::Int(1), V::String("text".into())],
        vec![V::Bool(true), V::Float(1.0), V::String("x".repeat(16385))],
    ] {
        assert!(story.rules().validate_command("typed", &args).is_err());
    }
}
#[test]
fn screen_command_references_report_widget_locations() {
    let temp = command_fixture(award_commands());
    update(temp.path(), "manifest.jsonc", |v| {
        v["requires"]["capabilities"]
            .as_array_mut()
            .unwrap()
            .push(json!("ui.screens/1"));
        v["screens"] = json!(["screen.jsonc"]);
        v["initialScreen"] = json!("dialogue");
    });
    for (command, args, expected) in [
        ("missing", json!([]), "unknown command"),
        ("award", json!([true]), "expected Int"),
        ("award", json!([]), "expects 1"),
    ] {
        fs::write(temp.path().join("screen.jsonc"), json!({"screenSchema":1,"root":{"type":"screen","id":"dialogue","children":[{"type":"button","id":"call","text":"Call","action":{"do":"command.call","command":command,"args":args}},{"type":"choiceList","id":"choices"}]}}).to_string()).unwrap();
        let errors = Story::load(temp.path()).unwrap_err();
        assert!(
            errors.iter().any(|e| e.file == "screen.jsonc"
                && e.message.contains("/root/children/0/action")
                && e.message.contains(expected)),
            "{errors:?}"
        );
    }
}

#[test]
fn oversized_leaf_identifiers_are_rejected_before_repeated_expansion() {
    for (action, expected) in [
        (
            json!({"do":"state.set","key":"x".repeat(1024 * 1024),"value":true}),
            "512 bytes",
        ),
        (
            json!({"do":"signal.emit","id":"x".repeat(1024 * 1024),"value":true}),
            "1024 bytes",
        ),
    ] {
        let temp = command_fixture(json!([
            {"id":"a","actions":vec![json!({"do":"command.call","command":"leaf"});256]},
            {"id":"leaf","actions":[action]}
        ]));
        let errors = Story::load(temp.path()).unwrap_err();
        assert!(
            errors[0].message.contains("/commands/1/actions/0")
                && errors[0].message.contains(expected),
            "{errors:?}"
        );
    }
}

fn collections_fixture() -> tempfile::TempDir {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/collections-demo");
    let temp = tempfile::tempdir().unwrap();
    for file in [
        "manifest.jsonc",
        "main.yarn",
        "events.jsonc",
        "screen.jsonc",
    ] {
        fs::copy(root.join(file), temp.path().join(file)).unwrap();
    }
    temp
}
#[test]
fn collections_use_explicit_action_literals_and_keep_argument_named_map_keys() {
    use weftora_api::Value as V;
    let temp = collections_fixture();
    let story = Story::load(temp.path()).unwrap();
    let mut state = story.initial_state().unwrap();
    story.rules().call("pack", &[], &mut state, 0).unwrap();
    assert_eq!(
        state.get("items"),
        Ok(&V::List(vec![
            V::String("key".into()),
            V::String("ticket".into())
        ]))
    );
    let V::Map(schedule) = state.get("schedule").unwrap() else {
        panic!("not a map")
    };
    assert_eq!(schedule["argument"], V::String("literal map entry".into()));
    let from = state.pending_changes().len();
    story.rules().call("unpack", &[], &mut state, from).unwrap();
    assert_eq!(state.get("items"), Ok(&V::List(vec![])));
    assert_eq!(state.get("schedule"), Ok(&V::Map(Default::default())));
    for invalid in [
        json!(["key"]),
        json!({"argument":1}),
        json!({"literal":1,"extra":true}),
        json!({"key":"value"}),
    ] {
        update(temp.path(), "events.jsonc", |v| {
            v["commands"][0]["actions"][0]["value"] = invalid
        });
        let errors = Story::load(temp.path()).unwrap_err();
        assert!(
            errors[0]
                .message
                .contains("collection action values require"),
            "{errors:?}"
        );
    }
}
#[test]
fn collection_capability_and_scalar_boundaries_fail_before_activation() {
    let temp = collections_fixture();
    update(temp.path(), "manifest.jsonc", |v| {
        v["requires"]["capabilities"]
            .as_array_mut()
            .unwrap()
            .retain(|c| c != "state.collections/1")
    });
    assert!(
        Story::load(temp.path())
            .unwrap_err()
            .iter()
            .any(|e| e.message.contains("state.collections/1"))
    );
    for (action, expected) in [
        (
            json!({"do":"state.add","key":"items","amount":1}),
            "int or float",
        ),
        (
            json!({"do":"signal.emit","id":"test","value":{"literal":[]}}),
            "scalar",
        ),
        (
            json!({"do":"command.call","command":"unpack","args":[{"literal":[]}]}),
            "expects 0",
        ),
    ] {
        let temp = collections_fixture();
        update(temp.path(), "events.jsonc", |v| {
            v["commands"][0]["actions"] = json!([action])
        });
        let errors = Story::load(temp.path()).unwrap_err();
        assert!(errors[0].message.contains(expected), "{errors:?}");
    }
    let temp = collections_fixture();
    update(
        temp.path(),
        "events.jsonc",
        |v| v["events"] = json!([{"id":"bad","trigger":"start","condition":{"op":"compare","operator":"==","left":{"state":"items"},"right":{"literal":[]}},"actions":[]}]),
    );
    assert!(
        Story::load(temp.path()).unwrap_err()[0]
            .message
            .contains("does not accept list")
    );
    let temp = collections_fixture();
    update(temp.path(), "screen.jsonc", |v| {
        v["root"]["children"][2]["bind"] = json!({"state":"items"})
    });
    let story = Story::load(temp.path()).unwrap();
    assert!(
        story
            .screen("dialogue")
            .unwrap()
            .state_errors(&story.initial_state().unwrap())
            .iter()
            .any(|e| e.message.contains("scalar"))
    );
}
#[test]
fn collection_payloads_are_charged_during_static_command_expansion() {
    let temp = collections_fixture();
    let payload = json!({"literal":["x".repeat(16300)]});
    let calls = vec![json!({"do":"command.call","command":"zleaf"}); 256];
    let mut commands: Vec<_> = (0..9)
        .map(|i| json!({"id":format!("large{i}"),"actions":calls}))
        .collect();
    commands
        .push(json!({"id":"zleaf","actions":[{"do":"state.set","key":"items","value":payload}]}));
    update(temp.path(), "events.jsonc", |v| {
        v["commands"] = commands.into()
    });
    assert!(
        Story::load(temp.path()).unwrap_err()[0]
            .message
            .contains("expanded rule data exceeds 32 MiB")
    );
}

#[test]
fn script_metadata_confines_sources_capabilities_permissions_and_variants() {
    for (field, value, expected) in [
        ("file", json!("../escape.rhai"), "must not contain"),
        ("function", json!("bad-name"), "identifier"),
        ("reads", json!(["missing"]), "does not exist"),
        ("signals", json!(["state.changed.points"]), "impersonate"),
        ("writes", json!(["temporary"]), "Persistent"),
    ] {
        let temp = command_fixture(
            json!([{"id":"scripted","script":{"file":"mechanics.rhai","function":"run"}}]),
        );
        update(temp.path(), "manifest.jsonc", |v| {
            v["requires"]["capabilities"]
                .as_array_mut()
                .unwrap()
                .push(json!("rules.rhai/1"))
        });
        fs::write(temp.path().join("mechanics.rhai"), "fn run(state) {}").unwrap();
        update(temp.path(), "events.jsonc", |v| {
            v["commands"][0]["script"][field] = value
        });
        let errors = Story::load(temp.path()).unwrap_err();
        assert!(errors[0].message.contains(expected), "{errors:?}");
    }
    let temp = command_fixture(
        json!([{"id":"scripted","script":{"file":"mechanics.rhai","function":"run"}}]),
    );
    fs::write(temp.path().join("mechanics.rhai"), "fn run(state) {}").unwrap();
    assert!(
        Story::load(temp.path()).unwrap_err()[0]
            .message
            .contains("rules.rhai/1")
    );
    update(temp.path(), "manifest.jsonc", |v| {
        v["requires"]["capabilities"]
            .as_array_mut()
            .unwrap()
            .push(json!("rules.rhai/1"))
    });
    let story = Story::load(temp.path()).unwrap();
    assert_eq!(story.rules().scripts().len(), 1);
    assert!(
        story
            .rules()
            .call("scripted", &[], &mut story.initial_state().unwrap(), 0)
            .unwrap_err()
            .message
            .contains("no executor")
    );
    update(temp.path(), "events.jsonc", |v| {
        v["commands"][0]["actions"] = json!([])
    });
    assert!(
        Story::load(temp.path()).unwrap_err()[0]
            .message
            .contains("exactly one")
    );
}
