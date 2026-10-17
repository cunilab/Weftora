//! Yarn Spinner adapter for Weftora (architecture §6).
//!
//! Uses the Bevy-independent `yarnspinner` crates only. Yarn variables are a view onto the canonical
//! [`SharedState`]; there is no separate Yarn variable store.
//!
//! Variable mapping (provisional, Phase 0): Yarn `$name` ↔ state key `name`, with `__` in the Yarn name
//! standing for `.` in the key (`$world__day` ↔ `world.day`). `bool` and `string` map directly. `Float`
//! maps to Yarn's `f32` number. `Int` maps to number only within ±2^24; reading a larger `Int` is an error.
//! A Yarn number written to a key that holds an `Int` must be integral, otherwise it is a type error.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, VecDeque};

use weftora_api::StoryCommand;
use weftora_api::Value;
use weftora_core::{SharedState, State};
use yarnspinner_core::prelude::{Program, YarnValue};
use yarnspinner_runtime::prelude::{Dialogue, OptionId, StringTableTextProvider};

mod artifact;
mod checkpoint;
mod queries;
#[doc(hidden)]
pub use queries::library as collection_query_library;
mod storage;
mod validation;
pub use artifact::{
    ARTIFACT_FILE, ARTIFACT_FORMAT, COMPILER_ID, COMPILER_VERSION, MAX_ARTIFACT_BYTES, input_hash,
};
pub use checkpoint::{SaveRefusal, context as save_context};
pub use validation::{check_story, media_enabled, presentation_commands};

use storage::StateStorage;

/// Commands are processed immediately, so they must not sustain execution forever.
const MAX_CONSECUTIVE_COMMANDS: usize = 1_000;
const MAX_CONSECUTIVE_CONTINUES: usize = 1_000;
const MAX_INSTRUCTIONS_BEFORE_BLOCKING: usize = 100_000;

/// A static use of a Yarn command, found by scanning the compiled program.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandSite {
    /// Story-relative Yarn file, empty when unknown.
    pub file: String,
    /// 1-based line when known.
    pub line: Option<u32>,
    pub node: String,
    /// Commands built from expressions (`<<cmd {$x}>>`) keep a `{0}`-style placeholder in `args`.
    pub command: StoryCommand,
}

/// Dialogue compiled from a Story's Yarn files.
#[derive(Clone)]
pub struct CompiledDialogue {
    program: Program,
    strings: BTreeMap<String, String>,
    sites: Vec<CommandSite>,
    input_hash: String,
    node_files: BTreeMap<String, String>,
}

impl std::fmt::Debug for CompiledDialogue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CompiledDialogue").finish_non_exhaustive()
    }
}

impl CompiledDialogue {
    /// Compiler-only provenance; never serialized into the dialogue artifact.
    #[doc(hidden)]
    pub fn set_node_file(&mut self, node: &str, file: &str) {
        if self.program.nodes.contains_key(node) {
            self.node_files.insert(node.into(), file.into());
        }
    }
    pub(crate) fn node_file(&self, node: &str) -> &str {
        self.node_files
            .get(node)
            .map(String::as_str)
            .unwrap_or("<file unknown>")
    }

    /// Stable checkpoint IDs mapped to their validated node-entry commands.
    pub fn checkpoints(&self) -> BTreeMap<String, String> {
        self.sites
            .iter()
            .filter(|site| site.command.name == "checkpoint")
            .map(|site| (site.command.args[0].clone(), site.node.clone()))
            .collect()
    }

    /// Names of all nodes, sorted.
    pub fn node_names(&self) -> Vec<String> {
        // `Program::nodes` is a BTreeMap, so keys are already sorted.
        self.program.nodes.keys().cloned().collect()
    }

    /// Every command statically present in the compiled program, in node-name then instruction order.
    ///
    /// Found by scanning the compiled instructions for `RUN_COMMAND`, so commands reachable only at
    /// runtime through expressions appear with `{n}` placeholders in their arguments.
    pub fn commands(&self) -> Vec<StoryCommand> {
        self.sites.iter().map(|s| s.command.clone()).collect()
    }

