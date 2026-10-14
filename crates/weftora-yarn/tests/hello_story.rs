use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use weftora_api::{StoryCommand, Value};
use weftora_core::{SharedState, State};
use weftora_story::{DialogueSource, Story};
use weftora_yarn::{
    Choice, CompiledDialogue, DialogueError, DialogueEvent, DialogueRunner, compile,
};

fn hello_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/hello-story")
}

fn hello() -> (Story, CompiledDialogue) {
    let story = Story::load(&hello_dir()).expect("hello-story loads");
    let compiled = compile(&story.dialogue_sources().unwrap()).expect("hello-story compiles");
    (story, compiled)
}

fn shared(initial: &[(&str, Value)]) -> SharedState {
    let mut state = State::new();
    for (k, v) in initial {
        state.set(k, v.clone()).unwrap();
    }
    Arc::new(RwLock::new(state))
}

fn source(contents: &str) -> Vec<DialogueSource> {
    vec![DialogueSource {
        file: "dialogue/test.yarn".into(),
        contents: contents.into(),
    }]
}

fn cmd(name: &str, arg: &str) -> DialogueEvent {
    DialogueEvent::Command(StoryCommand {
        name: name.into(),
        args: vec![arg.into()],
    })
}

fn guide(text: &str) -> DialogueEvent {
    DialogueEvent::Line {
        character: Some("Guide".into()),
        text: text.into(),
    }
}

fn run_hello(choice: usize) -> (Vec<DialogueEvent>, SharedState) {
    let (story, compiled) = hello();
    let state = Arc::new(RwLock::new(story.initial_state().unwrap()));
    let mut runner = DialogueRunner::new(&compiled, state.clone());
    runner.start(&story.manifest().entrypoint).unwrap();
    let mut trace = Vec::new();
    loop {
        let event = runner.next_event().unwrap();
        let done = event == DialogueEvent::End;
        if matches!(event, DialogueEvent::Choices(_)) {
            trace.push(event);
            runner.choose(choice).unwrap();
            continue;
        }
        trace.push(event);
        if done {
            break;
        }
    }
    assert_eq!(runner.next_event().unwrap(), DialogueEvent::End);
    (trace, state)
}

fn expected_choices() -> DialogueEvent {
    DialogueEvent::Choices(vec![
        Choice {
            index: 0,
            text: "Yes, show me.".into(),
            available: true,
        },
        Choice {
            index: 1,
            text: "Not right now.".into(),
            available: true,
        },
    ])
}

fn expected_trace(result_line: &str) -> Vec<DialogueEvent> {
    vec![
        cmd("background", "bg.room"),
        cmd("sound", "sfx.chime"),
        guide(
            "Welcome to Weftora. Everything you see comes from a Story folder, not from Rust code.",
        ),
        guide("Want to see a choice change the game state?"),
        expected_choices(),
        guide(result_line),
        guide("That is the whole walking skeleton. Edit dialogue/intro.yarn and run it again."),
        DialogueEvent::End,
    ]
}

#[test]
fn compiles_hello_story() {
    let (_, compiled) = hello();
    assert_eq!(compiled.node_names(), vec!["Result", "Start"]);
    let commands = compiled.commands();
    assert_eq!(
        commands,
        vec![
            StoryCommand {
                name: "background".into(),
                args: vec!["bg.room".into()]
            },
            StoryCommand {
                name: "sound".into(),
                args: vec!["sfx.chime".into()]
            },
        ]
    );
    let site = &compiled.command_sites()[0];
    assert_eq!(site.file, "dialogue/intro.yarn");
    assert_eq!(site.line, Some(4));
}

#[test]
fn yes_branch_sets_curious_true() {
    let (trace, state) = run_hello(0);
    assert_eq!(
        trace,
        expected_trace("You looked closer, so the state key \"curious\" is now true.")
    );
    assert_eq!(state.read().unwrap().get("curious"), Ok(&Value::Bool(true)));
}

#[test]
fn no_branch_leaves_curious_false() {
    let (trace, state) = run_hello(1);
    assert_eq!(
        trace,
        expected_trace("Fair enough. The state key \"curious\" stays false.")
    );
    assert_eq!(
        state.read().unwrap().get("curious"),
        Ok(&Value::Bool(false))
    );
}

