use std::cell::RefCell;
use std::rc::Rc;
use std::task::{Context, Poll, Waker};
use weftora_api::{StateError, Value};
use weftora_core::catalog::Catalog;
use weftora_core::command::{CommandError, CommandRegistry, ValueType};
use weftora_core::lifecycle::{Lifecycle, Phase, Service};
use weftora_core::signal::{Signal, SignalBus, SignalError};
use weftora_core::{Scope, State, StateDefinition};

struct TestService {
    name: &'static str,
    live: Rc<RefCell<Vec<&'static str>>>,
    fail: bool,
    fail_cleanup: bool,
}
impl Service for TestService {
    fn name(&self) -> &str {
        self.name
    }
    fn init(&mut self) -> Result<(), String> {
        self.live.borrow_mut().push(self.name);
        if self.fail {
            Err("injected init failure".into())
        } else {
            Ok(())
        }
    }
    fn shutdown(&mut self) -> Result<(), String> {
        assert_eq!(self.live.borrow_mut().pop(), Some(self.name));
        if self.fail_cleanup {
            Err("injected cleanup failure".into())
        } else {
            Ok(())
        }
    }
}
fn lifecycle(live: &Rc<RefCell<Vec<&'static str>>>, fail: bool) -> Lifecycle {
    Lifecycle::new(
        ["state", "commands", "signals"]
            .into_iter()
            .map(|name| {
                Box::new(TestService {
                    name,
                    live: live.clone(),
                    fail: fail && name == "commands",
                    fail_cleanup: fail && name == "commands",
                }) as Box<dyn Service>
            })
            .collect(),
    )
}
#[test]
fn lifecycle_order_failure_cleanup_and_no_restart() {
    let live = Rc::new(RefCell::new(Vec::new()));
    let mut runtime = lifecycle(&live, false);
    assert!(runtime.play().is_err());
    runtime.load().unwrap();
    runtime.play().unwrap();
    runtime.shutdown().unwrap();
    assert!(live.borrow().is_empty());
    assert_eq!(
        runtime.trace(),
        [
            "boot",
            "load",
            "init:state",
            "init:commands",
            "init:signals",
            "ready",
            "play",
            "shutdown:signals",
            "shutdown:commands",
            "shutdown:state",
            "shutdown"
        ]
    );
    runtime.shutdown().unwrap();
    assert!(runtime.load().is_err());
    let mut runtime = lifecycle(&live, true);
    let error = runtime.load().unwrap_err().to_string();
    assert!(error.contains("injected init failure") && error.contains("injected cleanup failure"));
    assert!(live.borrow().is_empty());
    assert_eq!(runtime.phase(), Phase::Shutdown);
    assert_eq!(
        runtime.trace(),
        [
            "boot",
            "load",
            "init:state",
            "init:commands",
            "shutdown:commands",
            "shutdown:state",
            "shutdown"
        ]
    );
    assert!(runtime.play().is_err());
    {
        let mut runtime = lifecycle(&live, false);
        runtime.load().unwrap();
    }
    assert!(live.borrow().is_empty());
}

#[test]
fn state_types_defaults_scopes_and_notifications() {
    let mut state = State::new();
    for (key, value, scope) in [
        ("b", Value::Bool(true), Scope::Session),
        ("i", Value::Int(2), Scope::Persistent),
        ("f", Value::Float(1.5), Scope::Persistent),
        ("s", Value::String("hello".into()), Scope::Settings),
    ] {
        state
            .declare(
                key,
                StateDefinition {
                    default: value.clone(),
                    scope,
                },
            )
            .unwrap();
        assert_eq!(state.get(key), Ok(&value));
        assert_eq!(state.scope(key), Ok(scope));
        assert!(
            state
                .declare(
                    key,
                    StateDefinition {
                        default: value.clone(),
                        scope
                    }
                )
                .is_err()
        );
        for wrong in [
            Value::Bool(false),
            Value::Int(4),
            Value::Float(1.0),
            Value::String("other".into()),
        ] {
            if value.type_name() != wrong.type_name() {
                assert!(matches!(
                    state.set(key, wrong),
                    Err(StateError::TypeMismatch { .. })
                ));
            }
        }
        assert!(state.drain_changes().is_empty());
        state.remove(key).unwrap();
        assert!(!state.exists(key));
        assert!(matches!(state.remove(key), Err(StateError::MissingKey(_))));
        state.reset(key).unwrap();
        let changes = state.drain_changes();
        assert_eq!(changes.len(), 2);
        assert_eq!(changes[0].scope, scope);
        assert_eq!(changes[1].after, Some(value));
    }
    assert_eq!(
        state.snapshot(Scope::Settings).keys().collect::<Vec<_>>(),
        ["s"]
    );
    for f in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(state.set("f", Value::Float(f)), Err(StateError::NonFinite));
    }
    assert!(state.drain_changes().is_empty());
    assert!(state.reset("missing").is_err());
}