    /// Like [`commands`](Self::commands) but with file, line and node for diagnostics.
    pub fn command_sites(&self) -> &[CommandSite] {
        &self.sites
    }
}

/// Diagnostic-only header lookup; unknown/outside-node errors stay explicitly unknown.
#[doc(hidden)]
pub fn source_node_at_line(source: &weftora_story::DialogueSource, line: u32) -> Option<&str> {
    let mut title = None;
    let mut body = false;
    for (index, text) in source.contents.lines().enumerate() {
        if index as u64 + 1 > u64::from(line) {
            break;
        }
        let text = text.trim_start_matches('\u{FEFF}').trim();
        if text == "===" {
            if index as u64 + 1 == u64::from(line) {
                break;
            }
            title = None;
            body = false;
        } else if text == "---" {
            body = true;
        } else if !body && let Some(value) = text.strip_prefix("title:") {
            title = Some(value.trim());
        }
    }
    title.filter(|s| !s.is_empty())
}

/// Checks canonical state values against the compiled Yarn variable types.
/// Missing values use Yarn defaults; ints and floats both match Yarn numbers.
/// Ints mapped to declared numbers must be within ±2^24.
pub fn check_state(dialogue: &CompiledDialogue, state: &State) -> Vec<DialogueError> {
    check_program_state(&dialogue.program, state)
}

fn check_defaults(program: &Program, state: &State) -> Vec<DialogueError> {
    let mut defaults: Vec<_> = program.initial_values.iter().collect();
    defaults.sort_by_key(|(name, _)| *name);
    defaults
        .into_iter()
        .filter_map(|(name, operand)| {
            storage::validate_default(name, &operand.clone().into(), state).err()
        })
        .collect()
}

fn check_program_state(program: &Program, state: &State) -> Vec<DialogueError> {
    let mut errors = check_defaults(program, state);
    errors.extend(state.iter().filter_map(|(key, value)| {
        let variable = format!("${}", key.replace('.', "__"));
        let declared: YarnValue = program.initial_values.get(&variable)?.clone().into();
        let expected = match declared {
            YarnValue::Boolean(_) => "bool",
            YarnValue::String(_) => "string",
            YarnValue::Number(_) => "number",
        };
        let matches = matches!(
            (value, expected),
            (Value::Bool(_), "bool")
                | (Value::String(_), "string")
                | (Value::Int(_) | Value::Float(_), "number")
        );
        if !matches {
            return Some(DialogueError::StateTypeMismatch {
                key: key.to_string(),
                variable,
                actual: value.type_name(),
                expected,
            });
        }
        if let Value::String(text) = value
            && text.len() > yarnspinner_runtime::resource_limits::MAX_STRING_BYTES
        {
            return Some(DialogueError::Variable {
                variable,
                message: "string budget (16384 bytes) exceeded".into(),
            });
        }
        if let Value::Int(value) = value
            && !(-storage::MAX_EXACT_INT..=storage::MAX_EXACT_INT).contains(value)
        {
            return Some(DialogueError::StatePrecision {
                key: key.to_string(),
                variable,
                value: *value,
            });
        }
        if let Value::Float(value) = value
            && !(*value as f32).is_finite()
        {
            return Some(DialogueError::Variable {
                variable,
                message: format!("float {value} does not fit Yarn's f32"),
            });
        }
        None
    }));
    errors
}

/// Splits command text on whitespace, honouring double quotes and `\"` / `\\` escapes inside them.
#[doc(hidden)]
pub fn split_command(input: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        if c.is_whitespace() {
            if !current.is_empty() {
                parts.push(std::mem::take(&mut current));
            }
        } else if c == '"' {
            while let Some(c) = chars.next() {
                match c {
                    '"' => break,
                    '\\' if matches!(chars.peek(), Some('"' | '\\')) => {
                        current.extend(chars.next());
                    }
                    _ => current.push(c),
                }
            }
            parts.push(std::mem::take(&mut current));
        } else {
            current.push(c);
        }
    }
    if !current.is_empty() {
        parts.push(current);
    }
    parts
}