#[test]
fn rust_written_state_is_seen_by_yarn() {
    let (_, compiled) = hello();
    let state = shared(&[("curious", Value::Bool(true))]);
    let mut runner = DialogueRunner::new(&compiled, state);
    runner.start("Result").unwrap();
    assert_eq!(
        runner.next_event().unwrap(),
        guide("You looked closer, so the state key \"curious\" is now true.")
    );
}

#[test]
fn declared_default_does_not_overwrite_existing_state() {
    let (_, compiled) = hello();
    let state = shared(&[("curious", Value::Bool(true))]);
    let _runner = DialogueRunner::new(&compiled, state.clone());
    assert_eq!(state.read().unwrap().get("curious"), Ok(&Value::Bool(true)));
}

#[test]
fn compiler_panic_becomes_diagnostic() {
    // Known to panic inside the Yarn parser in yarnspinner 0.9.0.
    let src = source("title: Start\n---\nHello\n<<set $x = >>\n===\n");
    let diags = compile(&src).unwrap_err();
    assert_eq!(diags[0].file, "dialogue/test.yarn");
}

#[test]
fn syntax_error_has_file_and_line() {
    let src = source("title: Start\n---\nHello\n<<if true>>\nInside\n===\n");
    let diags = compile(&src).unwrap_err();
    assert_eq!(diags[0].file, "dialogue/test.yarn");
    assert!(diags[0].line.is_some(), "{diags:?}");
}

#[test]
fn start_missing_node() {
    let (_, compiled) = hello();
    let mut runner = DialogueRunner::new(&compiled, shared(&[]));
    assert!(matches!(
        runner.start("Nope"),
        Err(DialogueError::MissingNode(n)) if n == "Nope"
    ));
}

#[test]
fn large_initial_int_is_rejected_before_start() {
    let compiled = compile(&source(
        "title: Start\n---\n<<declare $big = 0>>\nValue {$big}\n===\n",
    ))
    .unwrap();
    let state = shared(&[("big", Value::Int(20_000_000))]);
    let mut runner = DialogueRunner::new(&compiled, state);
    let err = runner.start("Start").unwrap_err();
    assert!(
        matches!(&err, DialogueError::StatePrecision { variable, .. } if variable == "$big"),
        "{err:?}"
    );
}

#[test]
fn namespaced_and_typed_variables() {
    let compiled = compile(&source(
        "title: Start\n---\n<<declare $world__day = 0>>\n<<declare $world__ratio = 0.5>>\n\
         <<set $world__day = $world__day + 1>>\n<<set $world__ratio = 0.25>>\n\
         <<set $name = \"Ann\">>\n===\n",
    ))
    .unwrap();
    let state = shared(&[
        ("world.day", Value::Int(4)),
        ("world.ratio", Value::Float(1.0)),
    ]);
    let mut runner = DialogueRunner::new(&compiled, state.clone());
    runner.start("Start").unwrap();
    assert_eq!(runner.next_event().unwrap(), DialogueEvent::End);
    let s = state.read().unwrap();
    assert_eq!(s.get("world.day"), Ok(&Value::Int(5)));
    assert_eq!(s.get("world.ratio"), Ok(&Value::Float(0.25)));
    assert_eq!(s.get("name"), Ok(&Value::String("Ann".into())));
}

#[test]
fn non_integral_number_to_int_key_errors() {
    let compiled = compile(&source(
        "title: Start\n---\n<<declare $n = 0>>\n<<set $n = 1.5>>\n===\n",
    ))
    .unwrap();
    let state = shared(&[("n", Value::Int(1))]);
    let mut runner = DialogueRunner::new(&compiled, state.clone());
    runner.start("Start").unwrap();
    assert!(matches!(
        runner.next_event(),
        Err(DialogueError::Variable { .. })
    ));
    assert_eq!(state.read().unwrap().get("n"), Ok(&Value::Int(1)));
}

