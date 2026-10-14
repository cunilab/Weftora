//! Yarn Spinner adapter for Weftora (architecture §6).
//!
//! Uses the Bevy-independent `yarnspinner` crates only. Yarn variables are a view onto the canonical
//! [`SharedState`]; there is no separate Yarn variable store.
//!
//! Variable mapping (provisional, Phase 0): Yarn `$name` ↔ state key `name`, with `__` in the Yarn name
//! standing for `.` in the key (`$world__day` ↔ `world.day`). `bool` and `string` map directly. `Float`
//! maps to Yarn's `f32` number. `Int` maps to number only within ±2^24; reading a larger `Int` is an error.
//! A Yarn number written to a key that holds an `Int` must be integral, otherwise it is a type error.

use std::collections::{HashMap, VecDeque};

use weftora_api::Value;
use weftora_api::{Diagnostic, StoryCommand};
use weftora_core::{SharedState, State};
use weftora_story::DialogueSource;
use yarnspinner::compiler::{Compiler, DiagnosticSeverity, File as YarnFile};
use yarnspinner::core::{Program, YarnValue};
use yarnspinner::runtime::{Dialogue, OptionId, StringTableTextProvider};

mod storage;

use storage::StateStorage;

/// Commands are processed immediately, so they must not sustain execution forever.
const MAX_CONSECUTIVE_COMMANDS: usize = 1_000;
const MAX_CONSECUTIVE_CONTINUES: usize = 1_000;
const MAX_INSTRUCTIONS_BEFORE_BLOCKING: usize = 100_000;

/// A static use of a Yarn command, found by scanning the compiled program.
#[derive(Debug, Clone, PartialEq, Eq)]
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
    strings: HashMap<yarnspinner::core::LineId, String>,
    sites: Vec<CommandSite>,
}

impl std::fmt::Debug for CompiledDialogue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CompiledDialogue").finish_non_exhaustive()
    }
}

impl CompiledDialogue {
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

/// `OpCode::RunCommand` in `yarnspinner_core` (the enum is not re-exported by `yarnspinner`).
const OPCODE_RUN_COMMAND: i32 = 3;

/// Compiles Yarn sources. Errors carry the Story-relative file and line.
pub fn compile(sources: &[DialogueSource]) -> Result<CompiledDialogue, Vec<Diagnostic>> {
    let mut compiler = Compiler::new();
    for source in sources {
        compiler.add_file(YarnFile {
            file_name: source.file.clone(),
            source: source.contents.clone(),
        });
    }
    // The Rust port's parser can panic on some malformed input (for example `<<set $x = >>`);
    // treat that as a compile failure rather than aborting the tool.
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| compiler.compile()));
    let Ok(outcome) = outcome else {
        return Err(vec![Diagnostic::new(
            sources.first().map(|s| s.file.as_str()).unwrap_or_default(),
            None,
            "the Yarn compiler crashed on malformed input; check the dialogue syntax",
        )]);
    };
    let compilation = match outcome {
        Ok(c) => c,
        Err(e) => {
            let diags: Vec<Diagnostic> =
                e.0.iter()
                    .filter(|d| d.severity == DiagnosticSeverity::Error)
                    .map(|d| {
                        let line = d
                            .range
                            .as_ref()
                            .map(|r| r.start.line)
                            .unwrap_or(d.start_line);
                        Diagnostic::new(
                            d.file_name.clone().unwrap_or_default(),
                            u32::try_from(line + 1).ok(),
                            d.message.clone(),
                        )
                    })
                    .collect();
            return Err(if diags.is_empty() {
                vec![Diagnostic::new("", None, "Yarn compilation failed")]
            } else {
                diags
            });
        }
    };
    let Some(program) = compilation.program else {
        return Err(vec![Diagnostic::new(
            "",
            None,
            "Yarn compilation produced no program",
        )]);
    };

    let strings = compilation
        .string_table
        .into_iter()
        .map(|(id, info)| (id, info.text))
        .collect();

    let mut sites = Vec::new();
    for (name, node) in &program.nodes {
        let debug = compilation.debug_info.get(name);
        for (i, instruction) in node.instructions.iter().enumerate() {
            if instruction.opcode != OPCODE_RUN_COMMAND {
                continue;
            }
            let text: String = instruction.read_operand(0);
            let mut parts = split_command(&text);
            if parts.is_empty() {
                continue;
            }
            let command = StoryCommand {
                name: parts.remove(0),
                args: parts,
            };
            let info = debug.and_then(|d| d.try_get_line_info(i));
            let line = info
                .as_ref()
                .and_then(|l| l.position)
                .and_then(|p| u32::try_from(p.line + 1).ok());
            sites.push(CommandSite {
                file: debug.map(|d| d.file_name.clone()).unwrap_or_default(),
                line,
                node: name.clone(),
                command,
            });
        }
    }

    Ok(CompiledDialogue {
        program,
        strings,
        sites,
    })
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
        if let Value::Int(value) = value
            && !(-storage::MAX_EXACT_INT..=storage::MAX_EXACT_INT).contains(value)
        {
            return Some(DialogueError::StatePrecision {
                key: key.to_string(),
                variable,
                value: *value,
            });
        }
        None
    }));
    errors
}

