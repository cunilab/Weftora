use std::sync::{Arc, RwLock};
use weftora_api::Value;
use weftora_core::{Scope, State, StateDefinition};
use weftora_story::DialogueSource;
use weftora_yarn::{DialogueEvent, DialogueRunner, SaveRefusal};
use weftora_yarn_compiler::compile;

fn sources(text: &str) -> Vec<DialogueSource> {
    vec![DialogueSource {
        file: "checkpoint.yarn".into(),
        contents: text.into(),
    }]
}
const SCRIPT: &str = "title: Start\n---\n<<declare $n = 0>>\n<<checkpoint scene.start>>\n<<set $n = $n + 1>>\nFirst {$n}\nSecond\n===\n";

#[test]
fn capture_is_before_the_prefix_and_progress_or_persistent_change_disarms_it() {
    let compiled = compile(&sources(SCRIPT)).unwrap();
    assert_eq!(
        compiled.checkpoints().get("scene.start"),
        Some(&"Start".into())
    );
    let state = Arc::new(RwLock::new(State::new()));
    state
        .write()
        .unwrap()
        .declare(
            "volume",
            StateDefinition {
                default: Value::Float(1.0),
                scope: Scope::Settings,
            },
        )
        .unwrap();
    let mut runner = DialogueRunner::new(&compiled, state.clone());
    runner.start("Start").unwrap();
    assert!(matches!(
        runner.save_point(),
        Err(SaveRefusal::NotAtCheckpoint)
    ));
    assert!(matches!(runner.next_event().unwrap(),DialogueEvent::Line{text,..} if text=="First 1"));
    let snapshot = runner.save_point().unwrap();
    assert_eq!(snapshot.state["n"], Some(Value::Float(0.0)));
    assert_eq!(state.read().unwrap().get("n"), Ok(&Value::Float(1.0)));
    state
        .write()
        .unwrap()
        .set("volume", Value::Float(0.5))
        .unwrap();
    assert!(runner.save_point().is_ok());
    state.write().unwrap().set("n", Value::Float(2.0)).unwrap();
    assert!(matches!(runner.save_point(), Err(SaveRefusal::Changed)));
    state.write().unwrap().set("n", Value::Float(1.0)).unwrap();
    assert!(runner.save_point().is_ok());
    runner.next_event().unwrap();
    assert!(matches!(
        runner.save_point(),
        Err(SaveRefusal::NotAtCheckpoint)
    ));
}

#[test]
fn frozen_snapshot_restore_replays_prefix_once_and_resaves_identically() {
    let compiled = compile(&sources(SCRIPT)).unwrap();
    let context = weftora_save::Context {
        story_id: "test.checkpoint".into(),
        story_version: "0.1.0".into(),
        save_schema: 1,
        writer_engine_api: "0.1.0".into(),
        writer_engine_release: "0.0.1".into(),
        checkpoints: compiled.checkpoints().keys().cloned().collect(),
    };
    let state = Arc::new(RwLock::new(State::new()));
    let mut original = DialogueRunner::new(&compiled, state.clone());
    original.start("Start").unwrap();
    let line = original.next_event().unwrap();
    let game = weftora_save::SavedGame::new(&context, original.save_point().unwrap()).unwrap();
    let bytes = game.to_bytes().unwrap();
    let candidate_state = Arc::new(RwLock::new(State::new()));
    let mut candidate = DialogueRunner::new(&compiled, candidate_state.clone());
    let prepared = game
        .prepare_state(
            &context,
            &candidate_state.read().unwrap(),
            &state.read().unwrap(),
        )
        .unwrap();
    *candidate_state.write().unwrap() = prepared;
    candidate.restore_visits(game.visits()).unwrap();
    candidate
        .start(&compiled.checkpoints()[game.checkpoint()])
        .unwrap();
    assert_eq!(candidate.next_event().unwrap(), line);
    assert_eq!(
        candidate_state.read().unwrap().snapshot(Scope::Persistent),
        state.read().unwrap().snapshot(Scope::Persistent)
    );
    let second = weftora_save::SavedGame::new(&context, candidate.save_point().unwrap())
        .unwrap()
        .to_bytes()
        .unwrap();
    assert_eq!(bytes, second);
}