#[test]
fn invalid_choice_is_rejected() {
    let (_, compiled) = hello();
    let mut runner = DialogueRunner::new(&compiled, shared(&[]));
    runner.start("Start").unwrap();
    while !matches!(runner.next_event().unwrap(), DialogueEvent::Choices(_)) {}
    assert!(matches!(
        runner.choose(7),
        Err(DialogueError::InvalidChoice(7))
    ));
    assert!(runner.next_event().is_err());
    runner.choose(0).unwrap();
}

#[test]
fn initial_state_types_are_checked_before_start() {
    let compiled = compile(&source(
        "title: Start\n---\n<<declare $flag = true>>\n<<declare $name = \"Ann\">>\n<<declare $world__day = 0>>\n<<if $flag>>\nHello {$name}, day {$world__day}\n<<endif>>\n===\n",
    )).unwrap();
    for (key, value, expected) in [
        ("flag", Value::String("not-a-bool".into()), "bool"),
        ("flag", Value::Int(1), "bool"),
        ("name", Value::Bool(true), "string"),
        ("name", Value::Float(1.0), "string"),
        ("world.day", Value::String("one".into()), "number"),
        ("world.day", Value::Bool(true), "number"),
    ] {
        let state = shared(&[(key, value)]);
        let errors = weftora_yarn::check_state(&compiled, &state.read().unwrap());
        assert_eq!(errors.len(), 1);
        assert!(
            matches!(&errors[0], DialogueError::StateTypeMismatch { expected: actual, .. } if *actual == expected)
        );
        let mut runner = DialogueRunner::new(&compiled, state);
        assert!(matches!(
            runner.start("Start"),
            Err(DialogueError::StateTypeMismatch { .. })
        ));
    }
    let state = shared(&[("flag", Value::String("not-a-bool".into()))]);
    assert_eq!(
        weftora_yarn::check_state(&compiled, &state.read().unwrap())[0].to_string(),
        "initialState `flag` is a string but Yarn declares $flag as bool"
    );
    for day in [Value::Int(1), Value::Float(1.5)] {
        let state = shared(&[
            ("flag", Value::Bool(true)),
            ("name", Value::String("Ann".into())),
            ("world.day", day),
        ]);
        assert!(weftora_yarn::check_state(&compiled, &state.read().unwrap()).is_empty());
        let mut runner = DialogueRunner::new(&compiled, state);
        runner.start("Start").unwrap();
        assert!(matches!(
            runner.next_event().unwrap(),
            DialogueEvent::Line { .. }
        ));
    }
}

#[test]
fn integer_precision_boundaries_never_panic() {
    let compiled = compile(&source(
        "title: Start\n---\n<<declare $n = 0>>\nValue {$n}\n===\n",
    ))
    .unwrap();
    for n in [
        i64::MIN,
        i64::MAX,
        -(1 << 24),
        1 << 24,
        -((1 << 24) + 1),
        (1 << 24) + 1,
    ] {
        let mut runner = DialogueRunner::new(&compiled, shared(&[("n", Value::Int(n))]));
        if (-(1 << 24)..=(1 << 24)).contains(&n) {
            runner.start("Start").unwrap();
            assert!(matches!(
                runner.next_event().unwrap(),
                DialogueEvent::Line { .. }
            ));
        } else {
            let error = runner.start("Start").unwrap_err();
            assert!(
                matches!(error, DialogueError::StatePrecision { .. }),
                "{n}: {error:?}"
            );
        }
    }
}

#[test]
fn self_jump_exhausts_budget_and_fails_runner() {
    let compiled = compile(&source("title: Start\n---\n<<jump Start>>\n===\n")).unwrap();
    let mut runner = DialogueRunner::new(&compiled, shared(&[]));
    runner.start("Start").unwrap();
    let error = runner.next_event().unwrap_err();
    assert!(matches!(error, DialogueError::Runtime(_)));
    assert!(
        error
            .to_string()
            .contains("instruction budget (100000) exceeded in node `Start`")
    );
    assert!(matches!(
        runner.next_event(),
        Err(DialogueError::Runtime(_))
    ));
    assert!(matches!(runner.choose(0), Err(DialogueError::Runtime(_))));
    assert!(matches!(
        runner.start("Start"),
        Err(DialogueError::Runtime(_))
    ));
}

