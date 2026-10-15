//! Deterministic JSON-lines replay without starting the player or a renderer.
use super::{Checked, CliResult, Failure, check, presentation_commands};
use serde_json::json;
use std::cell::RefCell;
use std::collections::VecDeque;
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
pub struct Options {
    choices: Vec<usize>,
    save_at: Option<String>,
    load: bool,
    save_dir: Option<PathBuf>,
}

pub fn parse_options(flags: &[&str]) -> Result<Options, Failure> {
    let mut options = Options::default();
    let mut seen = std::collections::BTreeSet::new();
    let mut flags = flags.iter();
    while let Some(flag) = flags.next() {
        if !seen.insert(*flag) {
            return Err(Failure::Usage);
        }
        match *flag {
            "--load" => options.load = true,
            "--choices" => options.choices = parse_choices(flags.next().ok_or(Failure::Usage)?)?,
            "--save-at" => options.save_at = Some(flags.next().ok_or(Failure::Usage)?.to_string()),
            "--save-dir" => {
                options.save_dir = Some(PathBuf::from(flags.next().ok_or(Failure::Usage)?))
            }
            _ => return Err(Failure::Usage),
        }
    }
    if options.save_dir.is_some() && !options.load && options.save_at.is_none() {
        return Err(Failure::Usage);
    }
    Ok(options)
}

#[derive(Default)]
struct Resources {
    checked: Option<Checked>,
    state: Option<SharedState>,
    commands: Option<CommandRegistry>,
    signals: Option<SignalBus>,
    runner: Option<DialogueRunner>,
    save: Option<(weftora_save::Context, weftora_save::storage::Store)>,
    preload: VecDeque<DialogueEvent>,
}

/// One Story owns its state, commands, observers and dialogue until lifecycle teardown.
struct StoryService {
    root: PathBuf,
    options: Options,
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
        let mut runner = DialogueRunner::new(&checked.dialogue, state.clone());
        let mut preload = VecDeque::new();
        let save = if self.options.load || self.options.save_at.is_some() {
            let context = weftora_yarn::save_context(&checked.story, &checked.dialogue)
                .map_err(|e| e.to_string())?;
            if let Some(id) = &self.options.save_at
                && !context.checkpoints.contains(id)
            {
                return Err(format!("save checkpoint '{id}' is not available"));
            }
            let directory = weftora_save::directory::resolve(
                &context.story_id,
                checked.story.root(),
                self.options.save_dir.as_deref(),
            )
            .map_err(|e| e.to_string())?;
            Some((context, weftora_save::storage::Store::new(directory)))
        } else {
            None
        };
        state
            .write()
            .map_err(|_| "state lock is poisoned")?
            .drain_changes();
        if self.options.load {
            let (context, store) = save.as_ref().expect("save options validated");
            let game = store.load(context).map_err(|e| e.to_string())?;
            let prepared = {
                let template = state.read().map_err(|_| "state lock is poisoned")?;
                game.prepare_state(context, &template, &template)
                    .map_err(|e| e.to_string())?
            };
            *state.write().map_err(|_| "state lock is poisoned")? = prepared;
            runner
                .restore_visits(game.visits())
                .map_err(|e| e.to_string())?;
            runner
                .start(&checked.dialogue.checkpoints()[game.checkpoint()])
                .map_err(|e| e.to_string())?;
            // Validate the complete restore prefix before publishing any gameplay event.
            for _ in 0..10_000 {
                let event = runner.next_event().map_err(|e| e.to_string())?;
                match &event {
                    DialogueEvent::Command(command) => {
                        poll_command(
                            resources.commands.as_ref().expect("commands ready"),
                            command,
                        )?;
                    }
                    DialogueEvent::Line { .. } | DialogueEvent::Choices(_) => {
                        let point = runner.save_point().map_err(|e| e.to_string())?;
                        if point.checkpoint != game.checkpoint() {
                            return Err("restored display is not at the saved checkpoint".into());
                        }
                        preload.push_back(event);
                        break;
                    }
                    DialogueEvent::End => {
                        return Err("saved checkpoint has no blocking display".into());
                    }
                }
                preload.push_back(event);
            }
            if preload.back().is_none_or(|event| {
                !matches!(
                    event,
                    DialogueEvent::Line { .. } | DialogueEvent::Choices(_)
                )
            }) {
                return Err("checkpoint restore event budget exceeded".into());
            }
            if weftora_yarn::media_enabled(&checked.story) {
                weftora_core::presentation::volumes(
                    &*state.read().map_err(|_| "state lock is poisoned")?,
                )?;
            }
        } else {
            runner.start(&entrypoint).map_err(|e| e.to_string())?;
        }
        resources.runner = Some(runner);
        resources.save = save;
        resources.preload = preload;
        Ok(())
    }
    fn shutdown(&mut self) -> Result<(), String> {
        let mut resources = self.resources.borrow_mut();
        resources.runner = None;
        resources.save = None;
        resources.preload.clear();
        resources.signals = None;
        resources.commands = None;
        resources.state = None;
        resources.checked = None;
        Ok(())
    }
}