#[test]
fn state_numeric_properties_and_atomic_rejections() {
    for initial in -64..64 {
        for delta in -8..8 {
            let mut state = State::new();
            state.set("i", Value::Int(initial)).unwrap();
            state.drain_changes();
            state.add("i", &Value::Int(delta)).unwrap();
            assert_eq!(state.get("i"), Ok(&Value::Int(initial + delta)));
            assert_eq!(state.drain_changes().len(), usize::from(delta != 0));
            state.add("i", &Value::Int(-delta)).unwrap();
            assert_eq!(state.get("i"), Ok(&Value::Int(initial)));
        }
    }
    let mut state = State::new();
    state.set("i", Value::Int(i64::MAX)).unwrap();
    state.drain_changes();
    assert!(state.add("i", &Value::Int(1)).is_err());
    assert_eq!(state.get("i"), Ok(&Value::Int(i64::MAX)));
    assert!(state.drain_changes().is_empty());
}

#[test]
fn commands_validate_arguments_results_failure_and_cancellation() {
    let mut registry = CommandRegistry::default();
    registry
        .register("math.twice", vec![ValueType::Int], ValueType::Int, |args| {
            Box::pin(async move {
                let Value::Int(n) = args[0] else {
                    unreachable!()
                };
                Ok(Value::Int(n * 2))
            })
        })
        .unwrap();
    assert!(matches!(
        registry.start("unknown", vec![]),
        Err(CommandError::Unknown(_))
    ));
    assert!(matches!(
        registry.start("math.twice", vec![]),
        Err(CommandError::Arity { .. })
    ));
    assert!(matches!(
        registry.start("math.twice", vec![Value::Bool(true)]),
        Err(CommandError::Argument { .. })
    ));
    assert!(matches!(
        registry.register("math.twice", vec![], ValueType::Bool, |_| Box::pin(async {
            Ok(Value::Bool(true))
        })),
        Err(CommandError::Duplicate(_))
    ));
    let mut context = Context::from_waker(Waker::noop());
    let mut task = registry.start("math.twice", vec![Value::Int(3)]).unwrap();
    assert_eq!(task.poll(&mut context), Poll::Ready(Ok(Value::Int(6))));
    task.cancel();
    assert_eq!(task.poll(&mut context), Poll::Ready(Ok(Value::Int(6))));
    registry
        .register("bad.result", vec![], ValueType::Float, |_| {
            Box::pin(async { Ok(Value::Float(f64::NAN)) })
        })
        .unwrap();
    assert!(matches!(
        registry
            .start("bad.result", vec![])
            .unwrap()
            .poll(&mut context),
        Poll::Ready(Err(CommandError::Result { .. }))
    ));
    registry
        .register("bad.fail", vec![], ValueType::Bool, |_| {
            Box::pin(async { Err(CommandError::Failed("expected".into())) })
        })
        .unwrap();
    assert!(matches!(
        registry
            .start("bad.fail", vec![])
            .unwrap()
            .poll(&mut context),
        Poll::Ready(Err(CommandError::Failed(_)))
    ));
    struct Pending {
        polls: Rc<RefCell<usize>>,
        drops: Rc<RefCell<usize>>,
    }
    impl std::future::Future for Pending {
        type Output = Result<Value, CommandError>;
        fn poll(self: std::pin::Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
            *self.polls.borrow_mut() += 1;
            Poll::Pending
        }
    }
    impl Drop for Pending {
        fn drop(&mut self) {
            *self.drops.borrow_mut() += 1;
        }
    }
    let polls = Rc::new(RefCell::new(0));
    let drops = Rc::new(RefCell::new(0));
    let p = polls.clone();
    let d = drops.clone();
    registry
        .register("async.wait", vec![], ValueType::Bool, move |_| {
            Box::pin(Pending {
                polls: p.clone(),
                drops: d.clone(),
            })
        })
        .unwrap();
    let mut task = registry.start("async.wait", vec![]).unwrap();
    assert_eq!(task.poll(&mut context), Poll::Pending);
    task.cancel();
    assert_eq!(*drops.borrow(), 1);
    assert_eq!(
        task.poll(&mut context),
        Poll::Ready(Err(CommandError::Cancelled))
    );
    assert_eq!(*polls.borrow(), 1);
}

