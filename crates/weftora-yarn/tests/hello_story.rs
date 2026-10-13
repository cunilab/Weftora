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
    let state = Arc::new(RwLock::new(
        State::with_values(&story.manifest().initial_state).unwrap(),
    ));
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
fn large_int_read_from_yarn_errors() {
    let compiled = compile(&source(
        "title: Start\n---\n<<declare $big = 0>>\nValue {$big}\n===\n",
    ))
    .unwrap();
    let state = shared(&[("big", Value::Int(20_000_000))]);
    let mut runner = DialogueRunner::new(&compiled, state);
    runner.start("Start").unwrap();
    let err = runner.next_event().unwrap_err();
    assert!(
        matches!(&err, DialogueError::Variable { variable, .. } if variable == "$big"),
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