pub fn run(dir: &Path, options: Options) -> CliResult {
    let choices = options.choices.clone();
    let save_at = options.save_at.clone();
    let resources = Rc::new(RefCell::new(Resources::default()));
    let observed = Rc::new(RefCell::new(Vec::new()));
    let mut lifecycle = Lifecycle::new(vec![Box::new(StoryService {
        root: dir.into(),
        options,
        resources: resources.clone(),
        observed: observed.clone(),
    })]);
    lifecycle.load().map_err(|error| match error {
        LifecycleError::Init {
            message, cleanup, ..
        } => {
            let mut messages = vec![message];
            if !cleanup.is_empty() {
                messages.push(format!("cleanup errors: {cleanup:?}"));
            }
            Failure::Messages(messages)
        }
        other => Failure::one(other.to_string()),
    })?;
    lifecycle.play().map_err(|e| Failure::one(e.to_string()))?;
    let result = replay(&mut resources.borrow_mut(), &choices, save_at.as_deref());
    lifecycle
        .shutdown()
        .map_err(|e| Failure::one(e.to_string()))?;
    let (values, changes, saved) = result?;
    println!(
        "{}",
        json!({"event":if saved { "saved" } else { "end" }, "state":values,"changes":changes,"signals":*observed.borrow(),"lifecycle":lifecycle.trace()})
    );
    Ok(std::process::ExitCode::SUCCESS)
}

type Replay = (
    std::collections::BTreeMap<String, Value>,
    Vec<StateChange>,
    bool,
);
fn replay(
    resources: &mut Resources,
    choices: &[usize],
    save_at: Option<&str>,
) -> Result<Replay, Failure> {
    let media_enabled =
        weftora_yarn::media_enabled(&resources.checked.as_ref().expect("ready Story").story);
    let state = resources.state.as_ref().expect("ready state");
    let runner = resources.runner.as_mut().expect("ready dialogue");
    let commands = resources.commands.as_ref().expect("ready commands");
    let signals = resources.signals.as_mut().expect("ready signals");
    let mut changes = Vec::new();
    let mut change_bytes = 0usize;
    let mut choices = choices.iter();
    // A scripted run must terminate even when Story code emits an unlimited sequence of lines.
    let mut ended = false;
    let mut saved = false;
    for _ in 0..10_000 {
        let preloaded = !resources.preload.is_empty();
        let event = match resources.preload.pop_front() {
            Some(event) => event,
            None => runner
                .next_event()
                .map_err(|e| Failure::one(e.to_string()))?,
        };
        let save_here = if matches!(
            event,
            DialogueEvent::Line { .. } | DialogueEvent::Choices(_)
        ) {
            save_at.is_some_and(|id| {
                runner
                    .save_point()
                    .is_ok_and(|point| point.checkpoint == id)
            })
        } else {
            false
        };
        {
            // Dispatch while holding this one write guard; handlers use the supplied State, never relock it.
            let mut state = state
                .write()
                .map_err(|_| Failure::one("state lock is poisoned"))?;
            if media_enabled {
                weftora_core::presentation::volumes(&state).map_err(Failure::one)?;
            }
            let pending = state.drain_changes();
            let pending_bytes: usize = pending
                .iter()
                .map(|change| {
                    change.key.len()
                        + [&change.before, &change.after]
                            .into_iter()
                            .flatten()
                            .map(|value| match value {
                                Value::String(text) => text.len(),
                                _ => 0,
                            })
                            .sum::<usize>()
                })
                .sum();
            if pending.len() > 4096usize.saturating_sub(changes.len())
                || pending_bytes > (32 * 1024 * 1024usize).saturating_sub(change_bytes)
            {
                return Err(Failure::one(
                    "headless change budget exceeded (4096 changes / 32 MiB)",
                ));
            }
            change_bytes += pending_bytes;
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
                if !preloaded {
                    poll_command(commands, &command).map_err(Failure::one)?;
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
                if save_here {
                    save_checkpoint(runner, resources.save.as_ref().expect("save configured"))?;
                    saved = true;
                    break;
                }
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
        if save_here {
            save_checkpoint(runner, resources.save.as_ref().expect("save configured"))?;
            saved = true;
            break;
        }
    }
    if !ended && !saved {
        return Err(Failure::one("headless event budget (10000) exceeded"));
    }
    if !saved && choices.next().is_some() {
        return Err(Failure::one("unused headless choices after dialogue ended"));
    }
    let state = state
        .read()
        .map_err(|_| Failure::one("state lock is poisoned"))?;
    let values = state
        .iter()
        .map(|(key, value)| (key.to_string(), value.clone()))
        .collect();
    if !saved && save_at.is_some() {
        return Err(Failure::one(
            "requested checkpoint was not reached at an eligible display",
        ));
    }
    Ok((values, changes, saved))
}

fn save_checkpoint(
    runner: &DialogueRunner,
    (context, store): &(weftora_save::Context, weftora_save::storage::Store),
) -> Result<(), Failure> {
    let snapshot = runner
        .save_point()
        .map_err(|e| Failure::one(e.to_string()))?;
    let game =
        weftora_save::SavedGame::new(context, snapshot).map_err(|e| Failure::one(e.to_string()))?;
    store.write(&game).map_err(|e| Failure::one(e.to_string()))
}

fn poll_command(
    commands: &CommandRegistry,
    command: &weftora_api::StoryCommand,
) -> Result<(), String> {
    let arguments = command.args.iter().cloned().map(Value::String).collect();
    let mut task = commands
        .start(&command.name, arguments)
        .map_err(|e| e.to_string())?;
    match task.poll(&mut Context::from_waker(Waker::noop())) {
        Poll::Ready(result) => result.map(|_| ()).map_err(|error| error.to_string()),
        Poll::Pending => {
            task.cancel();
            Err("presentation command did not complete in headless mode".into())
        }
    }
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
            options: Options::default(),
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
