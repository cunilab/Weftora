use super::*;
use std::process::{Command, Stdio};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use yarnspinner_core::prelude::{Instruction, OpCode, Operand, OperandValue};
use yarnspinner_runtime::resource_limits::{MAX_OPTIONS, MAX_STACK, MAX_STRING_BYTES};

fn instruction(op: OpCode, operands: Vec<Operand>) -> Instruction {
    Instruction {
        opcode: op as i32,
        operands,
    }
}
fn text(value: &str) -> Operand {
    Operand {
        value: Some(OperandValue::StringValue(value.into())),
    }
}
fn number(value: f32) -> Operand {
    Operand {
        value: Some(OperandValue::FloatValue(value)),
    }
}
fn line() -> Instruction {
    instruction(
        OpCode::RunLine,
        vec![text("line:budget-first"), number(0.0)],
    )
}

fn compiled(case: &str, boundary: bool) -> CompiledDialogue {
    let mut fixture =
        CompiledDialogue::from_artifact_bytes(include_bytes!("../tests/fixtures/budget.json"))
            .unwrap();
    let node = fixture.program.nodes.get_mut("Start").unwrap();
    node.instructions.clear();
    node.labels.clear();
    node.labels.insert("loop".into(), 1);
    let push = |n| instruction(OpCode::PushFloat, vec![number(n)]);
    node.instructions = match case {
        "stack" => (0..MAX_STACK + usize::from(!boundary))
            .flat_map(|_| [push(1.0), line()])
            .collect(),
        "options" => (0..MAX_OPTIONS + usize::from(!boundary))
            .flat_map(|_| {
                [
                    instruction(
                        OpCode::AddOption,
                        vec![
                            text("line:budget-first"),
                            text("loop"),
                            number(0.0),
                            Operand {
                                value: Some(OperandValue::BoolValue(false)),
                            },
                        ],
                    ),
                    line(),
                ]
            })
            .chain(std::iter::once(instruction(OpCode::ShowOptions, vec![])))
            .collect(),
        "command" => vec![
            instruction(OpCode::PushString, vec![text(&"x".repeat(1024))]),
            instruction(
                OpCode::RunCommand,
                vec![text(&"{0}".repeat(128)), number(1.0)],
            ),
        ],
        "arity-short" => vec![
            push(2.0),
            instruction(OpCode::CallFunc, vec![text("Number.Add")]),
        ],
        "condition" => vec![instruction(
            OpCode::AddOption,
            vec![
                text("line:budget-first"),
                text("loop"),
                number(0.0),
                Operand {
                    value: Some(OperandValue::BoolValue(true)),
                },
            ],
        )],
        "nested-sub" => {
            fixture
                .strings
                .insert("line:budget-first".into(), "{0}".into());
            vec![
                instruction(OpCode::PushString, vec![text(&"{1}".repeat(200))]),
                instruction(OpCode::PushString, vec![text(&"x".repeat(1024))]),
                instruction(
                    OpCode::RunLine,
                    vec![text("line:budget-first"), number(2.0)],
                ),
            ]
        }
        "panic" => vec![
            instruction(OpCode::PushString, vec![text("Broken")]),
            instruction(OpCode::RunNode, vec![]),
        ],
        "write-bytes" => {
            fixture
                .program
                .initial_values
                .insert("$counter".into(), text(""));
            node.labels.insert("loop".into(), 0);
            vec![
                instruction(
                    OpCode::PushString,
                    vec![text(&"a".repeat(MAX_STRING_BYTES))],
                ),
                instruction(OpCode::StoreVariable, vec![text("$counter")]),
                instruction(OpCode::Pop, vec![]),
                instruction(
                    OpCode::PushString,
                    vec![text(&"b".repeat(MAX_STRING_BYTES))],
                ),
                instruction(OpCode::StoreVariable, vec![text("$counter")]),
                instruction(OpCode::Pop, vec![]),
                instruction(OpCode::JumpTo, vec![text("loop")]),
            ]
        }
        "arity" => vec![
            push(1_000_000.0),
            instruction(OpCode::CallFunc, vec![text("round")]),
        ],
        "substitution" => {
            fixture.strings.insert(
                "line:budget-first".into(),
                "{0}".repeat(if boundary {
                    MAX_STRING_BYTES / 1024
                } else {
                    128
                }),
            );
            vec![
                instruction(OpCode::PushString, vec![text(&"x".repeat(1024))]),
                instruction(
                    OpCode::RunLine,
                    vec![text("line:budget-first"), number(1.0)],
                ),
            ]
        }
        "string" => vec![
            instruction(
                OpCode::PushString,
                vec![text(&"x".repeat(MAX_STRING_BYTES / 2))],
            ),
            instruction(
                OpCode::PushString,
                vec![text(
                    &"y".repeat(MAX_STRING_BYTES / 2 + usize::from(!boundary)),
                )],
            ),
            push(2.0),
            instruction(OpCode::CallFunc, vec![text("String.Add")]),
            line(),
        ],
        "writes" => {
            fixture
                .program
                .initial_values
                .insert("$counter".into(), number(0.0));
            vec![
                push(1.0),
                instruction(OpCode::StoreVariable, vec![text("$counter")]),
                instruction(OpCode::JumpTo, vec![text("loop")]),
            ]
        }
        "events" => {
            node.labels.clear();
            vec![
                instruction(OpCode::PushString, vec![text("Start")]),
                instruction(OpCode::RunNode, vec![]),
            ]
        }
        _ => panic!("unknown case"),
    };
    if !matches!(case, "writes" | "events") {
        node.instructions.push(instruction(OpCode::Stop, vec![]));
    }
    if case == "panic" {
        let mut broken = node.clone();
        broken.name = "Broken".into();
        broken.labels.clear();
        broken.instructions = vec![
            instruction(OpCode::PushString, vec![text("abc")]),
            instruction(OpCode::PushFloat, vec![number(1.0)]),
            instruction(OpCode::CallFunc, vec![text("number")]),
            instruction(OpCode::Stop, vec![]),
        ];
        fixture.program.nodes.insert("Broken".into(), broken);
    }
    let sites = if case == "command" {
        vec![CommandSite {
            file: "dialogue/adversarial.yarn".into(),
            line: Some(1),
            node: "Start".into(),
            command: StoryCommand {
                name: "{0}".repeat(128),
                args: vec![],
            },
        }]
    } else {
        vec![]
    };
    let mut result =
        CompiledDialogue::from_parts(fixture.program, fixture.strings, sites, fixture.input_hash)
            .unwrap();
    result.set_node_file("Start", "dialogue/adversarial.yarn");
    result.set_node_file("Broken", "dialogue/broken.yarn");
    result
}

