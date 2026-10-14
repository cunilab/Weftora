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
        let mut loaded = CompiledDialogue::from_artifact_bytes(&bytes).unwrap();
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

#[test]
fn runtime_failure_keeps_the_jumped_node_file_after_artifact_reload() {
    if std::env::var_os("WEFTORA_SOURCE_GROWTH_CHILD").is_none() {
        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "runtime_failure_keeps_the_jumped_node_file_after_artifact_reload",
                "--nocapture",
            ])
            .env("WEFTORA_SOURCE_GROWTH_CHILD", "1")
            .spawn()
            .unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
        loop {
            if let Some(status) = child.try_wait().unwrap() {
                assert!(status.success(), "{status}");
                return;
            }
            if std::time::Instant::now() >= deadline {
                child.kill().unwrap();
                child.wait().unwrap();
                panic!("source growth fixture exceeded 15 seconds");
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    }
    let sources = vec![
        DialogueSource {
            file: "dialogue/entry.yarn".into(),
            contents: "title: Start\n---\n<<declare $text = \"x\">>\n<<jump Broken>>\n===\n".into(),
        },
        DialogueSource {
            file: "dialogue/broken.yarn".into(),
            contents: format!(
                "title: Broken\n---\n{}\n===\n",
                "<<set $text = $text + $text>>\n".repeat(15)
            ),
        },
    ];
    let compiled = compile(&sources).unwrap();
    let bytes = compiled.to_artifact_bytes().unwrap();
    let mut loaded = CompiledDialogue::from_artifact_bytes(&bytes).unwrap();
    loaded.verify_sources(&sources).unwrap();
    assert_eq!(
        bytes,
        loaded.to_artifact_bytes().unwrap(),
        "provenance must not change format 1"
    );
    for dialogue in [&compiled, &loaded] {
        let mut runner =
            DialogueRunner::new(dialogue, Arc::new(RwLock::new(weftora_core::State::new())));
        runner.start("Start").unwrap();
        let message = runner.next_event().unwrap_err().to_string();
        assert!(message.contains("dialogue/broken.yarn"), "{message}");
        assert!(message.contains("node 'Broken'"), "{message}");
        assert!(message.contains("string budget"), "{message}");
    }
}

#[test]
fn compiler_panic_does_not_blame_the_first_healthy_file() {
    let sources = vec![
        DialogueSource {
            file: "healthy.yarn".into(),
            contents: "title: Healthy\n---\nHello\n===\n".into(),
        },
        DialogueSource {
            file: "broken.yarn".into(),
            contents: "title: Broken\n---\n<<set $x = >>\n===\n".into(),
        },
    ];
    let errors = compile(&sources).unwrap_err();
    assert!(errors.iter().all(|e| e.file == "broken.yarn"), "{errors:?}");
    assert!(errors.iter().any(|e| e.message.contains("node unknown")));
}