/// One thing the player must present or do.
#[derive(Debug, Clone, PartialEq)]
pub enum DialogueEvent {
    /// Show a line; the player waits for the user before calling [`DialogueRunner::next_event`] again.
    Line {
        character: Option<String>,
        text: String,
    },
    /// Show choices; the player calls [`DialogueRunner::choose`] with one `index`, then `next_event`.
    Choices(Vec<Choice>),
    /// Execute a Story command (e.g. `background bg.room`), then call `next_event` again immediately.
    Command(StoryCommand),
    /// The dialogue has finished.
    End,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Choice {
    pub index: usize,
    pub text: String,
    /// `false` when the option's Yarn condition failed; the player shows it disabled or hides it.
    pub available: bool,
}

/// Runtime errors from dialogue execution.
#[derive(Debug, thiserror::Error)]
pub enum DialogueError {
    #[error("initialState `{key}` is a {actual} but Yarn declares {variable} as {expected}")]
    StateTypeMismatch {
        key: String,
        variable: String,
        actual: &'static str,
        expected: &'static str,
    },
    #[error(
        "initialState `{key}` is an int ({value}) outside ±16777216 and cannot be read exactly by Yarn {variable}"
    )]
    StatePrecision {
        key: String,
        variable: String,
        value: i64,
    },
    #[error("dialogue node `{0}` does not exist")]
    MissingNode(String),
    #[error("choice {0} is not one of the offered choices")]
    InvalidChoice(usize),
    #[error("Yarn variable `{variable}`: {message}")]
    Variable { variable: String, message: String },
    #[error("dialogue error: {0}")]
    Runtime(String),
}

/// Runs compiled dialogue against canonical state, one presentation event at a time.
pub struct DialogueRunner {
    dialogue: Dialogue,
    storage: StateStorage,
    capture: Option<checkpoint::Capture>,
    state: SharedState,
    rules: weftora_story::rules::Rules,
    scripts: weftora_script::Programs,
    query_persistent_only: bool,
    program: Program,
    failure: Option<String>,
    node_files: BTreeMap<String, String>,
    writes: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    write_bytes: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    current_node: String,
    consecutive_commands: usize,
    consecutive_continues: usize,
    instructions_since_blocking: usize,
    queue: VecDeque<DialogueEvent>,
    /// `(id, available)` of the choices offered by the last `Choices` event that was not yet answered.
    pending_choices: Option<Vec<(usize, bool)>>,
    started: bool,
    ended: bool,
}

impl std::fmt::Debug for DialogueRunner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DialogueRunner").finish_non_exhaustive()
    }
}