#[test]
fn canonical_state_rejects_runtime_type_change_without_breaking_dialogue() {
    let compiled = compile(&source(
        "title: Start\n---\n<<declare $flag = true>>\nFirst\n<<if $flag>>\nSecond\n<<endif>>\n===\n",
    )).unwrap();
    let state = shared(&[("flag", Value::Bool(true))]);
    let mut runner = DialogueRunner::new(&compiled, state.clone());
    runner.start("Start").unwrap();
    assert!(matches!(
        runner.next_event().unwrap(),
        DialogueEvent::Line { .. }
    ));
    assert!(matches!(
        state
            .write()
            .unwrap()
            .set("flag", Value::String("not-a-bool".into())),
        Err(weftora_api::StateError::TypeMismatch { .. })
    ));
    assert!(
        matches!(runner.next_event().unwrap(), DialogueEvent::Line { text, .. } if text == "Second")
    );
    assert_eq!(runner.next_event().unwrap(), DialogueEvent::End);
}

#[test]
fn command_jump_loop_fails_after_one_thousand_commands() {
    let compiled = compile(&source(
        "title: Start\n---\n<<background bg>>\n<<jump Start>>\n===\n",
    ))
    .unwrap();
    let mut runner = DialogueRunner::new(&compiled, shared(&[]));
    runner.start("Start").unwrap();
    for _ in 0..1_000 {
        assert_eq!(runner.next_event().unwrap(), cmd("background", "bg"));
    }
    let error = runner.next_event().unwrap_err();
    assert!(
        error
            .to_string()
            .contains("consecutive command budget (1000) exceeded in node `Start`")
    );
    assert!(matches!(
        runner.next_event(),
        Err(DialogueError::Runtime(_))
    ));
    assert!(matches!(runner.choose(0), Err(DialogueError::Runtime(_))));
    assert!(matches!(
        runner.start("Start"),
        Err(DialogueError::Runtime(_))
    ));
}

#[test]
fn instruction_budget_is_cumulative_across_commands() {
    // This small node does substantial bounded work before each command. Each
    // call is below the VM budget, but their combined instruction count is not.
    let compiled = compile(&source(
        "title: Start\n---\n<<declare $n = 0>>\n<<set $n = $n + 1>>\n\
         <<if $n >= 100>>\n<<set $n = 0>>\n<<background bg>>\n<<endif>>\n\
         <<jump Start>>\n===\n",
    ))
    .unwrap();
    let mut runner = DialogueRunner::new(&compiled, shared(&[]));
    runner.start("Start").unwrap();
    let mut commands = 0;
    let error = loop {
        match runner.next_event() {
            Ok(DialogueEvent::Command(_)) => {
                commands += 1;
                assert!(commands < 200, "instruction budget reset across commands");
            }
            Err(error) => break error,
            other => panic!("unexpected event: {other:?}"),
        }
    };
    assert!(commands > 1);
    assert!(
        error
            .to_string()
            .contains("instruction budget (100000) exceeded in node `Start`")
    );
    assert!(matches!(
        runner.next_event(),
        Err(DialogueError::Runtime(_))
    ));
}

#[test]
fn blocking_lines_and_choices_reset_progress_budgets() {
    // More than 100,000 instructions and 1,000 commands overall are permitted
    // because each iteration returns a blocking event to the player.
    let body = "<<set $n = $n + 1>>\n".repeat(20);
    for (blocking, choices) in [
        ("Hello\n<<jump Start>>\n", false),
        ("-> Continue\n    <<jump Start>>\n", true),
    ] {
        let compiled = compile(&source(&format!(
            "title: Start\n---\n<<declare $n = 0>>\n{body}<<background bg>>\n{blocking}===\n"
        )))
        .unwrap();
        let mut runner = DialogueRunner::new(&compiled, shared(&[]));
        runner.start("Start").unwrap();
        for _ in 0..1_100 {
            assert_eq!(runner.next_event().unwrap(), cmd("background", "bg"));
            let event = runner.next_event().unwrap();
            if choices {
                assert!(matches!(event, DialogueEvent::Choices(_)));
                runner.choose(0).unwrap();
            } else {
                assert!(matches!(event, DialogueEvent::Line { .. }));
            }
        }
    }
}