/// Splits command text on whitespace, honouring double quotes and `\"` / `\\` escapes inside them.
fn split_command(input: &str) -> Vec<String> {
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
    state: SharedState,
    program: Program,
    failure: Option<String>,
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
        text.extend_base_language(dialogue.strings.clone());
        let storage = StateStorage::new(state.clone());
        let mut yarn = Dialogue::new(Box::new(storage), Box::new(text));
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
            state,
            program: dialogue.program.clone(),
            failure: default_error.or_else(|| {
                initialization
                    .err()
                    .map(|_| "Yarn runtime panicked during initialization".into())
            }),
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
        self.current_node = node.to_string();
        self.reset_progress_budget();
        self.started = true;
        self.ended = false;
        Ok(())
    }

    /// Returns the next event. After [`DialogueEvent::End`] it keeps returning `End` until `start` is called.
    pub fn next_event(&mut self) -> Result<DialogueEvent, DialogueError> {
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
        let result = self.runtime_call(|dialogue| {
            dialogue.continue_().map_err(|error| match error {
                yarnspinner::runtime::DialogueError::InstructionBudgetExceeded {
                    node_name,
                    ..
                } if remaining < per_call_budget => {
                    yarnspinner::runtime::DialogueError::InstructionBudgetExceeded {
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
                yarnspinner::runtime::DialogueEvent::Line(line) => {
                    self.queue.push_back(DialogueEvent::Line {
                        character: line.character_name().map(str::to_string),
                        text: line.text_without_character_name(),
                    });
                }
                yarnspinner::runtime::DialogueEvent::Options(options) => {
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
                yarnspinner::runtime::DialogueEvent::Command(command) => {
                    let mut parts = split_command(&command.raw);
                    if parts.is_empty() {
                        continue;
                    }
                    self.queue.push_back(DialogueEvent::Command(StoryCommand {
                        name: parts.remove(0),
                        args: parts,
                    }));
                }
                yarnspinner::runtime::DialogueEvent::DialogueComplete => {
                    self.ended = true;
                    self.queue.push_back(DialogueEvent::End);
                }
                yarnspinner::runtime::DialogueEvent::NodeStart(node) => {
                    self.current_node = node;
                }
                yarnspinner::runtime::DialogueEvent::NodeComplete(_)
                | yarnspinner::runtime::DialogueEvent::LineHints(_) => {}
            }
        }
        match self.queue.pop_front() {
            Some(event) => self.return_event(event),
            None => self.fail(DialogueError::Runtime("dialogue produced no event".into())),
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
        self.runtime_call(|dialogue| dialogue.set_selected_option(OptionId(index)).map(|_| ()))?;
        self.pending_choices = None;
        Ok(())
    }

    fn reset_progress_budget(&mut self) {
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
        self.failure = Some(error.to_string());
        self.queue.clear();
        self.pending_choices = None;
        Err(error)
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
        call: impl FnOnce(&mut Dialogue) -> Result<T, yarnspinner::runtime::DialogueError>,
    ) -> Result<T, DialogueError> {
        self.ensure_healthy()?;
        let result =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| call(&mut self.dialogue)));
        let error = match result {
            Ok(Ok(value)) => return Ok(value),
            Ok(Err(error)) => map_dialogue_error(error),
            Err(_) => DialogueError::Runtime("Yarn runtime panicked; runner has failed".into()),
        };
        self.fail(error)
    }
}

fn map_dialogue_error(error: yarnspinner::runtime::DialogueError) -> DialogueError {
    use yarnspinner::runtime::DialogueError as Y;
    match error {
        Y::InvalidNode { node_name } => DialogueError::MissingNode(node_name),
        Y::VariableStorageError(e) => storage::map_storage_error(e),
        other => DialogueError::Runtime(other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, RwLock};

    #[test]
    fn configurable_budget_resets_between_continue_calls() {
        let compiled = compile(&[DialogueSource {
            file: "test.yarn".into(),
            contents: "title: Start\n---\nFirst\nSecond\n===\n".into(),
        }])
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
        let compiled = compile(&[DialogueSource {
            file: "test.yarn".into(),
            contents: "title: Start\n---\n<<set $n = 1>>\n===\n".into(),
        }])
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
