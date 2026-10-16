//! Bounded Rhai execution. Sources and permissions are data; engine types stay in this crate.
use rhai::{
    AST, Array, CallFnOptions, Dynamic, Engine, EvalAltResult, ImmutableString, Map, Position,
    Scope,
};
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    rc::Rc,
};
use weftora_api::{Diagnostic, ScriptDefinition, Value};
use weftora_core::{
    Scope as StateScope, State,
    signal::{self, Signal},
};
use weftora_flow::events::{Budget, CommandExecutor};

const FORBIDDEN: &[&str] = &[
    "eval",
    "Fn",
    "call",
    "curry",
    "print",
    "debug",
    "type_of",
    "is_def_fn",
    "is_def_var",
    "is_shared",
    "import",
    "export",
];

type Error = Box<EvalAltResult>;
type Result<T> = std::result::Result<T, Error>;

#[derive(Default)]
pub struct Programs {
    commands: BTreeMap<String, (ScriptDefinition, AST)>,
}
impl Programs {
    pub fn compile(definitions: &[ScriptDefinition]) -> std::result::Result<Self, Vec<Diagnostic>> {
        let mut programs = Self::default();
        let mut bytes = 0usize;
        for definition in definitions {
            bytes = bytes.saturating_add(definition.source.len());
            let fail = |message| vec![Diagnostic::new(&definition.file, None, message)];
            if definitions.len() > 64
                || bytes > 1024 * 1024
                || definition.source.len() > 65536
                || definition.arity > 16
            {
                return Err(fail("scripts exceed source/command/parameter limits"));
            }
            let engine = engine();
            let mut ast = engine.compile(&definition.source).map_err(|e| {
                vec![Diagnostic::new(
                    &definition.file,
                    line(e.position()),
                    e.to_string(),
                )]
            })?;
            let mut forbidden = None;
            ast.walk(&mut |nodes| {
                if let Some(
                    rhai::ASTNode::Expr(
                        rhai::Expr::FnCall(call, pos) | rhai::Expr::MethodCall(call, pos),
                    )
                    | rhai::ASTNode::Stmt(rhai::Stmt::FnCall(call, pos)),
                ) = nodes.last()
                    && FORBIDDEN.contains(&call.name.as_str())
                {
                    forbidden = Some((call.name.to_string(), *pos));
                    return false;
                }
                true
            });
            if let Some((name, position)) = forbidden {
                return Err(vec![Diagnostic::new(
                    &definition.file,
                    line(position),
                    format!("function `{name}` is unavailable in Story scripts"),
                )]);
            }
            if ast.iter_functions().any(|f| f.name.starts_with("anon$")) {
                return Err(fail("anonymous functions are unsupported"));
            }
            if !ast.statements().is_empty() {
                return Err(fail(
                    "scripts may contain only function definitions; top-level statements are unsupported",
                ));
            }
            if !ast
                .iter_functions()
                .any(|f| f.name == definition.function && f.params.len() == definition.arity + 1)
            {
                return Err(fail(
                    "script function is missing or has wrong arity (first parameter must receive the state handle)",
                ));
            }
            ast.set_source(&definition.file);
            if programs
                .commands
                .insert(definition.id.clone(), (definition.clone(), ast))
                .is_some()
            {
                return Err(fail("duplicate script command"));
            }
        }
        Ok(programs)
    }
}
impl CommandExecutor for Programs {
    fn execute(
        &mut self,
        command: &str,
        args: &[Value],
        state: &mut State,
        budget: &mut Budget,
    ) -> std::result::Result<Vec<Signal>, Diagnostic> {
        let (definition, ast) = self.commands.get(command).ok_or_else(|| {
            Diagnostic::new("", None, format!("unknown script command `{command}`"))
        })?;
        let fail = |message: String| Diagnostic::new(&definition.file, None, message);
        if args.len() != definition.arity
            || args
                .iter()
                .any(|v| v.is_collection() || bounded(v).is_err())
        {
            return Err(fail(
                "script arguments must match the declared scalar arity".into(),
            ));
        }
        let remaining = budget.external_operations();
        if remaining == 0 {
            return Err(fail("script operation budget (10000) exceeded".into()));
        }
        let mut engine = engine();
        engine.set_max_operations(remaining);
        let operations = Rc::new(Cell::new(0u64));
        let counter = operations.clone();
        engine.on_progress(move |count| {
            counter.set(count);
            None
        });
        let context = Rc::new(RefCell::new(Context {
            state: std::mem::take(state),
            definition: definition.clone(),
            work: budget.remaining(),
            signals: Vec::new(),
        }));
        let restore = Restore {
            target: state,
            context: context.clone(),
        };
        let mut arguments = vec![Dynamic::from(Host(context.clone()))];
        arguments.extend(args.iter().map(to_dynamic));
        let result = engine.call_fn_with_options::<Dynamic>(
            CallFnOptions::new().eval_ast(false),
            &mut Scope::new(),
            ast,
            &definition.function,
            arguments,
        );
        let mut context = context.borrow_mut();
        let used = budget.remaining() - context.work;
        let charged = budget
            .consume(used)
            .and_then(|()| budget.consume_external_operations(operations.get()));
        let signals = std::mem::take(&mut context.signals);
        drop(context);
        drop(restore);
        charged.map_err(fail)?;
        let value = result.map_err(|error| {
            Diagnostic::new(
                &definition.file,
                error_line(&error),
                format!("script command `{command}`: {error}"),
            )
        })?;
        if !value.is_unit() {
            return Err(fail("script commands must return unit ()".into()));
        }
        Ok(signals)
    }
}

