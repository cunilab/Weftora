use weftora_api::{StateError, Value};
use weftora_core::{
    Scope, State, StateDefinition,
    signal::{Signal, SignalBus, SignalError},
};
use weftora_flow::{
    CompareOp, Condition, Operand,
    events::{Action, ActionError, Event, FlowError, MAX_ACTIONS, MAX_EVENTS, Scheduler, Trace},
};

fn signal(id: &str) -> Signal {
    Signal {
        id: id.into(),
        value: Value::Bool(true),
    }
}
fn event(id: &str, trigger: &str, priority: i32, actions: Vec<Action>) -> Event {
    Event {
        id: id.into(),
        trigger: trigger.into(),
        priority,
        condition: None,
        actions,
    }
}
fn set(key: &str, value: Value) -> Action {
    Action::Set {
        key: key.into(),
        value,
    }
}
fn add(amount: i64) -> Action {
    Action::Add {
        key: "count".into(),
        amount: Value::Int(amount),
    }
}
fn condition(op: CompareOp, count: i64) -> Condition {
    Condition::Compare {
        op,
        left: Operand::State("count".into()),
        right: Operand::Literal(Value::Int(count)),
    }
}
fn state() -> State {
    let mut state = State::new();
    state.set("count", Value::Int(0)).unwrap();
    state.set("flag", Value::Bool(false)).unwrap();
    state.drain_changes();
    state
}

fn render(trace: &[Trace]) -> String {
    trace
        .iter()
        .map(|entry| match entry {
            Trace::Signal(signal) => format!("signal {} {:?}\n", signal.id, signal.value),
            Trace::Event { id, matched } => format!("event {id} {matched}\n"),
            Trace::Action { event, index } => format!("action {event} {index}\n"),
        })
        .collect()
}

#[test]
fn priorities_live_conditions_and_fifo_have_a_repeatable_golden_trace() {
    let mut low = event("low", "start", i32::MIN, vec![add(10)]);
    low.condition = Some(condition(CompareOp::Ge, 3));
    let first = event(
        "first",
        "start",
        5,
        vec![
            set("count", Value::Int(1)),
            Action::Emit(signal("nested.a")),
        ],
    );
    let mut second = event(
        "second",
        "start",
        5,
        vec![add(2), Action::Emit(signal("nested.b"))],
    );
    second.condition = Some(condition(CompareOp::Eq, 1));
    let mut skipped = event("skipped", "start", 0, vec![set("flag", Value::Bool(false))]);
    skipped.condition = Some(condition(CompareOp::Lt, 0));
    let events = vec![
        low,
        first,
        second,
        skipped,
        event(
            "high",
            "start",
            i32::MAX,
            vec![set("flag", Value::Bool(true))],
        ),
        event(
            "a",
            "nested.a",
            0,
            vec![add(100), Action::Emit(signal("finish"))],
        ),
        event("b", "nested.b", 0, vec![add(1000)]),
        event("finish", "finish", 0, vec![add(10000)]),
    ];
    for _ in 0..10 {
        let mut state = state();
        let scheduler = Scheduler::new(events.clone(), &state).unwrap();
        let trace = scheduler
            .dispatch(signal("start"), &mut state, 100)
            .unwrap();
        let mut actual = render(&trace);
        actual.push_str(&format!("state {:?}\n", state.snapshot(Scope::Persistent)));
        assert_eq!(actual, include_str!("fixtures/events.txt"));
    }
}

#[test]
fn nested_signal_order_matches_the_core_bus() {
    let mut state = state();
    let scheduler = Scheduler::new(
        vec![
            event(
                "start",
                "start",
                0,
                vec![Action::Emit(signal("a")), Action::Emit(signal("b"))],
            ),
            event("a", "a", 0, vec![Action::Emit(signal("end"))]),
        ],
        &state,
    )
    .unwrap();
    let actual: Vec<_> = scheduler
        .dispatch(signal("start"), &mut state, 100)
        .unwrap()
        .into_iter()
        .filter_map(|entry| match entry {
            Trace::Signal(signal) => Some(signal),
            _ => None,
        })
        .collect();
    let mut bus = SignalBus::default();
    bus.subscribe("start", |_, _| Ok(vec![signal("a"), signal("b")]))
        .unwrap();
    bus.subscribe("a", |_, _| Ok(vec![signal("end")])).unwrap();
    assert_eq!(actual, bus.emit(signal("start"), &mut state, 100).unwrap());
}

