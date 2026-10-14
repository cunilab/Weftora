use std::sync::{Arc, RwLock};
use weftora_story::DialogueSource;
use weftora_yarn::{CompiledDialogue, DialogueEvent, DialogueRunner};
use weftora_yarn_compiler::compile;

#[test]
fn fresh_compilations_and_loaded_artifacts_produce_identical_bytes_and_events() {
    let sources = vec![DialogueSource {
        file: "dialogue/main.yarn".into(),
        contents: "title: Start\n---\n<<declare $flag = false>>\n-> Yes\n    <<set $flag = true>>\n-> No\n    <<set $flag = false>>\nResult {$flag}\n===\n".into(),
    }];
    let original = compile(&sources).unwrap();
    let bytes = original.to_artifact_bytes().unwrap();
    for _ in 0..10 {
        let fresh = compile(&sources).unwrap();
        assert_eq!(bytes, fresh.to_artifact_bytes().unwrap());
        let loaded = CompiledDialogue::from_artifact_bytes(&bytes).unwrap();
        loaded.verify_sources(&sources).unwrap();
        for branch in [0, 1] {
            let play = |compiled: &CompiledDialogue| {
                let state = Arc::new(RwLock::new(weftora_core::State::new()));
                let mut runner = DialogueRunner::new(compiled, state.clone());
                runner.start("Start").unwrap();
                let mut trace = Vec::new();
                loop {
                    let event = runner.next_event().unwrap();
                    if matches!(event, DialogueEvent::Choices(_)) {
                        runner.choose(branch).unwrap();
                    }
                    let ended = event == DialogueEvent::End;
                    trace.push(event);
                    if ended {
                        break;
                    }
                }
                (trace, state.read().unwrap().clone())
            };
            assert_eq!(play(&original), play(&loaded));
        }
    }
}

#[test]
fn compiler_identity_matches_the_locked_dependency() {
    let lock = include_str!("../../../Cargo.lock");
    assert!(lock.contains(&format!(
        "name = \"yarnspinner_compiler\"\nversion = \"{}\"",
        weftora_yarn::COMPILER_VERSION
    )));
}
