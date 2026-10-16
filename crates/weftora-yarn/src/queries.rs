//! Read-only scalar views; collections never enter Yarn's variable store.
use crate::CompiledDialogue;
use weftora_api::{Diagnostic, Value};
use weftora_core::{Scope, SharedState, State};
use weftora_story::Story;
use yarnspinner_core::prelude::{Library, OpCode, OperandValue, YarnValue};

fn arity(name: &str) -> Option<usize> {
    match name {
        "state_list_len" | "state_map_len" => Some(1),
        "state_list_contains" | "state_map_contains_key" => Some(2),
        _ => None,
    }
}

/// Signature-only registrations for compilation/artifact checking; execution uses the fallible handler.
pub fn library() -> Library {
    fn missing_handler() -> ! {
        panic!("collection query requires the Weftora host handler")
    }
    let mut library = Library::new();
    library
        .add_function("state_list_len", |_: String| -> f32 { missing_handler() })
        .add_function("state_map_len", |_: String| -> f32 { missing_handler() })
        .add_function("state_list_contains", |_: String, _: YarnValue| -> bool {
            missing_handler()
        })
        .add_function("state_map_contains_key", |_: String, _: String| -> bool {
            missing_handler()
        });
    library
}

fn target<'a>(
    state: &'a State,
    key: &str,
    name: &str,
    persistent_only: bool,
    default: bool,
) -> Result<&'a Value, String> {
    if key.len() > 512 {
        return Err("state key exceeds 512 bytes".into());
    }
    weftora_core::state::validate_key(key).map_err(|e| e.to_string())?;
    let scope = state.scope(key).map_err(|e| e.to_string())?;
    if persistent_only && scope != Scope::Persistent {
        return Err("save-enabled collection queries require Persistent state".into());
    }
    let value = if default {
        state.default_value(key)
    } else {
        state.get(key)
    }
    .map_err(|e| e.to_string())?;
    if !matches!(
        (name, value),
        ("state_list_len" | "state_list_contains", Value::List(_))
            | ("state_map_len" | "state_map_contains_key", Value::Map(_))
    ) {
        return Err(format!(
            "{name} requires a {} state key, `{key}` is {}",
            if name.starts_with("state_list") {
                "list"
            } else {
                "map"
            },
            value.type_name()
        ));
    }
    Ok(value)
}

pub(super) fn evaluate(
    name: &str,
    args: &[YarnValue],
    state: &SharedState,
    persistent_only: bool,
) -> Result<Option<YarnValue>, String> {
    let Some(arity) = arity(name) else {
        return Ok(None);
    };
    let fail = |message: String| format!("{name}: {message}");
    if args.len() != arity {
        return Err(fail(format!("requires {arity} arguments")));
    }
    let YarnValue::String(key) = &args[0] else {
        return Err(fail("state key must be a string".into()));
    };
    let state = state
        .read()
        .map_err(|_| fail("state lock is poisoned".into()))?;
    let value = target(&state, key, name, persistent_only, false).map_err(fail)?;
    let result = match (name, value) {
        ("state_list_len", Value::List(items)) => YarnValue::Number(items.len() as f32),
        ("state_map_len", Value::Map(items)) => YarnValue::Number(items.len() as f32),
        ("state_map_contains_key", Value::Map(items)) => {
            let YarnValue::String(entry) = &args[1] else {
                return Err(fail("map entry key must be a string".into()));
            };
            if entry.len() > 512 {
                return Err(fail("map entry key exceeds 512 bytes".into()));
            }
            YarnValue::Boolean(items.contains_key(entry))
        }
        ("state_list_contains", Value::List(items)) => {
            let needle = &args[1];
            match needle {
                YarnValue::String(s) if s.len() > 16384 => {
                    return Err(fail("needle exceeds 16384 bytes".into()));
                }
                YarnValue::Number(n) if !n.is_finite() => {
                    return Err(fail("needle must be finite".into()));
                }
                _ => {}
            }
            YarnValue::Boolean(items.iter().any(|item| match (item, needle) {
                (Value::Bool(a), YarnValue::Boolean(b)) => a == b,
                (Value::String(a), YarnValue::String(b)) => a == b,
                (Value::Float(a), YarnValue::Number(b)) => *a == f64::from(*b),
                // Compare integers as integers; rounding a large stored int could yield a false match.
                (Value::Int(a), YarnValue::Number(b)) => {
                    b.fract() == 0.0
                        && f64::from(*b) >= -9223372036854775808.0
                        && f64::from(*b) < 9223372036854775808.0
                        && *a == *b as i64
                }
                _ => false,
            }))
        }
        _ => unreachable!("target checked the collection kind"),
    };
    Ok(Some(result))
}