#[test]
fn state_precision_check_only_checks_declared_numbers() {
    let compiled = compile(&source(
        "title: Start\n---\n<<declare $world__day = 0>>\nHello\n===\n",
    ))
    .unwrap();
    for n in [i64::MIN, i64::MAX, -16_777_217, 16_777_217] {
        let state = shared(&[
            ("world.day", Value::Int(n)),
            ("unused", Value::Int(i64::MAX)),
        ]);
        let errors = weftora_yarn::check_state(&compiled, &state.read().unwrap());
        assert_eq!(errors.len(), 1);
        assert!(
            matches!(&errors[0], DialogueError::StatePrecision { key, variable, value }
            if key == "world.day" && variable == "$world__day" && *value == n)
        );
    }
    for n in [-16_777_216, 16_777_216] {
        let state = shared(&[
            ("world.day", Value::Int(n)),
            ("unused", Value::Int(i64::MAX)),
        ]);
        assert!(weftora_yarn::check_state(&compiled, &state.read().unwrap()).is_empty());
    }
}

#[test]
fn a_blocking_event_is_allowed_after_exactly_one_thousand_commands() {
    let commands = "<<background bg>>\n".repeat(1_000);
    let compiled = compile(&source(&format!(
        "title: Start\n---\n{commands}Hello\n{commands}===\n"
    )))
    .unwrap();
    let mut runner = DialogueRunner::new(&compiled, shared(&[]));
    runner.start("Start").unwrap();
    for _ in 0..1_000 {
        assert_eq!(runner.next_event().unwrap(), cmd("background", "bg"));
    }
    assert!(matches!(
        runner.next_event().unwrap(),
        DialogueEvent::Line { .. }
    ));
    for _ in 0..1_000 {
        assert_eq!(runner.next_event().unwrap(), cmd("background", "bg"));
    }
    assert_eq!(runner.next_event().unwrap(), DialogueEvent::End);
}

#[test]
fn runtime_panic_is_caught_and_runner_cannot_be_restarted() {
    let compiled = compile(&source(
        "title: Start\n---\n<<declare $flag = true>>\nFirst\n<<if $flag>>\nSecond\n<<endif>>\n===\n",
    )).unwrap();
    let state = shared(&[("flag", Value::Bool(true))]);
    let mut runner = DialogueRunner::new(&compiled, state.clone());
    runner.start("Start").unwrap();
    assert!(matches!(
        runner.next_event().unwrap(),
        DialogueEvent::Line { .. }
    ));
    // Replacing the entire shared state bypasses per-key type preservation; catch the VM panic at this boundary.
    *state.write().unwrap() = shared(&[("flag", Value::String("not-a-bool".into()))])
        .read()
        .unwrap()
        .clone();
    let error = runner.next_event().unwrap_err();
    assert!(matches!(error, DialogueError::Runtime(_)));
    assert!(error.to_string().contains("runtime panicked"));
    *state.write().unwrap() = shared(&[("flag", Value::Bool(true))])
        .read()
        .unwrap()
        .clone();
    assert!(matches!(
        runner.next_event(),
        Err(DialogueError::Runtime(_))
    ));
    assert!(matches!(runner.choose(0), Err(DialogueError::Runtime(_))));
    assert!(matches!(
        runner.start("Start"),
        Err(DialogueError::Runtime(_))
    ));
}

#[test]
fn invalid_default_is_rejected_before_any_canonical_defaults_are_written() {
    let compiled = compile(&source(
        "title: Start\n---\n<<declare $Bad = true>>\n<<declare $good = true>>\nHello.\n===\n",
    ))
    .unwrap();
    let state = shared(&[]);
    assert!(
        weftora_yarn::check_state(&compiled, &state.read().unwrap())
            .iter()
            .any(|error| error.to_string().contains("$Bad"))
    );
    let mut runner = DialogueRunner::new(&compiled, state.clone());
    assert!(
        runner
            .start("Start")
            .unwrap_err()
            .to_string()
            .contains("$Bad")
    );
    assert_eq!(state.read().unwrap().iter().count(), 0);
}