// Shared handles clone cheaply when Rhai passes them to helper functions; State itself never clones.
#[derive(Clone)]
struct Host(Rc<RefCell<Context>>);
struct Context {
    state: State,
    definition: ScriptDefinition,
    work: usize,
    signals: Vec<Signal>,
}
struct Restore<'a> {
    target: &'a mut State,
    context: Rc<RefCell<Context>>,
}
impl Drop for Restore<'_> {
    fn drop(&mut self) {
        *self.target = std::mem::take(&mut self.context.borrow_mut().state);
    }
}
impl Context {
    fn permission(&self, key: &str, write: bool) -> Result<()> {
        let allowed = if write {
            &self.definition.writes
        } else {
            &self.definition.reads
        };
        if !allowed.iter().any(|k| k == key) {
            return Err(error(format!(
                "{} permission denied for `{key}`",
                if write { "write" } else { "read" }
            )));
        }
        let scope = self.state.scope(key).map_err(error)?;
        if self.definition.persistent_only && scope != StateScope::Persistent {
            return Err(error("save-enabled script requires Persistent state"));
        }
        Ok(())
    }
    fn spend(&mut self) -> Result<()> {
        // Reserve a queue unit for each emission until Flow enqueues it after the command.
        if self.work <= self.signals.len() {
            return Err(error("command work budget exceeded"));
        }
        self.work -= 1;
        Ok(())
    }
}
fn error(message: impl std::fmt::Display) -> Error {
    EvalAltResult::ErrorRuntime(message.to_string().into(), Position::NONE).into()
}
fn line(position: Position) -> Option<u32> {
    position.line().and_then(|n| u32::try_from(n).ok())
}
fn error_line(error: &EvalAltResult) -> Option<u32> {
    match error {
        EvalAltResult::ErrorInFunctionCall(_, _, inner, _) => {
            error_line(inner).or_else(|| line(error.position()))
        }
        _ => line(error.position()),
    }
}
fn bounded(value: &Value) -> Result<()> {
    value.validate().map_err(error)?;
    if value.payload_bytes() > 16384 {
        return Err(error("value payload exceeds 16384 bytes"));
    }
    Ok(())
}
fn to_dynamic(value: &Value) -> Dynamic {
    match value {
        Value::Bool(v) => (*v).into(),
        Value::Int(v) => (*v).into(),
        Value::Float(v) => (*v).into(),
        Value::String(v) => v.clone().into(),
        Value::List(v) => v.iter().map(to_dynamic).collect::<Array>().into(),
        Value::Map(v) => v
            .iter()
            .map(|(k, v)| (k.clone().into(), to_dynamic(v)))
            .collect::<Map>()
            .into(),
    }
}
fn from_dynamic(value: Dynamic) -> Result<Value> {
    let value = if value.is::<bool>() {
        Value::Bool(value.cast())
    } else if value.is::<i64>() {
        Value::Int(value.cast())
    } else if value.is::<f64>() {
        Value::Float(value.cast())
    } else if value.is::<ImmutableString>() {
        Value::String(value.cast::<ImmutableString>().to_string())
    } else if value.is::<Array>() {
        Value::List(
            value
                .cast::<Array>()
                .into_iter()
                .map(scalar)
                .collect::<Result<_>>()?,
        )
    } else if value.is::<Map>() {
        Value::Map(
            value
                .cast::<Map>()
                .into_iter()
                .map(|(k, v)| Ok((k.to_string(), scalar(v)?)))
                .collect::<Result<_>>()?,
        )
    } else {
        return Err(error(
            "only bool/int/finite float/string and flat scalar lists/maps can enter state",
        ));
    };
    bounded(&value)?;
    Ok(value)
}
fn scalar(value: Dynamic) -> Result<Value> {
    if value.is::<Array>() || value.is::<Map>() {
        return Err(error("nested collections are unsupported"));
    }
    from_dynamic(value)
}
fn engine() -> Engine {
    let mut engine = Engine::new_raw();
    engine
        .set_max_operations(10000)
        .set_max_call_levels(8)
        .set_max_expr_depths(16, 16)
        .set_max_variables(256)
        .set_max_functions(64)
        .set_max_string_size(16384)
        .set_max_array_size(256)
        .set_max_map_size(256);
    for symbol in FORBIDDEN {
        engine.disable_symbol(*symbol);
    }
    engine.register_type_with_name::<Host>("State");
    engine.register_fn(
        "get",
        |host: &mut Host, key: ImmutableString| -> Result<Dynamic> {
            let context = host.0.borrow();
            context.permission(&key, false)?;
            let value = context.state.get(&key).map_err(error)?;
            bounded(value)?;
            Ok(to_dynamic(value))
        },
    );
    engine.register_fn(
        "set",
        |host: &mut Host, key: ImmutableString, value: Dynamic| -> Result<()> {
            let mut context = host.0.borrow_mut();
            context.permission(&key, true)?;
            let value = from_dynamic(value)?;
            context.spend()?;
            context.state.set(&key, value).map_err(error)
        },
    );
    engine.register_fn(
        "add",
        |host: &mut Host, key: ImmutableString, value: Dynamic| -> Result<()> {
            let mut context = host.0.borrow_mut();
            context.permission(&key, true)?;
            let value = scalar(value)?;
            context.spend()?;
            context.state.add(&key, &value).map_err(error)
        },
    );
    engine.register_fn(
        "reset",
        |host: &mut Host, key: ImmutableString| -> Result<()> {
            let mut context = host.0.borrow_mut();
            context.permission(&key, true)?;
            context.spend()?;
            context.state.reset(&key).map_err(error)
        },
    );
    engine.register_fn(
        "remove",
        |host: &mut Host, key: ImmutableString| -> Result<()> {
            let mut context = host.0.borrow_mut();
            context.permission(&key, true)?;
            context.spend()?;
            context.state.remove(&key).map(|_| ()).map_err(error)
        },
    );
    engine.register_fn(
        "emit",
        |host: &mut Host, id: ImmutableString, value: Dynamic| -> Result<()> {
            let mut context = host.0.borrow_mut();
            if id == "state.changed"
                || id.starts_with("state.changed.")
                || !context.definition.signals.iter().any(|s| s == id.as_str())
            {
                return Err(error(format!("signal permission denied for `{id}`")));
            }
            let signal = Signal {
                id: id.to_string(),
                value: scalar(value)?,
            };
            signal::validate(&signal).map_err(error)?;
            context.spend()?;
            if context.work <= context.signals.len() {
                return Err(error("signal queue budget exceeded"));
            }
            context.signals.push(signal);
            Ok(())
        },
    );
    engine.register_fn("len", |items: &mut Array| items.len() as i64);
    engine.register_fn("len", |items: &mut Map| items.len() as i64);
    engine.register_fn("len", |text: ImmutableString| text.len() as i64);
    engine.register_fn("push", |items: &mut Array, value: Dynamic| -> Result<()> {
        scalar(value.clone())?;
        if items.len() >= 256 {
            return Err(error("array entry limit exceeded"));
        }
        items.push(value);
        Ok(())
    });
    engine.register_fn(
        "contains",
        |items: &mut Array, value: Dynamic| -> Result<bool> {
            let value = scalar(value)?;
            for item in items.iter() {
                if scalar(item.clone())? == value {
                    return Ok(true);
                }
            }
            Ok(false)
        },
    );
    engine.register_fn("contains", |items: &mut Map, key: ImmutableString| {
        items.contains_key(key.as_str())
    });
    engine.register_fn("keys", |items: &mut Map| {
        items
            .keys()
            .map(|k| Dynamic::from(k.to_string()))
            .collect::<Array>()
    });
    engine.register_iterator::<Array>();
    engine
}

