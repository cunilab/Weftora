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

use weftora_api::{Diagnostic, StoryCommand};
use weftora_core::SharedState;
use weftora_story::DialogueSource;
use yarnspinner::compiler::{Compiler, DiagnosticSeverity, File as YarnFile};
use yarnspinner::core::Program;
use yarnspinner::runtime::{Dialogue, OptionId, StringTableTextProvider};

mod storage;

use storage::StateStorage;

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
        let storage = StateStorage::new(state);
        let mut yarn = Dialogue::new(Box::new(storage), Box::new(text));
        yarn.replace_program(dialogue.program.clone());
        Self {
            dialogue: yarn,
            queue: VecDeque::new(),
            pending_choices: None,
            started: false,
            ended: false,
        }
    }

    /// Starts (or restarts) the dialogue at `node`.
    pub fn start(&mut self, node: &str) -> Result<(), DialogueError> {
        if !self.dialogue.node_exists(node) {
            return Err(DialogueError::MissingNode(node.to_string()));
        }
        if self.dialogue.is_active() {
            self.dialogue.stop();
        }
        self.dialogue.set_node(node).map_err(map_dialogue_error)?;
        self.queue.clear();
        self.pending_choices = None;
        self.started = true;
        self.ended = false;
        Ok(())
    }

    /// Returns the next event. After [`DialogueEvent::End`] it keeps returning `End` until `start` is called.
    pub fn next_event(&mut self) -> Result<DialogueEvent, DialogueError> {
        if let Some(event) = self.queue.pop_front() {
            return Ok(event);
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
        let events = self.dialogue.continue_().map_err(map_dialogue_error)?;
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
                yarnspinner::runtime::DialogueEvent::NodeStart(_)
                | yarnspinner::runtime::DialogueEvent::NodeComplete(_)
                | yarnspinner::runtime::DialogueEvent::LineHints(_) => {}
            }
        }
        self.queue
            .pop_front()
            .ok_or_else(|| DialogueError::Runtime("dialogue produced no event".into()))
    }

    /// Selects one of the choices from the last [`DialogueEvent::Choices`].
    pub fn choose(&mut self, index: usize) -> Result<(), DialogueError> {
        let valid = self
            .pending_choices
            .as_ref()
            .is_some_and(|c| c.iter().any(|(i, available)| *i == index && *available));
        if !valid {
            return Err(DialogueError::InvalidChoice(index));
        }
        self.dialogue
            .set_selected_option(OptionId(index))
            .map_err(map_dialogue_error)?;
        self.pending_choices = None;
        Ok(())
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
