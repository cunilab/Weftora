//! Deterministic JSON-lines replay without starting the player or a renderer.
use super::{Checked, CliResult, Failure, check, presentation_commands};
use serde_json::json;
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::{Arc, RwLock};
use std::task::{Context, Poll, Waker};
use weftora_api::Value;
use weftora_core::command::CommandRegistry;
use weftora_core::lifecycle::{Lifecycle, LifecycleError, Service};
use weftora_core::signal::{Signal, SignalBus};
use weftora_core::{SharedState, StateChange};
use weftora_yarn::{DialogueEvent, DialogueRunner};

#[derive(Default)]
struct Resources {
    checked: Option<Checked>,
    state: Option<SharedState>,
    commands: Option<CommandRegistry>,
    signals: Option<SignalBus>,
    runner: Option<DialogueRunner>,
}

/// One Story owns its state, commands, observers and dialogue until lifecycle teardown.
struct StoryService {
    root: PathBuf,
    resources: Rc<RefCell<Resources>>,
    observed: Rc<RefCell<Vec<Signal>>>,
}
impl Service for StoryService {
    fn name(&self) -> &str {
        "story"
    }
    fn init(&mut self) -> Result<(), String> {
        let checked = check(&self.root).map_err(|errors| errors.join("\n"))?;
        let state = Arc::new(RwLock::new(
            checked.story.initial_state().map_err(|e| e.to_string())?,
        ));
        let mut resources = self.resources.borrow_mut();
        resources.checked = Some(checked);
        resources.state = Some(state.clone());
        resources.commands = Some(presentation_commands());
        let mut signals = SignalBus::default();
        let observed = self.observed.clone();
        signals
            .subscribe("state.changed", move |signal, _| {
                observed.borrow_mut().push(signal.clone());
                Ok(Vec::new())
            })
            .map_err(|e| e.to_string())?;
        resources.signals = Some(signals);
        let checked = resources.checked.as_ref().expect("Story loaded");
        let entrypoint = checked.story.manifest().entrypoint.clone();
        let runner = DialogueRunner::new(&checked.dialogue, state.clone());
        resources.runner = Some(runner);
        resources
            .runner
            .as_mut()
            .expect("runner created")
            .start(&entrypoint)
            .map_err(|e| e.to_string())?;
        state
            .write()
            .map_err(|_| "state lock is poisoned")?
            .drain_changes();
        Ok(())
    }
    fn shutdown(&mut self) -> Result<(), String> {
        let mut resources = self.resources.borrow_mut();
        resources.runner = None;
        resources.signals = None;
        resources.commands = None;
        resources.state = None;
        resources.checked = None;
        Ok(())
    }
}

pub fn run(dir: &Path, choices: &[usize]) -> CliResult {
    let resources = Rc::new(RefCell::new(Resources::default()));
    let observed = Rc::new(RefCell::new(Vec::new()));
    let mut lifecycle = Lifecycle::new(vec![Box::new(StoryService {
        root: dir.into(),
        resources: resources.clone(),
        observed: observed.clone(),
    })]);
    lifecycle.load().map_err(|error| match error {
        LifecycleError::Init { message, .. } => {
            Failure::Messages(message.lines().map(str::to_owned).collect())
        }
        other => Failure::one(other.to_string()),
    })?;
    lifecycle.play().map_err(|e| Failure::one(e.to_string()))?;
    let (values, changes) = replay(&mut resources.borrow_mut(), choices)?;
    lifecycle
        .shutdown()
        .map_err(|e| Failure::one(e.to_string()))?;
    println!(
        "{}",
        json!({"event":"end", "state":values,"changes":changes,"signals":*observed.borrow(),"lifecycle":lifecycle.trace()})
    );
    Ok(std::process::ExitCode::SUCCESS)
}

type Replay = (std::collections::BTreeMap<String, Value>, Vec<StateChange>);
fn replay(resources: &mut Resources, choices: &[usize]) -> Result<Replay, Failure> {
    let state = resources.state.as_ref().expect("ready state");
    let runner = resources.runner.as_mut().expect("ready dialogue");
    let commands = resources.commands.as_ref().expect("ready commands");
    let signals = resources.signals.as_mut().expect("ready signals");
    let mut changes = Vec::new();
    let mut choices = choices.iter();
    // A scripted run must terminate even when Story code emits an unlimited sequence of lines.
    let mut ended = false;
    for _ in 0..10_000 {
        let event = runner
            .next_event()
            .map_err(|e| Failure::one(e.to_string()))?;
        {
            // Dispatch while holding this one write guard; handlers use the supplied State, never relock it.
            let mut state = state
                .write()
                .map_err(|_| Failure::one("state lock is poisoned"))?;
            let pending = state.drain_changes();
            for change in &pending {
                signals
                    .emit(
                        Signal {
                            id: "state.changed".into(),
                            value: Value::String(change.key.clone()),
                        },
                        &mut state,
                        1024,
                    )
                    .map_err(|e| Failure::one(e.to_string()))?;
            }
            changes.extend(pending);
        }
        match event {
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
    let state = state
        .read()
        .map_err(|_| Failure::one("state lock is poisoned"))?;
    let values = state
        .iter()
        .map(|(key, value)| (key.to_string(), value.clone()))
        .collect();
    Ok((values, changes))
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

#[cfg(test)]
mod tests {
    use super::*;
    use weftora_core::lifecycle::Phase;

    #[test]
    fn actual_story_service_rolls_back_partial_startup_and_releases_state() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("manifest.jsonc"), r#"{"manifestSchema":0,"id":"test.startup","storyVersion":"1","title":"Test","entrypoint":"Start","dialogue":["main.yarn"]}"#).unwrap();
        std::fs::write(
            dir.path().join("main.yarn"),
            "title: Start\n---\nHello.\n===\n",
        )
        .unwrap();
        let resources = Rc::new(RefCell::new(Resources::default()));
        let weak = Rc::new(RefCell::new(None));
        struct FailAfterInit {
            service: StoryService,
            weak: Rc<RefCell<Option<std::sync::Weak<RwLock<weftora_core::State>>>>>,
        }
        impl Service for FailAfterInit {
            fn name(&self) -> &str {
                self.service.name()
            }
            fn init(&mut self) -> Result<(), String> {
                self.service.init()?;
                let resources = self.service.resources.borrow();
                assert!(
                    resources.checked.is_some()
                        && resources.commands.is_some()
                        && resources.signals.is_some()
                        && resources.runner.is_some()
                );
                *self.weak.borrow_mut() = Some(Arc::downgrade(resources.state.as_ref().unwrap()));
                Err("injected failure after actual Story initialization".into())
            }
            fn shutdown(&mut self) -> Result<(), String> {
                self.service.shutdown()
            }
        }
        let service = StoryService {
            root: dir.path().into(),
            resources: resources.clone(),
            observed: Rc::new(RefCell::new(Vec::new())),
        };
        let mut lifecycle = Lifecycle::new(vec![Box::new(FailAfterInit {
            service,
            weak: weak.clone(),
        })]);
        assert!(lifecycle.load().is_err());
        assert!(weak.borrow().as_ref().unwrap().upgrade().is_none());
        assert_eq!(lifecycle.phase(), Phase::Shutdown);
        assert_eq!(
            lifecycle.trace(),
            ["boot", "load", "init:story", "shutdown:story", "shutdown"]
        );
        let resources = resources.borrow();
        assert!(
            resources.checked.is_none()
                && resources.state.is_none()
                && resources.commands.is_none()
                && resources.signals.is_none()
                && resources.runner.is_none()
        );
    }
}