#[test]
fn actions_reuse_state_semantics_and_preserve_notification_order() {
    let mut state = state();
    state
        .declare(
            "ratio",
            StateDefinition {
                default: Value::Float(0.5),
                scope: Scope::Settings,
            },
        )
        .unwrap();
    state.set("count", Value::Int(20)).unwrap();
    let scheduler = Scheduler::new(
        vec![
            event(
                "actions",
                "start",
                0,
                vec![
                    set("count", Value::Int(30)),
                    add(2),
                    Action::Remove {
                        key: "count".into(),
                    },
                    Action::Reset {
                        key: "count".into(),
                    },
                    Action::Add {
                        key: "ratio".into(),
                        amount: Value::Int(2),
                    },
                    Action::Add {
                        key: "ratio".into(),
                        amount: Value::Float(0.5),
                    },
                ],
            ),
            event(
                "not.automatic",
                "state.changed",
                0,
                vec![set("flag", Value::Bool(true))],
            ),
        ],
        &state,
    )
    .unwrap();
    scheduler
        .dispatch(signal("start"), &mut state, 100)
        .unwrap();
    assert_eq!(state.get("count"), Ok(&Value::Int(0)));
    assert_eq!(state.get("ratio"), Ok(&Value::Float(3.0)));
    assert_eq!(state.get("flag"), Ok(&Value::Bool(false)));
    let changes = state.drain_changes();
    let count: Vec<_> = changes
        .iter()
        .filter(|c| c.key == "count")
        .map(|c| (c.before.clone(), c.after.clone()))
        .collect();
    assert_eq!(
        count,
        vec![
            (Some(Value::Int(0)), Some(Value::Int(20))),
            (Some(Value::Int(20)), Some(Value::Int(30))),
            (Some(Value::Int(30)), Some(Value::Int(32))),
            (Some(Value::Int(32)), None),
            (None, Some(Value::Int(0))),
        ]
    );
    assert!(
        changes
            .iter()
            .filter(|c| c.key == "ratio")
            .all(|c| c.scope == Scope::Settings)
    );
}

#[test]
fn runtime_failure_retains_writes_but_discards_actions_events_and_queued_signals() {
    let mut state = state();
    let scheduler = Scheduler::new(
        vec![
            event(
                "failing",
                "start",
                1,
                vec![
                    set("count", Value::Int(i64::MAX)),
                    Action::Emit(signal("pending")),
                    add(1),
                    set("flag", Value::Bool(true)),
                ],
            ),
            event("later", "start", 0, vec![set("flag", Value::Bool(true))]),
            event(
                "pending",
                "pending",
                0,
                vec![set("flag", Value::Bool(true))],
            ),
        ],
        &state,
    )
    .unwrap();
    let error = scheduler
        .dispatch(signal("start"), &mut state, 100)
        .unwrap_err();
    assert_eq!(
        error.error,
        FlowError::Action {
            event: "failing".into(),
            index: 2,
            source: ActionError::State(StateError::Overflow("count".into()))
        }
    );
    assert_eq!(
        error.trace,
        vec![
            Trace::Signal(signal("start")),
            Trace::Event {
                id: "failing".into(),
                matched: true
            },
            Trace::Action {
                event: "failing".into(),
                index: 0
            },
            Trace::Action {
                event: "failing".into(),
                index: 1
            }
        ]
    );
    assert_eq!(state.get("count"), Ok(&Value::Int(i64::MAX)));
    assert_eq!(state.get("flag"), Ok(&Value::Bool(false)));
    assert_eq!(state.drain_changes().len(), 1);
    assert_eq!(
        scheduler.dispatch(signal("idle"), &mut state, 1),
        Ok(vec![Trace::Signal(signal("idle"))])
    );
    assert_eq!(state.get("flag"), Ok(&Value::Bool(false)));
    scheduler
        .dispatch(signal("pending"), &mut state, 10)
        .unwrap();
    assert_eq!(state.get("flag"), Ok(&Value::Bool(true)));
}