#[test]
fn malformed_duplicate_nonentry_and_random_checkpoints_are_rejected() {
    for script in [
        "title: Start\n---\n<<checkpoint>>\nLine\n===\n",
        "title: Start\n---\nLine\n<<checkpoint scene.start>>\n===\n",
        "title: Start\n---\n<<checkpoint bad__id>>\nLine\n===\n",
        "title: Start\n---\n<<checkpoint scene.same>>\nLine\n===\ntitle: Other\n---\n<<checkpoint scene.same>>\nOther\n===\n",
        "title: Start\n---\n<<checkpoint scene.start>>\n{random()}\n===\n",
    ] {
        assert!(compile(&sources(script)).is_err(), "{script}");
    }
}

#[test]
fn first_blocking_event_in_another_node_cannot_arm_the_point() {
    let compiled=compile(&sources("title: Start\n---\n<<checkpoint scene.start>>\n<<jump Other>>\n===\ntitle: Other\n---\nOther\n===\n")).unwrap();
    let mut runner = DialogueRunner::new(&compiled, Arc::new(RwLock::new(State::new())));
    runner.start("Start").unwrap();
    assert!(matches!(
        runner.next_event().unwrap(),
        DialogueEvent::Line { .. }
    ));
    assert!(matches!(
        runner.save_point(),
        Err(SaveRefusal::NotAtCheckpoint)
    ));
}

#[test]
fn prefix_command_cannot_be_saved_before_its_blocking_display() {
    let compiled = compile(&sources(
        "title: Start\n---\n<<checkpoint scene.start>>\n<<background room>>\nLine\n===\n",
    ))
    .unwrap();
    let mut runner = DialogueRunner::new(&compiled, Arc::new(RwLock::new(State::new())));
    runner.start("Start").unwrap();
    assert!(matches!(
        runner.next_event().unwrap(),
        DialogueEvent::Command(_)
    ));
    assert!(matches!(
        runner.save_point(),
        Err(SaveRefusal::NotAtCheckpoint)
    ));
    assert!(matches!(
        runner.next_event().unwrap(),
        DialogueEvent::Line { .. }
    ));
    assert!(runner.save_point().is_ok());
}

#[test]
fn choices_arm_and_selection_disarms_a_checkpoint() {
    let compiled = compile(&sources(
        "title: Start\n---\n<<checkpoint scene.start>>\n-> Yes\n    Yes\n-> No\n    No\n===\n",
    ))
    .unwrap();
    let mut runner = DialogueRunner::new(&compiled, Arc::new(RwLock::new(State::new())));
    runner.start("Start").unwrap();
    assert!(matches!(
        runner.next_event().unwrap(),
        DialogueEvent::Choices(_)
    ));
    assert!(runner.save_point().is_ok());
    assert!(runner.choose(999).is_err());
    assert!(runner.save_point().is_ok());
    runner.choose(0).unwrap();
    assert!(matches!(
        runner.save_point(),
        Err(SaveRefusal::NotAtCheckpoint)
    ));
}

#[test]
fn visit_counters_are_frozen_and_restored_before_checkpoint_prefix() {
    let compiled=compile(&sources("title: Before\ntracking: always\n---\n<<jump Check>>\n===\ntitle: Check\ntracking: always\n---\n<<checkpoint scene.check>>\nVisits {visited_count(\"Before\")}\n===\n")).unwrap();
    let mut original = DialogueRunner::new(&compiled, Arc::new(RwLock::new(State::new())));
    original.start("Before").unwrap();
    let line = original.next_event().unwrap();
    let snapshot = original.save_point().unwrap();
    assert_eq!(snapshot.visits.get("Before"), Some(&1));
    let mut restored = DialogueRunner::new(&compiled, Arc::new(RwLock::new(State::new())));
    assert!(
        restored
            .restore_visits(&[("Gone".into(), 1)].into())
            .is_err()
    );
    restored.restore_visits(&snapshot.visits).unwrap();
    restored.start("Check").unwrap();
    assert_eq!(restored.next_event().unwrap(), line);
    assert_eq!(restored.save_point().unwrap().visits, snapshot.visits);
    assert!(restored.restore_visits(&snapshot.visits).is_err());
}

