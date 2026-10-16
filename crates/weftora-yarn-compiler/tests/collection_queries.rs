use std::sync::{Arc, RwLock};
use weftora_api::Value;
use weftora_core::State;
use weftora_story::DialogueSource;
use weftora_yarn::{CompiledDialogue, DialogueEvent, DialogueRunner};
use weftora_yarn_compiler::compile;
fn compiled(body: &str) -> CompiledDialogue {
    compile(&[DialogueSource {
        file: "query.yarn".into(),
        contents: format!("title: Start\n---\n{body}\n===\n"),
    }])
    .unwrap()
}
#[test]
fn compiled_and_reloaded_queries_read_live_state_and_dynamic_arguments() {
    let dialogue = compiled("Count {state_list_len(\"items\")}");
    let dialogue =
        CompiledDialogue::from_artifact_bytes(&dialogue.to_artifact_bytes().unwrap()).unwrap();
    let mut state = State::new();
    state
        .set("items", Value::List(vec![Value::String("key".into())]))
        .unwrap();
    let state = Arc::new(RwLock::new(state));
    let mut runner = DialogueRunner::new(&dialogue, state.clone());
    runner.start("Start").unwrap();
    assert!(matches!(runner.next_event().unwrap(),DialogueEvent::Line{text,..} if text=="Count 1"));
    state
        .write()
        .unwrap()
        .set("items", Value::List(vec![]))
        .unwrap();
    runner.start("Start").unwrap();
    assert!(matches!(runner.next_event().unwrap(),DialogueEvent::Line{text,..} if text=="Count 0"));
    for needle in ["\"key\"", "true", "1", "0.5"] {
        let dialogue = compiled(&format!(
            "<<declare $target = \"items\">>\nContains {{state_list_contains($target, {needle})}}"
        ));
        let mut state = State::new();
        state
            .set(
                "items",
                Value::List(vec![
                    Value::String("key".into()),
                    Value::Bool(true),
                    Value::Int(1),
                    Value::Float(0.5),
                ]),
            )
            .unwrap();
        let mut runner = DialogueRunner::new(&dialogue, Arc::new(RwLock::new(state)));
        runner.start("Start").unwrap();
        assert!(
            matches!(runner.next_event().unwrap(),DialogueEvent::Line{text,..} if text=="Contains true")
        );
    }
}
#[test]
fn query_failure_stops_before_later_writes_and_clears_batched_lines() {
    let dialogue = compiled(
        "<<declare $after = 0>>\n<<declare $target = \"missing\">>\n<<set $after = 1>>\nFailure {state_list_len($target)}\n<<set $after = 2>>\nAfter {$after}",
    );
    let state = Arc::new(RwLock::new(State::new()));
    let mut runner = DialogueRunner::new(&dialogue, state.clone());
    runner.start("Start").unwrap();
    let error = runner.next_event().unwrap_err().to_string();
    assert!(
        error.contains("query.yarn") && error.contains("Start") && error.contains("missing"),
        "{error}"
    );
    assert_eq!(state.read().unwrap().get("after"), Ok(&Value::Float(1.0)));
    assert!(runner.next_event().is_err());
    assert!(runner.start("Start").is_err());
}
#[test]
fn query_signatures_reject_wrong_types_arity_and_nonconstant_declarations() {
    for body in [
        "{state_list_len()}",
        "{state_list_len(1)}",
        "{state_map_contains_key(\"map\",true)}",
        "<<declare $x = state_list_len(\"items\")>>",
    ] {
        let result = compile(&[DialogueSource {
            file: "query.yarn".into(),
            contents: format!("title: Start\n---\n{body}\n==="),
        }]);
        let errors = result.unwrap_err();
        assert!(
            errors[0].file == "query.yarn" && errors[0].line.is_some(),
            "{errors:?}"
        );
    }
}

#[test]
fn queries_use_the_same_handler_after_choice_selection() {
    let dialogue = compiled(
        "-> Inspect <<if state_list_contains(\"items\", 1)>>\n    Count {state_list_len(\"items\")}\n",
    );
    let mut state = State::new();
    state
        .set("items", Value::List(vec![Value::Int(1)]))
        .unwrap();
    let mut runner = DialogueRunner::new(&dialogue, Arc::new(RwLock::new(state)));
    runner.start("Start").unwrap();
    assert!(matches!(
        runner.next_event().unwrap(),
        DialogueEvent::Choices(_)
    ));
    runner.choose(0).unwrap();
    assert!(matches!(runner.next_event().unwrap(),DialogueEvent::Line{text,..} if text=="Count 1"));
}