#[test]
fn signals_are_fifo_unsubscribable_and_bounded() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut bus = SignalBus::default();
    let mut state = State::new();
    let a = calls.clone();
    bus.subscribe("a", move |_, _| {
        a.borrow_mut().push("a1");
        Ok(vec![Signal {
            id: "b".into(),
            value: Value::Bool(true),
        }])
    })
    .unwrap();
    let a = calls.clone();
    let token = bus
        .subscribe("a", move |_, _| {
            a.borrow_mut().push("a2");
            Ok(vec![])
        })
        .unwrap();
    let b = calls.clone();
    bus.subscribe("b", move |_, _| {
        b.borrow_mut().push("b1");
        Ok(vec![])
    })
    .unwrap();
    let trace = bus
        .emit(
            Signal {
                id: "a".into(),
                value: Value::Bool(true),
            },
            &mut state,
            10,
        )
        .unwrap();
    assert_eq!(*calls.borrow(), ["a1", "a2", "b1"]);
    let normalized = serde_json::json!({"calls": *calls.borrow(), "signals": trace});
    assert_eq!(
        serde_json::to_string(&normalized).unwrap(),
        include_str!("fixtures/signals.json").trim()
    );
    assert_eq!(
        trace.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(),
        ["a", "b"]
    );
    assert!(bus.unsubscribe(token));
    assert!(!bus.unsubscribe(token));
    calls.borrow_mut().clear();
    bus.emit(
        Signal {
            id: "a".into(),
            value: Value::Bool(true),
        },
        &mut state,
        10,
    )
    .unwrap();
    assert_eq!(*calls.borrow(), ["a1", "b1"]);
    bus.subscribe("loop", |signal, _| Ok(vec![signal.clone()]))
        .unwrap();
    assert_eq!(
        bus.emit(
            Signal {
                id: "loop".into(),
                value: Value::Bool(true)
            },
            &mut state,
            5
        ),
        Err(SignalError::Budget(5))
    );
    assert!(
        bus.emit(
            Signal {
                id: "a".into(),
                value: Value::Float(f64::NAN)
            },
            &mut state,
            10
        )
        .is_err()
    );
    bus.emit(
        Signal {
            id: "a".into(),
            value: Value::Bool(true),
        },
        &mut state,
        10,
    )
    .unwrap(); // Failed dispatch leaves no queued residue.
}

#[test]
fn catalogs_report_duplicate_and_missing_ids_without_overwrite() {
    let mut catalog = Catalog::default();
    catalog
        .insert("item.apple", 1, "data/items.jsonc", "/0/id")
        .unwrap();
    let error = catalog
        .insert("item.apple", 2, "data/items.jsonc", "/1/id")
        .unwrap_err();
    assert_eq!(error.file, "data/items.jsonc");
    assert!(error.message.contains("/1/id"));
    assert_eq!(
        catalog.get("item.apple", "dialogue/a.yarn", "line 4"),
        Ok(&1)
    );
    let error = catalog
        .get("missing", "dialogue/a.yarn", "line 4")
        .unwrap_err();
    assert!(error.message.contains("missing") && error.message.contains("line 4"));
}

#[test]
fn async_command_completes_after_yielding_and_cancellation_does_not_poll() {
    struct YieldOnce(bool);
    impl std::future::Future for YieldOnce {
        type Output = Result<Value, CommandError>;
        fn poll(mut self: std::pin::Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            if self.0 {
                Poll::Ready(Ok(Value::Bool(true)))
            } else {
                self.0 = true;
                cx.waker().wake_by_ref();
                Poll::Pending
            }
        }
    }
    let mut registry = CommandRegistry::default();
    registry
        .register("async.yield", vec![], ValueType::Bool, |_| {
            Box::pin(YieldOnce(false))
        })
        .unwrap();
    let mut task = registry.start("async.yield", vec![]).unwrap();
    let mut cx = Context::from_waker(Waker::noop());
    assert_eq!(task.poll(&mut cx), Poll::Pending);
    assert_eq!(task.poll(&mut cx), Poll::Ready(Ok(Value::Bool(true))));
    let mut cancelled = registry.start("async.yield", vec![]).unwrap();
    cancelled.cancel();
    assert_eq!(
        cancelled.poll(&mut cx),
        Poll::Ready(Err(CommandError::Cancelled))
    );
    registry
        .register(
            "float.echo",
            vec![ValueType::Float],
            ValueType::Float,
            |args| Box::pin(async move { Ok(args[0].clone()) }),
        )
        .unwrap();
    assert!(matches!(
        registry.start("float.echo", vec![Value::Float(f64::INFINITY)]),
        Err(CommandError::Argument { .. })
    ));
}