#[cfg(test)]
mod tests {
    use super::*;
    fn definition(source: &str) -> ScriptDefinition {
        ScriptDefinition {
            id: "test".into(),
            file: "mechanic.rhai".into(),
            source: source.into(),
            function: "run".into(),
            arity: 0,
            persistent_only: true,
            reads: vec!["count".into(), "items".into(), "schedule".into()],
            writes: vec!["count".into(), "items".into(), "schedule".into()],
            signals: vec!["done".into()],
        }
    }
    fn state() -> State {
        let mut state = State::new();
        state.set("count", Value::Int(0)).unwrap();
        state.set("items", Value::List(vec![])).unwrap();
        state
            .set(
                "schedule",
                Value::Map(BTreeMap::from([("energy".into(), Value::Int(3))])),
            )
            .unwrap();
        state.drain_changes();
        state
    }
    #[test]
    fn collections_formulas_and_deferred_signals_use_canonical_state() {
        let source = r#"fn run(s) {
            let items=s.get("items");
            if !items.contains("key") { items.push("key"); }
            s.set("items",items);
            let schedule=s.get("schedule"); schedule.energy=schedule.energy-1;
            s.set("schedule",schedule);
            s.set("count",items.len());
            s.emit("done",true);
        }"#;
        let mut programs = Programs::compile(&[definition(source)]).unwrap();
        let mut state = state();
        let mut budget = Budget::new(100);
        let signals = programs
            .execute("test", &[], &mut state, &mut budget)
            .unwrap();
        assert_eq!(
            signals,
            vec![Signal {
                id: "done".into(),
                value: Value::Bool(true)
            }]
        );
        assert_eq!(state.get("count"), Ok(&Value::Int(1)));
        assert_eq!(
            state.get("items"),
            Ok(&Value::List(vec![Value::String("key".into())]))
        );
        assert_eq!(
            state.get("schedule"),
            Ok(&Value::Map(BTreeMap::from([(
                "energy".into(),
                Value::Int(2)
            )])))
        );
        assert_eq!(state.pending_changes().len(), 3);
        assert!(budget.external_operations() < 10000);
        assert_eq!(budget.remaining(), 96);
    }
    #[test]
    fn failures_keep_completed_writes_and_report_the_inner_source_line() {
        let source = "fn fail(s) {\n s.get(\"denied\");\n}\nfn run(s) {\n s.set(\"count\",1);\n s.emit(\"done\",true);\n fail(s);\n}";
        let mut programs = Programs::compile(&[definition(source)]).unwrap();
        let mut state = state();
        let error = programs
            .execute("test", &[], &mut state, &mut Budget::new(100))
            .unwrap_err();
        assert_eq!(error.file, "mechanic.rhai");
        assert_eq!(error.line, Some(2));
        assert!(error.message.contains("permission denied"), "{error}");
        assert_eq!(state.get("count"), Ok(&Value::Int(1)));
        assert_eq!(state.pending_changes().len(), 1);
    }
    #[test]
    fn raw_engine_excludes_globals_reflection_and_external_access() {
        for body in [
            "eval(\"1\")",
            "Fn(\"run\")",
            "call(\"run\")",
            "curry(\"run\")",
            "print(\"x\")",
            "debug(1)",
            "type_of(1)",
            "is_shared(1)",
            "let x = is_shared(1)",
            "1.is_shared()",
            "let f = |x| x + 1",
            "is_def_fn(\"run\",1)",
            "is_def_var(\"x\")",
            "import \"file\" as x",
        ] {
            let definition = definition(&format!("fn run(s) {{ {body}; }}"));
            assert!(Programs::compile(&[definition]).is_err(), "accepted {body}");
        }
        assert!(Programs::compile(&[definition("let hidden=1; fn run(s) {}")]).is_err());
        assert!(Programs::compile(&[definition("fn other(s) {}")]).is_err());
        for body in ["read_file(\"file\")", "exit(0)", "now()"] {
            let mut programs =
                Programs::compile(&[definition(&format!("fn run(s) {{ {body}; }}"))]).unwrap();
            assert!(
                programs
                    .execute("test", &[], &mut state(), &mut Budget::new(100))
                    .is_err()
            );
        }
    }
    #[test]
    fn operation_and_host_work_budgets_do_not_reset_between_calls() {
        let mut programs = Programs::compile(&[definition("fn run(s) { while true {} }")]).unwrap();
        let mut state = state();
        let before = state.clone();
        let mut budget = Budget::new(100);
        let error = programs
            .execute("test", &[], &mut state, &mut budget)
            .unwrap_err();
        assert!(
            error.message.to_lowercase().contains("operations"),
            "{error}"
        );
        assert_eq!(state, before);
        assert_eq!(budget.external_operations(), 0);
        assert!(
            programs
                .execute("test", &[], &mut state, &mut budget)
                .unwrap_err()
                .message
                .contains("operation budget")
        );
        let mut programs = Programs::compile(&[definition(
            "fn run(s) { while true { s.add(\"count\",1); } }",
        )])
        .unwrap();
        let mut budget = Budget::new(20);
        let error = programs
            .execute("test", &[], &mut state, &mut budget)
            .unwrap_err();
        assert!(error.message.contains("work budget"), "{error}");
        assert_eq!(state.get("count"), Ok(&Value::Int(20)));
        assert_eq!(budget.remaining(), 0);
    }
    #[test]
    fn value_and_scope_boundaries_fail_without_corrupting_state() {
        for body in [
            "s.set(\"items\",[[1]])",
            "s.set(\"count\",())",
            "s.set(\"count\",'x')",
            "s.set(\"count\",true)",
            "s.set(\"denied\",1)",
            "s.emit(\"denied\",true)",
            "s.emit(\"done\",[])",
        ] {
            let mut programs =
                Programs::compile(&[definition(&format!("fn run(s) {{ {body}; }}"))]).unwrap();
            let mut state = state();
            let before = state.clone();
            assert!(
                programs
                    .execute("test", &[], &mut state, &mut Budget::new(100))
                    .is_err(),
                "{body}"
            );
            assert_eq!(state, before);
        }
        let mut state = State::new();
        state
            .declare(
                "count",
                weftora_core::StateDefinition {
                    default: Value::Int(0),
                    scope: StateScope::Session,
                },
            )
            .unwrap();
        let mut programs =
            Programs::compile(&[definition("fn run(s) { s.set(\"count\",1); }")]).unwrap();
        assert!(
            programs
                .execute("test", &[], &mut state, &mut Budget::new(100))
                .unwrap_err()
                .message
                .contains("Persistent")
        );
    }
    #[test]
    fn recursive_expressions_fail_within_a_small_stack_and_deadline() {
        use std::{
            process::{Command, Stdio},
            time::{Duration, Instant},
        };
        if std::env::var_os("WEFTORA_SCRIPT_STACK_CHILD").is_some() {
            std::thread::Builder::new()
                .stack_size(1024 * 1024)
                .spawn(|| {
                    let mut deepest = None;
                    for depth in 0..16 {
                        let expression =
                            format!("{}run(s){}", "1+(".repeat(depth), ")".repeat(depth));
                        match Programs::compile(&[definition(&format!(
                            "fn run(s) {{ {expression}; }}"
                        ))]) {
                            Ok(programs) => deepest = Some(programs),
                            Err(_) => break,
                        }
                    }
                    let mut programs = deepest.expect("no recursive expression compiled");
                    let error = programs
                        .execute("test", &[], &mut state(), &mut Budget::new(100))
                        .unwrap_err();
                    assert!(error.message.to_lowercase().contains("stack"), "{error}");
                })
                .unwrap()
                .join()
                .unwrap();
            return;
        }
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "tests::recursive_expressions_fail_within_a_small_stack_and_deadline",
                "--nocapture",
            ])
            .env("WEFTORA_SCRIPT_STACK_CHILD", "1")
            .stdout(Stdio::null())
            .spawn()
            .unwrap();
        let start = Instant::now();
        loop {
            if let Some(status) = child.try_wait().unwrap() {
                assert!(status.success(), "script child failed: {status}");
                break;
            }
            if start.elapsed() > Duration::from_secs(10) {
                child.kill().unwrap();
                child.wait().unwrap();
                panic!("script child exceeded deadline");
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    #[test]
    fn recursion_expression_and_data_limits_are_controlled_errors() {
        for (body, expected) in [
            ("run(s)", "stack"),
            ("let a=[]; while true { a.push(1); }", "array entry limit"),
            ("let a=\"x\"; while true { a+=a; }", "string"),
        ] {
            let mut programs =
                Programs::compile(&[definition(&format!("fn run(s) {{ {body}; }}"))]).unwrap();
            let error = programs
                .execute("test", &[], &mut state(), &mut Budget::new(100))
                .unwrap_err();
            assert!(
                error.message.to_lowercase().contains(expected),
                "{body}: {error}"
            );
        }
        let entries = (0..257)
            .map(|i| format!("k{i}: {i}"))
            .collect::<Vec<_>>()
            .join(",");
        let error = Programs::compile(&[definition(&format!(
            "fn run(s) {{ let m=#{{{entries}}}; }}"
        ))])
        .err()
        .expect("oversized map accepted");
        assert!(
            error[0].message.to_lowercase().contains("map"),
            "{:?}",
            error
        );
        let source = format!(
            "fn run(s) {{ let a={}1{}; }}",
            "(".repeat(100),
            ")".repeat(100)
        );
        assert!(Programs::compile(&[definition(&source)]).is_err());
    }
}