#[test]
fn removal_can_fail_a_later_condition_or_action_without_rollback() {
    for failing_condition in [false, true] {
        let mut state = state();
        let mut later = event("later", "start", 0, vec![add(1)]);
        if failing_condition {
            later.condition = Some(condition(CompareOp::Eq, 0));
        }
        let scheduler = Scheduler::new(
            vec![
                event(
                    "remove",
                    "start",
                    1,
                    vec![Action::Remove {
                        key: "count".into(),
                    }],
                ),
                later,
            ],
            &state,
        )
        .unwrap();
        let error = scheduler
            .dispatch(signal("start"), &mut state, 100)
            .unwrap_err();
        assert!(!state.exists("count"));
        if failing_condition {
            assert!(matches!(error.error, FlowError::Condition { event, .. } if event == "later"));
        } else {
            assert!(
                matches!(error.error, FlowError::Action { event, index: 0, .. } if event == "later")
            );
        }
    }
}

#[test]
fn invalid_definitions_fail_before_dispatch_and_schema_drift_cannot_write() {
    let state = state();
    let invalid_actions = vec![
        set("unknown", Value::Int(1)),
        set("count", Value::Bool(true)),
        set("count", Value::Float(f64::NAN)),
        Action::Add {
            key: "flag".into(),
            amount: Value::Int(1),
        },
        Action::Add {
            key: "count".into(),
            amount: Value::Float(1.0),
        },
        Action::Remove {
            key: "unknown".into(),
        },
        Action::Reset {
            key: "unknown".into(),
        },
        Action::Emit(signal("BAD")),
        Action::Emit(Signal {
            id: "valid".into(),
            value: Value::Float(f64::INFINITY),
        }),
    ];
    for action in invalid_actions {
        let mut definition = event("invalid", "unused", 0, vec![action]);
        definition.condition = Some(condition(CompareOp::Lt, 0));
        assert!(
            matches!(Scheduler::new(vec![definition], &state), Err(FlowError::Action { event, index: 0, .. }) if event == "invalid")
        );
    }
    for (id, trigger) in [("BAD", "start"), ("valid", "BAD")] {
        assert!(matches!(
            Scheduler::new(vec![event(id, trigger, 0, vec![])], &state),
            Err(FlowError::State(_))
        ));
    }
    let duplicate = event("duplicate", "start", 0, vec![]);
    assert!(
        matches!(Scheduler::new(vec![duplicate.clone(), duplicate], &state), Err(FlowError::Duplicate(id)) if id == "duplicate")
    );
    let mut bad_condition = event("bad.condition", "start", 0, vec![]);
    bad_condition.condition = Some(Condition::And(vec![]));
    assert!(matches!(
        Scheduler::new(vec![bad_condition], &state),
        Err(FlowError::Condition { .. })
    ));
    let scheduler = Scheduler::new(
        vec![
            event("write", "start", 1, vec![set("flag", Value::Bool(true))]),
            event("other", "unrelated", 0, vec![add(1)]),
        ],
        &state,
    )
    .unwrap();
    for count in [None, Some(Value::Bool(false))] {
        let mut other = State::new();
        other.set("flag", Value::Bool(false)).unwrap();
        if let Some(count) = count {
            other.set("count", count).unwrap();
        }
        let before = other.clone();
        let error = scheduler
            .dispatch(signal("start"), &mut other, 100)
            .unwrap_err();
        assert!(error.trace.is_empty());
        assert_eq!(other, before);
    }
}