#[test]
fn invalid_continue_while_choices_wait_preserves_save_point() {
    let compiled = compile(&sources(
        "title: Start\n---\n<<checkpoint scene.choice>>\n-> Yes\n    Yes\n-> No\n    No\n===\n",
    ))
    .unwrap();
    let mut runner = DialogueRunner::new(&compiled, Arc::new(RwLock::new(State::new())));
    runner.start("Start").unwrap();
    assert!(matches!(
        runner.next_event().unwrap(),
        DialogueEvent::Choices(_)
    ));
    let before = runner.save_point().unwrap();
    assert!(runner.next_event().is_err());
    assert_eq!(runner.save_point().unwrap(), before);
}

#[test]
fn collection_float_bits_participate_in_checkpoint_eligibility() {
    let compiled = compile(&sources(SCRIPT)).unwrap();
    let state = Arc::new(RwLock::new(State::new()));
    state
        .write()
        .unwrap()
        .set("items", Value::List(vec![Value::Float(-0.0)]))
        .unwrap();
    let mut runner = DialogueRunner::new(&compiled, state.clone());
    runner.start("Start").unwrap();
    runner.next_event().unwrap();
    assert!(runner.save_point().is_ok());
    state
        .write()
        .unwrap()
        .set("items", Value::List(vec![Value::Float(0.0)]))
        .unwrap();
    assert!(matches!(runner.save_point(), Err(SaveRefusal::Changed)));
}
#[test]
fn oversized_collection_checkpoint_reports_data_error_without_arming_a_snapshot() {
    let compiled = compile(&sources(SCRIPT)).unwrap();
    let mut initial = State::new();
    let value = Value::List(vec![Value::String(
        "x".repeat(weftora_api::MAX_COLLECTION_BYTES - 8),
    )]);
    for index in 0..513 {
        initial
            .declare(
                &format!("large.n{index}"),
                StateDefinition {
                    default: value.clone(),
                    scope: Scope::Persistent,
                },
            )
            .unwrap();
    }
    let state = Arc::new(RwLock::new(initial));
    let mut runner = DialogueRunner::new(&compiled, state);
    runner.start("Start").unwrap();
    assert!(matches!(
        runner.next_event().unwrap(),
        DialogueEvent::Line { .. }
    ));
    let error = runner.save_point().unwrap_err();
    assert!(
        matches!(&error,SaveRefusal::Data(message) if message.contains("save limit")),
        "{error}"
    );
}

#[test]
fn removed_declarations_count_toward_expected_snapshot_limits() {
    let compiled = compile(&sources(
        "title: Start\n---\n<<checkpoint scene.start>>\n<<background bg.room>>\nFirst\n===\n",
    ))
    .unwrap();
    let state = Arc::new(RwLock::new(State::new()));
    let mut runner = DialogueRunner::new(&compiled, state.clone());
    runner.start("Start").unwrap();
    assert!(matches!(
        runner.next_event().unwrap(),
        DialogueEvent::Command(_)
    ));
    {
        let mut state = state.write().unwrap();
        for index in 0..4097 {
            let key = format!("removed.n{index}");
            state.set(&key, Value::Bool(true)).unwrap();
            state.remove(&key).unwrap();
        }
        state.drain_changes();
    }
    assert!(matches!(
        runner.next_event().unwrap(),
        DialogueEvent::Line { .. }
    ));
    assert!(
        matches!(runner.save_point(),Err(SaveRefusal::Data(message)) if message.contains("save limit"))
    );
}