pub(super) fn validate(story: &Story, dialogue: &CompiledDialogue) -> Vec<Diagnostic> {
    let mut errors = Vec::new();
    let used = dialogue.program.nodes.values().flat_map(|node| &node.instructions).any(|i| {
        i.opcode == OpCode::CallFunc as i32 && matches!(i.operands.first().and_then(|v| v.value.as_ref()), Some(OperandValue::StringValue(name)) if arity(name).is_some())
    });
    if !used {
        return errors;
    }
    let state = match story.initial_state() {
        Ok(state) => state,
        Err(_) => return errors,
    };
    let capability = story
        .manifest()
        .requires
        .as_ref()
        .is_some_and(|r| r.capabilities.iter().any(|c| c == "dialogue.collections/1"));
    for (node_name, node) in &dialogue.program.nodes {
        for (i, instruction) in node.instructions.iter().enumerate() {
            if instruction.opcode != OpCode::CallFunc as i32 {
                continue;
            }
            let Some(OperandValue::StringValue(name)) =
                instruction.operands.first().and_then(|v| v.value.as_ref())
            else {
                continue;
            };
            let Some(count) = arity(name) else {
                continue;
            };
            let fail = |message: String| {
                Diagnostic::new(
                    dialogue.node_file(node_name),
                    None,
                    format!("{name}: {message} (node `{node_name}`)"),
                )
            };
            if !capability {
                errors.push(fail("requires dialogue.collections/1".into()));
                continue;
            }
            // Validate directly pushed literal keys; computed arguments are always checked at execution.
            if let Some(start) = i.checked_sub(count + 1) {
                let args = &node.instructions[start..i - 1];
                if args.iter().all(|a| {
                    [
                        OpCode::PushString,
                        OpCode::PushFloat,
                        OpCode::PushBool,
                        OpCode::PushVariable,
                    ]
                    .iter()
                    .any(|op| a.opcode == *op as i32)
                }) && args[0].opcode == OpCode::PushString as i32
                    && let Some(OperandValue::StringValue(key)) =
                        args[0].operands.first().and_then(|v| v.value.as_ref())
                    && let Err(message) = target(
                        &state,
                        key,
                        name,
                        story.manifest().save_schema.is_some(),
                        true,
                    )
                {
                    errors.push(fail(message));
                }
            }
        }
    }
    errors
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        collections::BTreeMap,
        sync::{Arc, RwLock},
    };
    fn state() -> SharedState {
        let mut state = State::new();
        state
            .set(
                "bag.items",
                Value::List(vec![
                    Value::Bool(true),
                    Value::String("key".into()),
                    Value::Int(1),
                    Value::Int(16777217),
                    Value::Int(i64::MAX),
                    Value::Int(i64::MIN),
                    Value::Float(0.5),
                    Value::Float(0.1),
                ]),
            )
            .unwrap();
        state
            .set(
                "schedule",
                Value::Map(BTreeMap::from([
                    ("".into(), Value::Int(1)),
                    ("morning".into(), Value::String("station".into())),
                ])),
            )
            .unwrap();
        state.set("scalar", Value::Int(0)).unwrap();
        state
            .declare(
                "temporary",
                weftora_core::StateDefinition {
                    default: Value::List(vec![]),
                    scope: Scope::Session,
                },
            )
            .unwrap();
        state.drain_changes();
        Arc::new(RwLock::new(state))
    }
    #[test]
    fn scalar_queries_are_live_read_only_and_never_round_stored_numbers() {
        let state = state();
        for (name, args, expected) in [
            (
                "state_list_len",
                vec!["bag.items".into()],
                YarnValue::Number(8.0),
            ),
            (
                "state_map_len",
                vec!["schedule".into()],
                YarnValue::Number(2.0),
            ),
            (
                "state_map_contains_key",
                vec!["schedule".into(), "".into()],
                YarnValue::Boolean(true),
            ),
            (
                "state_map_contains_key",
                vec!["schedule".into(), "absent".into()],
                YarnValue::Boolean(false),
            ),
            (
                "state_list_contains",
                vec!["bag.items".into(), true.into()],
                YarnValue::Boolean(true),
            ),
            (
                "state_list_contains",
                vec!["bag.items".into(), "key".into()],
                YarnValue::Boolean(true),
            ),
            (
                "state_list_contains",
                vec!["bag.items".into(), 1.0f32.into()],
                YarnValue::Boolean(true),
            ),
            (
                "state_list_contains",
                vec!["bag.items".into(), 16777216.0f32.into()],
                YarnValue::Boolean(false),
            ),
            (
                "state_list_contains",
                vec!["bag.items".into(), (i64::MAX as f32).into()],
                YarnValue::Boolean(false),
            ),
            (
                "state_list_contains",
                vec!["bag.items".into(), (i64::MIN as f32).into()],
                YarnValue::Boolean(true),
            ),
            (
                "state_list_contains",
                vec!["bag.items".into(), 0.5f32.into()],
                YarnValue::Boolean(true),
            ),
            (
                "state_list_contains",
                vec!["bag.items".into(), 0.1f32.into()],
                YarnValue::Boolean(false),
            ),
            (
                "state_list_contains",
                vec!["bag.items".into(), "1".into()],
                YarnValue::Boolean(false),
            ),
        ] {
            let before = state.read().unwrap().clone();
            assert_eq!(evaluate(name, &args, &state, true).unwrap(), Some(expected));
            assert_eq!(*state.read().unwrap(), before);
        }
        state
            .write()
            .unwrap()
            .set("bag.items", Value::List(vec![]))
            .unwrap();
        let before = state.read().unwrap().clone();
        assert_eq!(
            evaluate("state_list_len", &["bag.items".into()], &state, true).unwrap(),
            Some(YarnValue::Number(0.0))
        );
        assert_eq!(*state.read().unwrap(), before);
        assert!(evaluate("round", &[], &state, true).unwrap().is_none());
    }
    #[test]
    fn invalid_query_arguments_and_state_fail_without_changes() {
        let state = state();
        for (name, args) in [
            ("state_list_len", vec![]),
            ("state_list_len", vec![1.0f32.into()]),
            ("state_list_len", vec!["missing".into()]),
            ("state_list_len", vec!["scalar".into()]),
            ("state_list_len", vec!["schedule".into()]),
            ("state_list_len", vec!["temporary".into()]),
            (
                "state_list_contains",
                vec!["bag.items".into(), f32::NAN.into()],
            ),
            (
                "state_list_contains",
                vec!["bag.items".into(), "x".repeat(16385).into()],
            ),
            (
                "state_map_contains_key",
                vec!["schedule".into(), true.into()],
            ),
            (
                "state_map_contains_key",
                vec!["schedule".into(), "x".repeat(513).into()],
            ),
        ] {
            let before = state.read().unwrap().clone();
            assert!(evaluate(name, &args, &state, true).is_err(), "{name}");
            assert_eq!(*state.read().unwrap(), before);
        }
        state.write().unwrap().remove("bag.items").unwrap();
        let before = state.read().unwrap().clone();
        assert!(evaluate("state_list_len", &["bag.items".into()], &state, true).is_err());
        assert_eq!(*state.read().unwrap(), before);
        assert_eq!(
            evaluate("state_list_len", &["temporary".into()], &state, false).unwrap(),
            Some(YarnValue::Number(0.0))
        );
    }
}