fn play(case: &str, boundary: bool) -> Result<(), DialogueError> {
    let state = Arc::new(RwLock::new(State::new()));
    let mut runner = DialogueRunner::new(&compiled(case, boundary), state.clone());
    runner.start("Start").unwrap();
    for _ in 0..MAX_STACK + 3 {
        match runner.next_event() {
            Ok(DialogueEvent::End | DialogueEvent::Choices(_)) => return Ok(()),
            Ok(_) => {}
            Err(error) => {
                if case == "write-bytes" {
                    let changes = state.write().unwrap().drain_changes();
                    assert!(changes.len() <= 1025, "{} notifications", changes.len());
                }
                assert!(
                    runner
                        .next_event()
                        .unwrap_err()
                        .to_string()
                        .contains(&error.to_string())
                );
                assert!(
                    runner.start("Start").is_err(),
                    "failed runner must remain terminal"
                );
                return Err(error);
            }
        }
    }
    panic!("bounded fixture did not terminate");
}

#[test]
fn adversarial_child() {
    let Ok(case) = std::env::var("WEFTORA_RESOURCE_CASE") else {
        return;
    };
    let message = play(&case, false).unwrap_err().to_string();
    assert!(
        message.contains(if case == "panic" {
            "dialogue/broken.yarn"
        } else {
            "dialogue/adversarial.yarn"
        }),
        "{message}"
    );
    assert!(
        message.contains(if case == "panic" {
            "node 'Broken'"
        } else {
            "node 'Start'"
        }),
        "{message}"
    );
    let expected = match case.as_str() {
        "arity" => "requires 1 parameters",
        "arity-short" => "requires 2 parameters",
        "condition" => "insufficient values",
        "nested-sub" => "string budget",
        "write-bytes" => "state string write budget",
        "panic" => "Yarn runtime panicked",
        "writes" => "state write budget",
        "substitution" | "command" => "string",
        other => other,
    };
    assert!(message.contains(expected), "{message}");
}