impl DialogueRunner {
    pub fn new(dialogue: &CompiledDialogue, state: SharedState) -> Self {
        let mut text = StringTableTextProvider::new();
        text.extend_base_language(dialogue.strings.iter().map(|(id, text)| {
            (
                yarnspinner_core::prelude::LineId::from(id.clone()),
                text.clone(),
            )
        }));
        let storage = StateStorage::new(state.clone());
        let (writes, write_bytes) = storage.write_counters();
        let mut yarn = Dialogue::new(Box::new(storage.clone()), Box::new(text));
        yarn.library_mut().extend(queries::library());
        let default_error = match state.read() {
            Ok(state) => check_defaults(&dialogue.program, &state)
                .into_iter()
                .next()
                .map(|error| error.to_string()),
            Err(_) => Some("state lock is poisoned".into()),
        };
        let initialization = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            if default_error.is_none() {
                yarn.replace_program(dialogue.program.clone());
            }
        }));
        Self {
            dialogue: yarn,
            storage,
            capture: None,
            state,
            rules: weftora_story::rules::Rules::default(),
            scripts: weftora_script::Programs::default(),
            query_persistent_only: false,
            program: dialogue.program.clone(),
            failure: default_error.or_else(|| {
                initialization
                    .err()
                    .map(|_| "Yarn runtime panicked during initialization".into())
            }),
            node_files: dialogue.node_files.clone(),
            writes,
            write_bytes,
            current_node: String::new(),
            consecutive_commands: 0,
            consecutive_continues: 0,
            instructions_since_blocking: 0,
            queue: VecDeque::new(),
            pending_choices: None,
            started: false,
            ended: false,
        }
    }

    /// Creates a Story-aware runner, including its validated rules. Hosts use this for play and restore.
    pub fn for_story(
        dialogue: &CompiledDialogue,
        state: SharedState,
        story: &weftora_story::Story,
    ) -> Self {
        let mut runner = Self::new(dialogue, state);
        runner.rules = story.rules().clone();
        runner.query_persistent_only = story.manifest().save_schema.is_some();
        if let Some(error) = queries::validate(story, dialogue).first() {
            runner.failure.get_or_insert_with(|| error.to_string());
        }
        match weftora_script::Programs::compile(runner.rules.scripts()) {
            Ok(scripts) => runner.scripts = scripts,
            Err(errors) => {
                runner.failure = Some(
                    errors
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join("\n"),
                )
            }
        }
        runner
    }

    /// Starts (or restarts) the dialogue at `node`.
    pub fn start(&mut self, node: &str) -> Result<(), DialogueError> {
        self.ensure_healthy()?;
        {
            let state = self
                .state
                .read()
                .map_err(|_| DialogueError::Runtime("state lock is poisoned".into()))?;
            if let Some(error) = check_program_state(&self.program, &state)
                .into_iter()
                .next()
            {
                return Err(error);
            }
        }
        if !self.dialogue.node_exists(node) {
            return Err(DialogueError::MissingNode(node.to_string()));
        }
        self.runtime_call(|dialogue| {
            if dialogue.is_active() {
                dialogue.stop();
            }
            dialogue.set_node(node).map(|_| ())
        })?;
        self.queue.clear();
        self.pending_choices = None;
        self.capture = None;
        self.current_node = node.to_string();
        self.reset_progress_budget();
        self.started = true;
        self.ended = false;
        Ok(())
    }

    /// Returns the next event. After [`DialogueEvent::End`] it keeps returning `End` until `start` is called.
    pub fn next_event(&mut self) -> Result<DialogueEvent, DialogueError> {
        if self.pending_choices.is_none() {
            self.disarm_after_progress();
        }
        loop {
            let from = if self.rules.is_empty() {
                0
            } else {
                self.state
                    .read()
                    .map_err(|_| DialogueError::Runtime("state lock is poisoned".into()))?
                    .pending_changes()
                    .len()
            };
            let event = self.next_event_one()?;
            let story_command = match &event {
                DialogueEvent::Command(command) if self.rules.has_command(&command.name) => {
                    Some(command)
                }
                _ => None,
            };
            if !self.rules.is_empty() {
                let settled = self
                    .state
                    .write()
                    .map_err(|_| "state lock is poisoned".to_string())
                    .and_then(|mut state| {
                        let after_vm = state.pending_changes().len();
                        if let Some(command) = story_command {
                            let args = self
                                .rules
                                .parse_command(&command.name, &command.args, false)
                                .map_err(|e| e.to_string())?;
                            self.rules
                                .call_with(
                                    &command.name,
                                    &args,
                                    &mut state,
                                    from,
                                    &mut self.scripts,
                                )
                                .map_err(|e| e.to_string())?;
                        } else {
                            self.rules
                                .settle_with(&mut state, from, &mut self.scripts)
                                .map_err(|e| e.to_string())?;
                        }
                        self.storage
                            .charge_rule_changes(&state.pending_changes()[after_vm..])
                            .map_err(|e| e.to_string())
                    });
                if let Err(error) = settled {
                    return self.fail_with_context(DialogueError::Runtime(error));
                }
            }
            if story_command.is_some() {
                continue;
            }
            if let DialogueEvent::Command(command) = &event
                && command.name == "checkpoint"
            {
                self.capture_checkpoint(&command.args[0]);
                continue;
            }
            self.arm_checkpoint(&event);
            return Ok(event);
        }
    }

    /// Apply a screen action through the same bounded rule bridge as Yarn commands.
    pub fn ui_action(&mut self, action: &weftora_story::ui::UiAction) -> Result<(), DialogueError> {
        self.ensure_healthy()?;
        let result = (|| {
            let mut state = self
                .state
                .write()
                .map_err(|_| "state lock is poisoned".to_string())?;
            let from = state.pending_changes().len();
            match action {
                weftora_story::ui::UiAction::SetState { key, value } => {
                    state.get(key).map_err(|e| e.to_string())?;
                    state.set(key, value.clone()).map_err(|e| e.to_string())?;
                    self.rules
                        .settle_with(&mut state, from, &mut self.scripts)
                        .map_err(|e| e.to_string())?;
                }
                weftora_story::ui::UiAction::CallCommand { command, args } => {
                    self.rules
                        .call_with(command, args, &mut state, from, &mut self.scripts)
                        .map_err(|e| e.to_string())?;
                }
                _ => return Err("expected a state or command UI action".into()),
            }
            // Each user action has its own bounded dispatch/notification budget.
            // UI inputs are absent from checkpoint replay, including changes later reversed by rules.
            if state.pending_changes()[from..]
                .iter()
                .any(|change| state.scope(&change.key) == Ok(weftora_core::Scope::Persistent))
            {
                self.capture = None;
            }
            Ok(())
        })();
        match result {
            Ok(()) => Ok(()),
            Err(error) => self.fail_with_context(DialogueError::Runtime(error)),
        }
    }

    fn next_event_one(&mut self) -> Result<DialogueEvent, DialogueError> {
        self.ensure_healthy()?;
        if let Some(event) = self.queue.pop_front() {
            return self.return_event(event);
        }
        if !self.started {
            return Err(DialogueError::Runtime(
                "dialogue has not been started".into(),
            ));
        }
        if self.ended {
            return Ok(DialogueEvent::End);
        }
        if self.pending_choices.is_some() {
            return Err(DialogueError::Runtime(
                "a choice must be made before continuing".into(),
            ));
        }
        let remaining = MAX_INSTRUCTIONS_BEFORE_BLOCKING - self.instructions_since_blocking;
        let per_call_budget = self.dialogue.instruction_budget();
        self.dialogue
            .set_instruction_budget(remaining.min(per_call_budget));
        let query_state = self.state.clone();
        let persistent_only = self.query_persistent_only;
        let result = self.runtime_call(|dialogue| {
            dialogue
                .continue_with_function_handler(|name, args| {
                    queries::evaluate(name, args, &query_state, persistent_only)
                })
                .map_err(|error| match error {
                    yarnspinner_runtime::prelude::DialogueError::InstructionBudgetExceeded {
                        node_name,
                        ..
                    } if remaining < per_call_budget => {
                        yarnspinner_runtime::prelude::DialogueError::InstructionBudgetExceeded {
                            node_name,
                            budget: MAX_INSTRUCTIONS_BEFORE_BLOCKING,
                        }
                    }
                    other => other,
                })
        });
        self.dialogue.set_instruction_budget(per_call_budget);
        self.instructions_since_blocking += self.dialogue.last_instruction_count();
        self.consecutive_continues += 1;
        let events = result?;
        for event in events {
            match event {
                yarnspinner_runtime::prelude::DialogueEvent::Line(line) => {
                    self.queue.push_back(DialogueEvent::Line {
                        character: line.character_name().map(str::to_string),
                        text: line.text_without_character_name(),
                    });
                }
                yarnspinner_runtime::prelude::DialogueEvent::Options(options) => {
                    let choices: Vec<Choice> = options
                        .iter()
                        .map(|o| Choice {
                            index: o.id.0,
                            text: o.line.text_without_character_name(),
                            available: o.is_available,
                        })
                        .collect();
                    self.pending_choices =
                        Some(choices.iter().map(|c| (c.index, c.available)).collect());
                    self.queue.push_back(DialogueEvent::Choices(choices));
                }
                yarnspinner_runtime::prelude::DialogueEvent::Command(command) => {
                    let mut parts = split_command(&command.raw);
                    if parts.is_empty() {
                        continue;
                    }
                    self.queue.push_back(DialogueEvent::Command(StoryCommand {
                        name: parts.remove(0),
                        args: parts,
                    }));
                }
                yarnspinner_runtime::prelude::DialogueEvent::DialogueComplete => {
                    self.ended = true;
                    self.queue.push_back(DialogueEvent::End);
                }
                yarnspinner_runtime::prelude::DialogueEvent::NodeStart(node) => {
                    self.current_node = node;
                }
                yarnspinner_runtime::prelude::DialogueEvent::NodeComplete(_)
                | yarnspinner_runtime::prelude::DialogueEvent::LineHints(_) => {}
            }
        }
        match self.queue.pop_front() {
            Some(event) => self.return_event(event),
            None => {
                self.fail_with_context(DialogueError::Runtime("dialogue produced no event".into()))
            }
        }
    }

    /// Selects one of the choices from the last [`DialogueEvent::Choices`].
    pub fn choose(&mut self, index: usize) -> Result<(), DialogueError> {
        self.ensure_healthy()?;
        let valid = self
            .pending_choices
            .as_ref()
            .is_some_and(|c| c.iter().any(|(i, available)| *i == index && *available));
        if !valid {
            return Err(DialogueError::InvalidChoice(index));
        }
        self.capture = None;
        self.runtime_call(|dialogue| dialogue.set_selected_option(OptionId(index)).map(|_| ()))?;
        self.pending_choices = None;
        Ok(())
    }

    fn reset_progress_budget(&mut self) {
        self.writes.store(0, std::sync::atomic::Ordering::Relaxed);
        self.write_bytes
            .store(0, std::sync::atomic::Ordering::Relaxed);
        self.consecutive_commands = 0;
        self.consecutive_continues = 0;
        self.instructions_since_blocking = 0;
    }

    fn return_event(&mut self, event: DialogueEvent) -> Result<DialogueEvent, DialogueError> {
        match &event {
            DialogueEvent::Command(_) => {
                self.consecutive_commands += 1;
                let exceeded = if self.consecutive_commands > MAX_CONSECUTIVE_COMMANDS {
                    Some(format!(
                        "consecutive command budget ({MAX_CONSECUTIVE_COMMANDS})"
                    ))
                } else if self.consecutive_continues > MAX_CONSECUTIVE_CONTINUES {
                    Some(format!(
                        "consecutive continue budget ({MAX_CONSECUTIVE_CONTINUES})"
                    ))
                } else {
                    None
                };
                if let Some(budget) = exceeded {
                    return self.fail(DialogueError::Runtime(format!(
                        "{budget} exceeded in node `{}`",
                        self.current_node
                    )));
                }
            }
            DialogueEvent::Line { .. } | DialogueEvent::Choices(_) | DialogueEvent::End => {
                self.reset_progress_budget();
            }
        }
        Ok(event)
    }

    fn fail<T>(&mut self, error: DialogueError) -> Result<T, DialogueError> {
        self.capture = None;
        self.failure = Some(error.to_string());
        self.queue.clear();
        self.pending_choices = None;
        Err(error)
    }

    fn fail_with_context<T>(&mut self, error: DialogueError) -> Result<T, DialogueError> {
        let file = self
            .node_files
            .get(&self.current_node)
            .map(String::as_str)
            .unwrap_or("<file unknown>");
        let context = format!("{file} (node '{}')", self.current_node);
        let error = match error {
            DialogueError::Variable { variable, message } => DialogueError::Variable {
                variable,
                message: format!("{context}: {message}"),
            },
            other => DialogueError::Runtime(format!("{context}: {other}")),
        };
        self.fail(error)
    }

    fn ensure_healthy(&self) -> Result<(), DialogueError> {
        match &self.failure {
            Some(message) => Err(DialogueError::Runtime(message.clone())),
            None => Ok(()),
        }
    }

    // A VM error may leave its stack/program counter partly changed. Never resume it.
    fn runtime_call<T>(
        &mut self,
        call: impl FnOnce(&mut Dialogue) -> Result<T, yarnspinner_runtime::prelude::DialogueError>,
    ) -> Result<T, DialogueError> {
        self.ensure_healthy()?;
        let result =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| call(&mut self.dialogue)));
        let error = match result {
            Ok(Ok(value)) => return Ok(value),
            Ok(Err(error)) => {
                match &error {
                    yarnspinner_runtime::prelude::DialogueError::ResourceBudgetExceeded {
                        node_name,
                        ..
                    }
                    | yarnspinner_runtime::prelude::DialogueError::InvalidExecution {
                        node_name,
                        ..
                    }
                    | yarnspinner_runtime::prelude::DialogueError::ExecutionError {
                        node_name,
                        ..
                    }
                    | yarnspinner_runtime::prelude::DialogueError::InstructionBudgetExceeded {
                        node_name,
                        ..
                    } => self.current_node = node_name.clone(),
                    _ => {}
                }
                map_dialogue_error(error)
            }
            Err(payload) => {
                if let Some(node) = self.dialogue.current_node() {
                    self.current_node = node;
                }
                let message = payload
                    .downcast_ref::<String>()
                    .map(String::as_str)
                    .or_else(|| payload.downcast_ref::<&str>().copied())
                    .unwrap_or("unknown panic");
                DialogueError::Runtime(format!(
                    "Yarn runtime panicked; runner has failed: {message}"
                ))
            }
        };
        self.fail_with_context(error)
    }
}