#[test]
fn definition_and_dispatch_limits_have_exact_boundaries() {
    let mut state = state();
    let make_events = |count| {
        (0..count)
            .map(|i| event(&format!("event.e{i}"), "start", 0, vec![]))
            .collect()
    };
    let scheduler = Scheduler::new(make_events(MAX_EVENTS), &state).unwrap();
    assert!(
        scheduler
            .dispatch(signal("start"), &mut state, MAX_EVENTS + 1)
            .is_ok()
    );
    assert!(matches!(
        scheduler
            .dispatch(signal("start"), &mut state, MAX_EVENTS)
            .unwrap_err()
            .error,
        FlowError::Budget { .. }
    ));
    assert!(matches!(
        Scheduler::new(make_events(MAX_EVENTS + 1), &state),
        Err(FlowError::Events)
    ));
    let scheduler = Scheduler::new(
        vec![event("actions", "start", 0, vec![add(0); MAX_ACTIONS])],
        &state,
    )
    .unwrap();
    assert!(
        scheduler
            .dispatch(signal("start"), &mut state, MAX_ACTIONS + 2)
            .is_ok()
    );
    assert!(
        matches!(scheduler.dispatch(signal("start"), &mut state, MAX_ACTIONS + 1).unwrap_err().error, FlowError::Budget { action: Some(index), .. } if index == MAX_ACTIONS - 1)
    );
    assert!(matches!(
        Scheduler::new(
            vec![event("too.many", "start", 0, vec![add(0); MAX_ACTIONS + 1])],
            &state
        ),
        Err(FlowError::Actions(_))
    ));
    let scheduler = Scheduler::new(
        vec![event(
            "emit",
            "start",
            0,
            vec![Action::Emit(signal("orphan"))],
        )],
        &state,
    )
    .unwrap();
    for limit in 0..4 {
        let error = scheduler
            .dispatch(signal("start"), &mut state, limit)
            .unwrap_err();
        assert!(matches!(error.error, FlowError::Budget { limit: actual, .. } if actual == limit));
        assert!(error.trace.len() <= limit);
    }
    assert_eq!(
        scheduler
            .dispatch(signal("start"), &mut state, 4)
            .unwrap()
            .last(),
        Some(&Trace::Signal(signal("orphan")))
    );
    assert!(
        scheduler
            .dispatch(signal("unhandled"), &mut state, 1)
            .is_ok()
    );
    let error = scheduler
        .dispatch(signal("BAD"), &mut state, 100)
        .unwrap_err();
    assert!(matches!(
        error.error,
        FlowError::Signal(SignalError::InvalidId(_))
    ));
    assert!(error.trace.is_empty());
    let error = scheduler
        .dispatch(
            Signal {
                id: "start".into(),
                value: Value::Float(f64::NAN),
            },
            &mut state,
            100,
        )
        .unwrap_err();
    assert!(matches!(
        error.error,
        FlowError::Signal(SignalError::NonFinite)
    ));
    let mut skipped = event("skipped", "start", 0, vec![add(1)]);
    skipped.condition = Some(condition(CompareOp::Lt, 0));
    let scheduler = Scheduler::new(vec![skipped], &state).unwrap();
    assert!(matches!(
        scheduler
            .dispatch(signal("start"), &mut state, 1)
            .unwrap_err()
            .error,
        FlowError::Budget { action: None, .. }
    ));
    assert_eq!(
        scheduler
            .dispatch(signal("start"), &mut state, 2)
            .unwrap()
            .last(),
        Some(&Trace::Event {
            id: "skipped".into(),
            matched: false
        })
    );
}

#[test]
fn cycles_stop_and_repeated_dispatches_do_not_keep_pending_work() {
    let mut state = state();
    let scheduler = Scheduler::new(
        vec![event(
            "cycle",
            "start",
            0,
            vec![add(1), Action::Emit(signal("start"))],
        )],
        &state,
    )
    .unwrap();
    for _ in 0..2 {
        let before = state.get("count").unwrap().clone();
        let error = scheduler
            .dispatch(signal("start"), &mut state, 12)
            .unwrap_err();
        assert_eq!(
            error.error,
            FlowError::Budget {
                limit: 12,
                event: Some("cycle".into()),
                action: Some(1)
            }
        );
        let Value::Int(before) = before else {
            panic!("not int")
        };
        assert_eq!(state.get("count"), Ok(&Value::Int(before + 3)));
        assert_eq!(render(&error.trace), include_str!("fixtures/cycle.txt"));
        assert_eq!(
            error
                .trace
                .iter()
                .filter(|e| matches!(e, Trace::Signal(_)))
                .count(),
            3
        );
    }
    let mut candidate = state.clone();
    let before = state.clone();
    scheduler
        .dispatch(signal("start"), &mut candidate, 12)
        .unwrap_err();
    assert_eq!(state, before);
    assert_ne!(candidate, state);
}