#[test]
fn adversarial_cases_fail_in_bounded_child_processes() {
    for case in [
        "stack",
        "options",
        "arity",
        "substitution",
        "string",
        "writes",
        "events",
        "arity-short",
        "condition",
        "nested-sub",
        "write-bytes",
        "panic",
        "command",
    ] {
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "resource_tests::adversarial_child",
                "--nocapture",
            ])
            .env("WEFTORA_RESOURCE_CASE", case)
            .stdin(Stdio::null())
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            if let Some(status) = child.try_wait().unwrap() {
                assert!(status.success(), "case {case}: {status}");
                break;
            }
            if Instant::now() >= deadline {
                child.kill().unwrap();
                child.wait().unwrap();
                panic!("case {case} exceeded 15 seconds");
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

#[test]
fn supported_resource_boundaries_remain_playable() {
    for case in ["stack", "options", "substitution", "string"] {
        play(case, true).unwrap();
    }
}

#[test]
fn fallible_function_handler_stops_before_returning_wrong_types() {
    let mut compiled = compiled("arity", false);
    compiled
        .program
        .nodes
        .get_mut("Start")
        .unwrap()
        .instructions = vec![
        instruction(OpCode::PushFloat, vec![number(1.0)]),
        instruction(OpCode::PushFloat, vec![number(1.0)]),
        instruction(OpCode::CallFunc, vec![text("round")]),
        instruction(OpCode::Stop, vec![]),
    ];
    for bad_type in [false, true] {
        let state = Arc::new(RwLock::new(weftora_core::State::new()));
        let mut dialogue = Dialogue::new(
            Box::new(StateStorage::new(state)),
            Box::new(StringTableTextProvider::new()),
        );
        dialogue.replace_program(compiled.program.clone());
        dialogue.set_node("Start").unwrap();
        let error = dialogue
            .continue_with_function_handler(|_, _| {
                if bad_type {
                    Ok(Some(YarnValue::Boolean(false)))
                } else {
                    Err("host query failed".into())
                }
            })
            .unwrap_err();
        assert!(
            matches!(error, yarnspinner_runtime::prelude::DialogueError::InvalidExecution {ref node_name,..} if node_name=="Start")
        );
        assert!(error.to_string().contains(if bad_type {
            "wrong type"
        } else {
            "host query failed"
        }));
        assert!(!dialogue.can_continue());
    }
}

#[test]
fn query_signatures_cannot_silently_run_without_the_host_handler() {
    let mut compiled = compiled("arity", false);
    compiled
        .program
        .nodes
        .get_mut("Start")
        .unwrap()
        .instructions = vec![
        instruction(OpCode::PushString, vec![text("items")]),
        instruction(OpCode::PushFloat, vec![number(1.0)]),
        instruction(OpCode::CallFunc, vec![text("state_list_len")]),
        instruction(OpCode::Stop, vec![]),
    ];
    let state = Arc::new(RwLock::new(weftora_core::State::new()));
    state
        .write()
        .unwrap()
        .set("items", weftora_api::Value::List(vec![]))
        .unwrap();
    let mut runner = DialogueRunner::new(&compiled, state);
    runner.start("Start").unwrap();
    let error = runner
        .runtime_call(|dialogue| dialogue.continue_())
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("requires the Weftora host handler"),
        "{error}"
    );
    assert!(runner.next_event().is_err());
}
#[test]
fn crafted_query_operands_fail_before_later_writes() {
    for count in [1.0, 2.0] {
        let mut compiled = compiled("arity", false);
        compiled
            .program
            .initial_values
            .insert("$after".into(), number(0.0));
        compiled
            .program
            .nodes
            .get_mut("Start")
            .unwrap()
            .instructions = vec![
            instruction(OpCode::PushFloat, vec![number(1.0)]),
            instruction(OpCode::PushFloat, vec![number(count)]),
            instruction(OpCode::CallFunc, vec![text("state_list_len")]),
            instruction(OpCode::PushFloat, vec![number(9.0)]),
            instruction(OpCode::StoreVariable, vec![text("$after")]),
            instruction(OpCode::Stop, vec![]),
        ];
        let compiled =
            CompiledDialogue::from_artifact_bytes(&compiled.to_artifact_bytes().unwrap()).unwrap();
        let state = Arc::new(RwLock::new(weftora_core::State::new()));
        let mut runner = DialogueRunner::new(&compiled, state.clone());
        runner.start("Start").unwrap();
        let error = runner.next_event().unwrap_err();
        assert!(error.to_string().contains("state_list_len"), "{error}");
        assert_eq!(
            state.read().unwrap().get("after"),
            Ok(&weftora_api::Value::Float(0.0))
        );
    }
}
