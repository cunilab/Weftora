//! Deterministic JSON-lines replay without starting the player or a renderer.
use super::{CliResult, Failure, check};
use serde_json::json;
use std::path::Path;
use std::sync::{Arc, RwLock};
use std::task::{Context, Poll, Waker};
use weftora_api::Value;
use weftora_core::command::{CommandRegistry, ValueType};
use weftora_core::lifecycle::Lifecycle;
use weftora_yarn::{DialogueEvent, DialogueRunner};

pub fn run(dir: &Path, choices: &[usize]) -> CliResult {
    let checked = check(dir).map_err(Failure::Messages)?;
    let state = Arc::new(RwLock::new(
        checked
            .story
            .initial_state()
            .map_err(|e| Failure::one(e.to_string()))?,
    ));
    let mut lifecycle = Lifecycle::new(Vec::new());
    lifecycle.load().map_err(|e| Failure::one(e.to_string()))?;
    let mut runner = DialogueRunner::new(&checked.dialogue, state.clone());
    runner
        .start(&checked.story.manifest().entrypoint)
        .map_err(|e| Failure::one(e.to_string()))?;
    lifecycle.play().map_err(|e| Failure::one(e.to_string()))?;
    let mut commands = CommandRegistry::default();
    for id in ["background", "sound"] {
        commands
            .register(id, vec![ValueType::String], ValueType::Bool, |_| {
                Box::pin(async { Ok(Value::Bool(true)) })
            })
            .map_err(|e| Failure::one(e.to_string()))?;
    }
    let mut choices = choices.iter();
    // A scripted run must terminate even when Story code emits an unlimited sequence of lines.
    let mut ended = false;
    for _ in 0..10_000 {
        match runner
            .next_event()
            .map_err(|e| Failure::one(e.to_string()))?
        {
            DialogueEvent::Line { character, text } => println!(
                "{}",
                json!({"event":"line", "character":character, "text":text})
            ),
            DialogueEvent::Command(command) => {
                let arguments = command.args.iter().cloned().map(Value::String).collect();
                let mut task = commands
                    .start(&command.name, arguments)
                    .map_err(|e| Failure::one(e.to_string()))?;
                match task.poll(&mut Context::from_waker(Waker::noop())) {
                    Poll::Ready(Ok(_)) => {}
                    Poll::Ready(Err(error)) => return Err(Failure::one(error.to_string())),
                    Poll::Pending => {
                        task.cancel();
                        return Err(Failure::one(
                            "presentation command did not complete in headless mode",
                        ));
                    }
                }
                println!(
                    "{}",
                    json!({"event":"command", "name":command.name, "args":command.args})
                );
            }
            DialogueEvent::Choices(options) => {
                let options: Vec<_> = options
                    .iter()
                    .map(|c| json!({"index":c.index,"text":c.text,"available":c.available}))
                    .collect();
                println!("{}", json!({"event":"choices", "options":options}));
                let index = choices.next().ok_or_else(|| {
                    Failure::one("headless choice requires --choices <comma-separated option IDs>")
                })?;
                runner
                    .choose(*index)
                    .map_err(|e| Failure::one(e.to_string()))?;
                println!("{}", json!({"event":"choose", "index":index}));
            }
            DialogueEvent::End => {
                ended = true;
                break;
            }
        }
    }
    if !ended {
        return Err(Failure::one("headless event budget (10000) exceeded"));
    }
    if choices.next().is_some() {
        return Err(Failure::one("unused headless choices after dialogue ended"));
    }
    lifecycle
        .shutdown()
        .map_err(|e| Failure::one(e.to_string()))?;
    let mut state = state
        .write()
        .map_err(|_| Failure::one("state lock is poisoned"))?;
    let values: std::collections::BTreeMap<_, _> = state
        .iter()
        .map(|(k, v)| (k.to_string(), v.clone()))
        .collect();
    let changes = state.drain_changes();
    println!(
        "{}",
        json!({"event":"end", "state":values,"changes":changes,"lifecycle":lifecycle.trace()})
    );
    Ok(std::process::ExitCode::SUCCESS)
}

pub fn parse_choices(text: &str) -> Result<Vec<usize>, Failure> {
    text.split(',')
        .map(|part| {
            part.parse().map_err(|_| {
                Failure::one(format!(
                    "invalid headless choice `{part}`: expected non-negative option ID"
                ))
            })
        })
        .collect()
}