#[test]
fn addition_checks_types_without_rejecting_defaults_that_would_overflow() {
    let mut state = state();
    state
        .declare(
            "huge",
            StateDefinition {
                default: Value::Int(i64::MAX),
                scope: Scope::Persistent,
            },
        )
        .unwrap();
    state
        .declare(
            "float",
            StateDefinition {
                default: Value::Float(f64::MAX),
                scope: Scope::Persistent,
            },
        )
        .unwrap();
    let scheduler = Scheduler::new(
        vec![event(
            "add",
            "start",
            0,
            vec![
                Action::Add {
                    key: "huge".into(),
                    amount: Value::Int(1),
                },
                Action::Add {
                    key: "float".into(),
                    amount: Value::Float(f64::MAX),
                },
            ],
        )],
        &state,
    )
    .unwrap();
    state.set("huge", Value::Int(0)).unwrap();
    state.set("float", Value::Float(0.0)).unwrap();
    scheduler
        .dispatch(signal("start"), &mut state, 100)
        .unwrap();
    assert_eq!(state.get("huge"), Ok(&Value::Int(1)));
    assert_eq!(state.get("float"), Ok(&Value::Float(f64::MAX)));
    assert!(matches!(
        scheduler
            .dispatch(signal("start"), &mut state, 100)
            .unwrap_err()
            .error,
        FlowError::Action {
            index: 1,
            source: ActionError::State(StateError::NonFinite),
            ..
        }
    ));
}

#[test]
fn direct_sequences_defer_signals_and_identify_root_failures() {
    use weftora_flow::events::Budget;
    let mut state = state();
    let scheduler =
        Scheduler::new(vec![event("observer", "done", 0, vec![add(10)])], &state).unwrap();
    let trace = scheduler
        .execute_actions(
            "award",
            &[add(1), Action::Emit(signal("done")), add(2)],
            &mut state,
            &mut Budget::new(20),
        )
        .unwrap();
    assert_eq!(state.get("count"), Ok(&Value::Int(13)));
    assert!(matches!(&trace[2], Trace::Action { event, index: 2 } if event == "award"));
    let error = scheduler
        .execute_actions(
            "award",
            &[add(1), add(i64::MAX)],
            &mut state,
            &mut Budget::new(20),
        )
        .unwrap_err();
    assert!(
        matches!(error.error, FlowError::Sequence(error) if matches!(*error, FlowError::Action { index: 1, .. }))
    );
    assert_eq!(state.get("count"), Ok(&Value::Int(14)));
    let error = scheduler
        .execute_actions(
            "award",
            &[Action::Emit(signal("done"))],
            &mut state,
            &mut Budget::new(3),
        )
        .unwrap_err();
    assert!(matches!(error.error, FlowError::Budget { event: Some(id), .. } if id == "observer"));
}

#[test]
fn external_commands_share_work_and_append_signals_after_root_actions() {
    use weftora_api::Diagnostic;
    use weftora_flow::events::{Budget, CommandExecutor};
    struct Executor;
    impl CommandExecutor for Executor {
        fn execute(
            &mut self,
            command: &str,
            args: &[Value],
            state: &mut State,
            budget: &mut Budget,
        ) -> Result<Vec<Signal>, Diagnostic> {
            assert_eq!(command, "external");
            assert_eq!(args, &[Value::Int(2)]);
            budget
                .consume(1)
                .map_err(|e| Diagnostic::new("test", None, e))?;
            state.add("count", &args[0]).unwrap();
            Ok(vec![signal("done")])
        }
    }
    let mut state = state();
    let scheduler =
        Scheduler::new(vec![event("finish", "done", 0, vec![add(10)])], &state).unwrap();
    let actions = [
        Action::Call {
            command: "external".into(),
            args: vec![Value::Int(2)],
        },
        add(1),
    ];
    assert!(
        scheduler
            .execute_actions("root", &actions, &mut state, &mut Budget::new(10))
            .is_err()
    );
    assert_eq!(state.get("count"), Ok(&Value::Int(0)));
    let mut budget = Budget::new(7);
    let trace = scheduler
        .execute_with("root", &actions, &mut state, &mut budget, &mut Executor)
        .unwrap();
    assert_eq!(state.get("count"), Ok(&Value::Int(13)));
    assert_eq!(budget.remaining(), 0);
    assert_eq!(
        trace,
        vec![
            Trace::Action {
                event: "root".into(),
                index: 0
            },
            Trace::Action {
                event: "root".into(),
                index: 1
            },
            Trace::Signal(signal("done")),
            Trace::Event {
                id: "finish".into(),
                matched: true
            },
            Trace::Action {
                event: "finish".into(),
                index: 0
            },
        ]
    );
}