fn map_dialogue_error(error: yarnspinner_runtime::prelude::DialogueError) -> DialogueError {
    use yarnspinner_runtime::prelude::DialogueError as Y;
    match error {
        Y::InvalidNode { node_name } => DialogueError::MissingNode(node_name),
        Y::VariableStorageError(e) => storage::map_storage_error(e),
        Y::ExecutionError { source, .. } => map_dialogue_error(*source),
        other => DialogueError::Runtime(other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, RwLock};

    #[test]
    fn configurable_budget_resets_between_continue_calls() {
        let compiled =
            CompiledDialogue::from_artifact_bytes(include_bytes!("../tests/fixtures/budget.json"))
                .unwrap();
        let mut runner = DialogueRunner::new(&compiled, Arc::new(RwLock::new(State::new())));
        runner.dialogue.set_instruction_budget(1);
        runner.start("Start").unwrap();
        assert!(matches!(
            runner.next_event().unwrap(),
            DialogueEvent::Line { .. }
        ));
        assert!(matches!(
            runner.next_event().unwrap(),
            DialogueEvent::Line { .. }
        ));
        assert_eq!(runner.next_event().unwrap(), DialogueEvent::End);
    }

    #[test]
    fn zero_budget_executes_no_instruction() {
        let compiled =
            CompiledDialogue::from_artifact_bytes(include_bytes!("../tests/fixtures/budget.json"))
                .unwrap();
        let state = Arc::new(RwLock::new(State::new()));
        let mut runner = DialogueRunner::new(&compiled, state.clone());
        runner.dialogue.set_instruction_budget(0);
        runner.start("Start").unwrap();
        assert!(
            runner
                .next_event()
                .unwrap_err()
                .to_string()
                .contains("instruction budget (0)")
        );
        assert_ne!(state.read().unwrap().get("n"), Ok(&Value::Float(1.0)));
    }
}

#[cfg(test)]
mod resource_tests;
