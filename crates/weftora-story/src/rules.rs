//! JSONC Story rules and the notification boundary shared by dialogue hosts.
use crate::{MANIFEST_FILE, Manifest, jsonc, paths, resolve_in_root};
use serde::Deserialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
    sync::{Arc, Mutex},
};
use weftora_api::{Diagnostic, ScriptDefinition, Value};
use weftora_core::{Scope, State, catalog::Catalog, command::ValueType, signal::Signal};
use weftora_flow::{
    CompareOp, Condition, MAX_DEPTH, MAX_NODES, Operand,
    events::{
        Action, Budget, CommandExecutor, Event, FlowError, MAX_ACTIONS, MAX_EVENTS, NoCommands,
        Scheduler,
    },
};

mod trace;
pub use trace::RuleTrace;

pub const DISPATCH_BUDGET: usize = 1024;
const CHANGED: &str = "state.changed";
const CHANGED_KEY: &str = "state.changed.";

#[derive(Debug, Default)]
pub struct Rules {
    scheduler: Arc<Scheduler>,
    sources: Arc<BTreeMap<String, (String, String)>>,
    action_sources: Arc<BTreeMap<String, Vec<Location>>>,
    commands: Arc<BTreeMap<String, Command>>,
    scripts: Arc<Vec<ScriptDefinition>>,
    trace: Option<Mutex<RuleTrace>>,
}

impl Clone for Rules {
    fn clone(&self) -> Self {
        Self {
            scheduler: self.scheduler.clone(),
            sources: self.sources.clone(),
            action_sources: self.action_sources.clone(),
            commands: self.commands.clone(),
            scripts: self.scripts.clone(),
            trace: None,
        }
    }
}

impl Rules {
    pub fn is_empty(&self) -> bool {
        self.sources.is_empty() && self.commands.is_empty()
    }

    fn diagnostic(&self, error: FlowError) -> Diagnostic {
        if let Some(diagnostic) = command_diagnostic(&error) {
            return diagnostic;
        }
        let (event, suffix) = match &error {
            FlowError::Action { event, index, .. } => (Some(event), format!("/actions/{index}")),
            FlowError::Condition { event, .. } => (Some(event), "/condition".into()),
            FlowError::Budget { event, action, .. } => (
                event.as_ref(),
                action.map(|i| format!("/actions/{i}")).unwrap_or_default(),
            ),
            FlowError::Actions(event) => (Some(event), "/actions".into()),
            _ => (None, String::new()),
        };
        let source = event.and_then(|id| self.sources.get(id)).or_else(|| {
            if self.sources.len() == 1 {
                self.sources.values().next()
            } else {
                None
            }
        });
        let action = match &error {
            FlowError::Action { event, index, .. } => Some((event, *index)),
            FlowError::Budget {
                event: Some(event),
                action: Some(index),
                ..
            } => Some((event, *index)),
            _ => None,
        };
        if let Some(location) =
            action.and_then(|(event, index)| self.action_sources.get(event)?.get(index))
        {
            return location.error(error);
        }
        match source {
            Some((file, path)) => Diagnostic::new(file, None, format!("{path}{suffix}: {error}")),
            None => Diagnostic::new(MANIFEST_FILE, None, format!("/events: {error}")),
        }
    }

    /// Settle new notifications under the caller's exclusive State borrow; preserve the full queue.
    /// The starting index is captured before one VM call, which never drains this queue.
    pub fn settle(&self, state: &mut State, from: usize) -> Result<(), Diagnostic> {
        self.settle_with(state, from, &mut NoCommands)
    }

    pub fn scripts(&self) -> &[ScriptDefinition] {
        &self.scripts
    }

    pub fn settle_with(
        &self,
        state: &mut State,
        from: usize,
        executor: &mut dyn CommandExecutor,
    ) -> Result<(), Diagnostic> {
        self.settle_budgeted(state, from, &mut Budget::new(DISPATCH_BUDGET), executor)
    }

    fn settle_budgeted(
        &self,
        state: &mut State,
        from: usize,
        budget: &mut Budget,
        executor: &mut dyn CommandExecutor,
    ) -> Result<(), Diagnostic> {
        check_notifications(state, from)?;
        let mut cursor = from;
        while let Some(change) = state.pending_changes().get(cursor) {
            let key = change.key.clone();
            cursor += 1;
            for trigger in [CHANGED.to_owned(), format!("{CHANGED_KEY}{key}")] {
                if self.scheduler.has_trigger(&trigger) {
                    let result = self.scheduler.dispatch_with(
                        Signal {
                            id: trigger,
                            value: Value::String(key.clone()),
                        },
                        state,
                        budget,
                        executor,
                    );
                    self.record_trace(&result);
                    result.map_err(|error| self.diagnostic(error.error))?;
                    check_notifications(state, from)?;
                }
            }
        }
        Ok(())
    }
}

fn command_diagnostic(error: &FlowError) -> Option<Diagnostic> {
    match error {
        FlowError::Sequence(error) => command_diagnostic(error),
        FlowError::Action {
            source: weftora_flow::events::ActionError::Command(diagnostic),
            ..
        } if !diagnostic.file.is_empty() => Some(diagnostic.clone()),
        _ => None,
    }
}

fn check_notifications(state: &State, from: usize) -> Result<(), Diagnostic> {
    let changes = state.pending_changes().get(from..).ok_or_else(|| {
        Diagnostic::new(
            MANIFEST_FILE,
            None,
            "/events: notification queue was drained during a dialogue boundary",
        )
    })?;
    let mut bytes = 0usize;
    for change in changes {
        bytes = bytes.saturating_add(change.key.len());
        for value in [&change.before, &change.after].into_iter().flatten() {
            bytes = bytes.saturating_add(value.payload_bytes());
        }
        if bytes > 32 * 1024 * 1024 {
            break;
        }
    }
    if changes.len() > 4096 || bytes > 32 * 1024 * 1024 {
        return Err(Diagnostic::new(
            MANIFEST_FILE,
            None,
            "/events: rule notification budget exceeded (4096 changes / 32 MiB)",
        ));
    }
    Ok(())
}

fn value_limit(value: &Value) -> Result<(), String> {
    value.validate().map_err(|e| e.to_string())?;
    if matches!(value, Value::String(text) if text.len() > 16 * 1024) {
        return Err("rule strings must not exceed 16384 bytes".into());
    }
    Ok(())
}

fn condition_limits(condition: &Condition) -> Result<(), String> {
    match condition {
        Condition::Compare { left, right, .. } => {
            for operand in [left, right] {
                if let Operand::Literal(value) = operand {
                    value_limit(value)?;
                }
            }
        }
        Condition::And(children) | Condition::Or(children) => {
            for child in children {
                condition_limits(child)?;
            }
        }
        Condition::Not(child) => condition_limits(child)?,
    }
    Ok(())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Document {
    rules_schema: u32,
    #[serde(default)]
    events: Vec<EventDocument>,
    #[serde(default)]
    commands: Vec<CommandDocument>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EventDocument {
    id: String,
    trigger: String,
    #[serde(default)]
    priority: i32,
    condition: Option<ConditionDocument>,
    actions: Vec<ActionDocument>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
enum OperandDocument {
    Literal(Value),
    State(String),
}
impl From<OperandDocument> for Operand {
    fn from(value: OperandDocument) -> Self {
        match value {
            OperandDocument::Literal(value) => Self::Literal(value),
            OperandDocument::State(key) => Self::State(key),
        }
    }
}

#[derive(Deserialize)]
enum Operator {
    #[serde(rename = "==")]
    Eq,
    #[serde(rename = "!=")]
    Ne,
    #[serde(rename = "<")]
    Lt,
    #[serde(rename = "<=")]
    Le,
    #[serde(rename = ">")]
    Gt,
    #[serde(rename = ">=")]
    Ge,
}
impl From<Operator> for CompareOp {
    fn from(value: Operator) -> Self {
        match value {
            Operator::Eq => Self::Eq,
            Operator::Ne => Self::Ne,
            Operator::Lt => Self::Lt,
            Operator::Le => Self::Le,
            Operator::Gt => Self::Gt,
            Operator::Ge => Self::Ge,
        }
    }
}

#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "camelCase", deny_unknown_fields)]
enum ConditionDocument {
    Compare {
        operator: Operator,
        left: OperandDocument,
        right: OperandDocument,
    },
    And {
        conditions: Vec<Self>,
    },
    Or {
        conditions: Vec<Self>,
    },
    Not {
        condition: Box<Self>,
    },
}
impl ConditionDocument {
    fn build(self, depth: usize, remaining: &mut usize) -> Result<Condition, String> {
        if depth > MAX_DEPTH {
            return Err(format!("condition depth exceeds {MAX_DEPTH}"));
        }
        *remaining = remaining
            .checked_sub(1)
            .ok_or_else(|| format!("condition node count exceeds {MAX_NODES}"))?;
        Ok(match self {
            Self::Compare {
                operator,
                left,
                right,
            } => Condition::Compare {
                op: operator.into(),
                left: left.into(),
                right: right.into(),
            },
            Self::And { conditions } => Condition::And(
                conditions
                    .into_iter()
                    .map(|c| c.build(depth + 1, remaining))
                    .collect::<Result<_, _>>()?,
            ),
            Self::Or { conditions } => Condition::Or(
                conditions
                    .into_iter()
                    .map(|c| c.build(depth + 1, remaining))
                    .collect::<Result<_, _>>()?,
            ),
            Self::Not { condition } => {
                Condition::Not(Box::new(condition.build(depth + 1, remaining)?))
            }
        })
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "do", deny_unknown_fields)]
enum ActionDocument {
    #[serde(rename = "state.set")]
    Set { key: String, value: SlotValue },
    #[serde(rename = "state.add")]
    Add { key: String, amount: SlotValue },
    #[serde(rename = "state.remove")]
    Remove { key: String },
    #[serde(rename = "state.reset")]
    Reset { key: String },
    #[serde(rename = "signal.emit")]
    Emit { id: String, value: SlotValue },
    #[serde(rename = "command.call")]
    Call {
        command: String,
        #[serde(default)]
        args: Vec<SlotValue>,
    },
}

#[derive(Debug, Clone)]
struct Location {
    file: String,
    path: String,
}
impl Location {
    fn error(&self, error: impl std::fmt::Display) -> Diagnostic {
        Diagnostic::new(&self.file, None, format!("{}: {error}", self.path))
    }
}

#[derive(Debug, Clone)]
enum SlotValue {
    Literal(Value),
    Argument { argument: String },
}
impl<'de> Deserialize<'de> for SlotValue {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged, deny_unknown_fields)]
        enum Input {
            Argument { argument: String },
            Explicit { literal: Value },
            Scalar(Value),
        }
        match Input::deserialize(deserializer)? {
            Input::Argument { argument } => Ok(Self::Argument { argument }),
            Input::Explicit { literal } => Ok(Self::Literal(literal)),
            Input::Scalar(value) if !value.is_collection() => Ok(Self::Literal(value)),
            _ => Err(serde::de::Error::custom(
                "collection action values require {literal: value}; argument references require exactly {argument: name}",
            )),
        }
    }
}
impl SlotValue {
    fn bind(&self, bindings: &BTreeMap<String, Self>) -> Result<Self, String> {
        match self {
            Self::Literal(value) => {
                value_limit(value)?;
                Ok(self.clone())
            }
            Self::Argument { argument } => bindings
                .get(argument)
                .cloned()
                .ok_or_else(|| format!("unknown argument `{argument}`")),
        }
    }
    fn sample(&self, types: &BTreeMap<String, ValueType>) -> Value {
        match self {
            Self::Literal(value) => value.clone(),
            Self::Argument { argument } => exemplar(types[argument]),
        }
    }
}
fn exemplar(kind: ValueType) -> Value {
    match kind {
        ValueType::Bool => Value::Bool(false),
        ValueType::Int => Value::Int(0),
        ValueType::Float => Value::Float(0.0),
        ValueType::String => Value::String(String::new()),
    }
}
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Parameter {
    name: String,
    #[serde(rename = "type")]
    kind: ValueType,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CommandDocument {
    id: String,
    #[serde(default)]
    parameters: Vec<Parameter>,
    actions: Option<Vec<ActionDocument>>,
    script: Option<ScriptDocument>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ScriptDocument {
    file: String,
    function: String,
    random: Option<String>,
    #[serde(default)]
    reads: Vec<String>,
    #[serde(default)]
    writes: Vec<String>,
    #[serde(default)]
    signals: Vec<String>,
}
#[derive(Debug, Clone)]
struct LocatedAction {
    action: ActionDocument,
    location: Location,
}
#[derive(Debug)]
struct Command {
    parameters: Vec<Parameter>,
    actions: Vec<LocatedAction>,
    location: Location,
}
type CommandDocuments = BTreeMap<String, (CommandDocument, Location)>;

fn validate_command_definition(command: &CommandDocument) -> Result<(), String> {
    if command.actions.is_some() == command.script.is_some() {
        return Err("commands require exactly one of actions or script".into());
    }
    weftora_core::state::validate_key(&command.id).map_err(|e| e.to_string())?;
    if command.id.len() > 512
        || command
            .id
            .chars()
            .any(|c| c.is_whitespace() || matches!(c, '"' | '\\' | '{' | '}'))
    {
        return Err("command ID must be an unquoted token of at most 512 bytes".into());
    }
    if command.id == "checkpoint"
        || weftora_core::presentation::COMMANDS
            .iter()
            .any(|(id, _)| *id == command.id)
    {
        return Err(format!("command ID `{}` is reserved", command.id));
    }
    if command.parameters.len() > 16 {
        return Err("parameter count exceeds 16".into());
    }
    let mut names = BTreeSet::new();
    for parameter in &command.parameters {
        weftora_core::state::validate_key(&parameter.name).map_err(|e| e.to_string())?;
        if parameter.name.len() > 512 || !names.insert(&parameter.name) {
            return Err("parameter names must be unique and at most 512 bytes".into());
        }
    }
    Ok(())
}

fn expand(
    actions: &[ActionDocument],
    location: &Location,
    bindings: &BTreeMap<String, SlotValue>,
    types: &BTreeMap<String, ValueType>,
    commands: &CommandDocuments,
    stack: &mut Vec<String>,
    remaining: &mut usize,
) -> Result<Vec<LocatedAction>, Diagnostic> {
    if actions.len() > MAX_ACTIONS {
        return Err(location.error("action count exceeds 256"));
    }
    let mut result = Vec::new();
    for (index, action) in actions.iter().enumerate() {
        let location = Location {
            file: location.file.clone(),
            path: format!("{}/actions/{index}", location.path),
        };
        *remaining = remaining
            .checked_sub(1)
            .ok_or_else(|| location.error("command expansion work exceeds 1024"))?;
        // Reject oversized identifiers before cloning repeated command leaves.
        match action {
            ActionDocument::Set { key, .. }
            | ActionDocument::Add { key, .. }
            | ActionDocument::Remove { key }
            | ActionDocument::Reset { key }
                if key.len() > 512 =>
            {
                return Err(location.error("rule state keys must not exceed 512 bytes"));
            }
            ActionDocument::Emit { id, .. } if id.len() > 1024 => {
                return Err(location.error("signal ID exceeds 1024 bytes"));
            }
            _ => {}
        }
        let bind = |v: &SlotValue| v.bind(bindings).map_err(|e| location.error(e));
        let action = match action {
            ActionDocument::Set { key, value } => ActionDocument::Set {
                key: key.clone(),
                value: bind(value)?,
            },
            ActionDocument::Add { key, amount } => ActionDocument::Add {
                key: key.clone(),
                amount: bind(amount)?,
            },
            ActionDocument::Emit { id, value } => ActionDocument::Emit {
                id: id.clone(),
                value: bind(value)?,
            },
            ActionDocument::Call { command, args } => {
                let (definition, target) = commands
                    .get(command)
                    .ok_or_else(|| location.error(format!("unknown command `{command}`")))?;
                if definition.script.is_none() && (stack.contains(command) || stack.len() >= 16) {
                    return Err(
                        location.error("recursive command call or command depth exceeds 16")
                    );
                }
                if args.len() != definition.parameters.len() {
                    return Err(location.error(format!(
                        "command `{command}` expects {} arguments, got {}",
                        definition.parameters.len(),
                        args.len()
                    )));
                }
                let mut next = BTreeMap::new();
                for (value, parameter) in args.iter().zip(&definition.parameters) {
                    let value = bind(value)?;
                    if !parameter.kind.accepts(&value.sample(types)) {
                        return Err(location.error(format!(
                            "command `{command}` argument `{}` expected {:?}",
                            parameter.name, parameter.kind
                        )));
                    }
                    next.insert(parameter.name.clone(), value);
                }
                if definition.script.is_some() {
                    result.push(LocatedAction {
                        action: ActionDocument::Call {
                            command: command.clone(),
                            args: definition
                                .parameters
                                .iter()
                                .map(|p| next[&p.name].clone())
                                .collect(),
                        },
                        location: target.clone(),
                    });
                    if result.len() > MAX_ACTIONS {
                        return Err(location.error("expanded action count exceeds 256"));
                    }
                    continue;
                }
                stack.push(command.clone());
                let nested = expand(
                    definition.actions.as_deref().unwrap_or_default(),
                    target,
                    &next,
                    types,
                    commands,
                    stack,
                    remaining,
                )?;
                stack.pop();
                result.extend(nested);
                if result.len() > MAX_ACTIONS {
                    return Err(location.error("expanded action count exceeds 256"));
                }
                continue;
            }
            action => action.clone(),
        };
        result.push(LocatedAction {
            action,
            location: location.clone(),
        });
        if result.len() > MAX_ACTIONS {
            return Err(location.error("expanded action count exceeds 256"));
        }
    }
    Ok(result)
}
fn materialize(
    actions: &[LocatedAction],
    arguments: &BTreeMap<String, SlotValue>,
) -> Result<Vec<Action>, Diagnostic> {
    actions
        .iter()
        .map(|entry| {
            let value = |slot: &SlotValue| match slot
                .bind(arguments)
                .map_err(|e| entry.location.error(e))?
            {
                SlotValue::Literal(value) => Ok(value),
                SlotValue::Argument { .. } => Err(entry.location.error("unresolved argument")),
            };
            Ok(match &entry.action {
                ActionDocument::Set { key, value: v } => Action::Set {
                    key: key.clone(),
                    value: value(v)?,
                },
                ActionDocument::Add { key, amount } => Action::Add {
                    key: key.clone(),
                    amount: value(amount)?,
                },
                ActionDocument::Emit { id, value: v } => Action::Emit(Signal {
                    id: id.clone(),
                    value: value(v)?,
                }),
                ActionDocument::Remove { key } => Action::Remove { key: key.clone() },
                ActionDocument::Reset { key } => Action::Reset { key: key.clone() },
                ActionDocument::Call { command, args } => Action::Call {
                    command: command.clone(),
                    args: args.iter().map(value).collect::<Result<_, _>>()?,
                },
            })
        })
        .collect()
}
// Reused command literals can multiply small source files into large expanded definitions.
fn charge_expansion(actions: &[LocatedAction], bytes: &mut usize) -> Result<(), Diagnostic> {
    for entry in actions {
        let size = match &entry.action {
            ActionDocument::Set { key, value } | ActionDocument::Add { key, amount: value } => {
                key.len() + slot_bytes(value)
            }
            ActionDocument::Emit { id, value } => id.len() + slot_bytes(value),
            ActionDocument::Remove { key } | ActionDocument::Reset { key } => key.len(),
            ActionDocument::Call { command, args } => {
                command.len() + args.iter().map(slot_bytes).sum::<usize>()
            }
        };
        *bytes = bytes.saturating_add(size + entry.location.file.len() + entry.location.path.len());
        if *bytes > 32 * 1024 * 1024 {
            return Err(entry.location.error("expanded rule data exceeds 32 MiB"));
        }
    }
    Ok(())
}
fn slot_bytes(value: &SlotValue) -> usize {
    match value {
        SlotValue::Literal(value) => 8 + value.payload_bytes(),
        SlotValue::Argument { argument } => argument.len(),
    }
}

fn validate_expanded(
    id: &str,
    actions: &[LocatedAction],
    types: &BTreeMap<String, ValueType>,
    manifest: &Manifest,
    state: &State,
) -> Result<(), Diagnostic> {
    let bindings = types
        .iter()
        .map(|(name, kind)| (name.clone(), SlotValue::Literal(exemplar(*kind))))
        .collect();
    let primitives = materialize(actions, &bindings)?;
    for (entry, action) in actions.iter().zip(primitives) {
        let event = Event {
            id: id.into(),
            trigger: "command.validation".into(),
            priority: 0,
            condition: None,
            actions: vec![action],
        };
        validate_scopes(&event, manifest, state).map_err(|e| entry.location.error(e))?;
        Scheduler::new(vec![event], state).map_err(|e| entry.location.error(e))?;
    }
    Ok(())
}
impl Rules {
    pub fn has_command(&self, name: &str) -> bool {
        self.commands.contains_key(name)
    }

    pub fn validate_command(&self, name: &str, args: &[Value]) -> Result<(), Diagnostic> {
        let command = self.commands.get(name).ok_or_else(|| {
            Diagnostic::new(MANIFEST_FILE, None, format!("unknown command `{name}`"))
        })?;
        if args.len() != command.parameters.len() {
            return Err(command.location.error(format!(
                "command `{name}` expects {} arguments, got {}",
                command.parameters.len(),
                args.len()
            )));
        }
        for (index, (value, parameter)) in args.iter().zip(&command.parameters).enumerate() {
            if !parameter.kind.accepts(value) {
                return Err(command.location.error(format!(
                    "argument {index} `{}` expected {:?}, got {}",
                    parameter.name,
                    parameter.kind,
                    value.type_name()
                )));
            }
            value_limit(value).map_err(|e| command.location.error(e))?;
        }
        Ok(())
    }

    /// Yarn supplies text arguments; interpolation placeholders are accepted only during validation.
    pub fn parse_command(
        &self,
        name: &str,
        args: &[String],
        allow_placeholders: bool,
    ) -> Result<Vec<Value>, Diagnostic> {
        let command = self.commands.get(name).ok_or_else(|| {
            Diagnostic::new(MANIFEST_FILE, None, format!("unknown command `{name}`"))
        })?;
        if args.len() != command.parameters.len() {
            return Err(command.location.error(format!(
                "command `{name}` expects {} arguments, got {}",
                command.parameters.len(),
                args.len()
            )));
        }
        let values = args
            .iter()
            .zip(&command.parameters)
            .enumerate()
            .map(|(index, (text, parameter))| {
                let interpolated = text.split('{').skip(1).any(|part| part.split_once('}').is_some_and(|(index, _)| !index.is_empty() && index.bytes().all(|b| b.is_ascii_digit())));
                if allow_placeholders && interpolated {
                    if parameter.kind == ValueType::String {
                        return Err(command.location.error("Yarn string arguments must be literal text; interpolated command strings are unsupported"));
                    }
                    return Ok(exemplar(parameter.kind));
                }
                let value = match parameter.kind {
                    ValueType::String => Some(Value::String(text.clone())),
                    ValueType::Bool => text.parse::<bool>().ok().map(Value::Bool),
                    ValueType::Int => text.parse::<i64>().ok().map(Value::Int),
                    ValueType::Float => text
                        .parse::<f64>()
                        .ok()
                        .filter(|v| v.is_finite())
                        .map(Value::Float),
                };
                value.ok_or_else(|| {
                    command.location.error(format!(
                        "argument {index} `{}` expected {:?}, got `{text}`",
                        parameter.name, parameter.kind
                    ))
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        self.validate_command(name, &values)?;
        Ok(values)
    }

    /// Validate all arguments before mutation; runtime failures preserve completed primitive writes.
    pub fn call(
        &self,
        name: &str,
        args: &[Value],
        state: &mut State,
        from: usize,
    ) -> Result<(), Diagnostic> {
        self.call_with(name, args, state, from, &mut NoCommands)
    }

    pub fn call_with(
        &self,
        name: &str,
        args: &[Value],
        state: &mut State,
        from: usize,
        executor: &mut dyn CommandExecutor,
    ) -> Result<(), Diagnostic> {
        check_notifications(state, from)?;
        self.validate_command(name, args)?;
        let command = &self.commands[name];
        let bindings = command
            .parameters
            .iter()
            .zip(args)
            .map(|(p, v)| (p.name.clone(), SlotValue::Literal(v.clone())))
            .collect();
        let actions = materialize(&command.actions, &bindings)?;
        let mut budget = Budget::new(DISPATCH_BUDGET);
        let result = self
            .scheduler
            .execute_with(name, &actions, state, &mut budget, executor);
        self.record_trace(&result);
        result.map_err(|error| {
            if let Some(diagnostic) = command_diagnostic(&error.error) {
                return diagnostic;
            }
            match error.error {
                FlowError::Sequence(error) => {
                    let index = match &*error {
                        FlowError::Action { index, .. } => Some(*index),
                        FlowError::Budget { action, .. } => *action,
                        _ => None,
                    };
                    index
                        .and_then(|i| command.actions.get(i))
                        .map(|a| &a.location)
                        .unwrap_or(&command.location)
                        .error(error)
                }
                error => self.diagnostic(error),
            }
        })?;
        self.settle_budgeted(state, from, &mut budget, executor)
    }
}

fn references(condition: &Condition, keys: &mut Vec<String>) {
    match condition {
        Condition::Compare { left, right, .. } => {
            for operand in [left, right] {
                if let Operand::State(key) = operand {
                    keys.push(key.clone());
                }
            }
        }
        Condition::And(children) | Condition::Or(children) => {
            for child in children {
                references(child, keys);
            }
        }
        Condition::Not(child) => references(child, keys),
    }
}

fn validate_scopes(event: &Event, manifest: &Manifest, state: &State) -> Result<(), String> {
    if event.id.len() > 512 || event.trigger.len() > 1024 {
        return Err("event ID/trigger exceeds 512/1024 bytes".into());
    }
    let mut keys = Vec::new();
    if let Some(key) = event.trigger.strip_prefix(CHANGED_KEY) {
        state.scope(key).map_err(|e| format!("/trigger: {e}"))?;
        keys.push(key.into());
    }
    if manifest.save_schema.is_some() && event.trigger == CHANGED {
        return Err("/trigger: save-enabled rules require a keyed state.changed.<key> trigger or an explicit signal".into());
    }
    if let Some(condition) = &event.condition {
        references(condition, &mut keys);
        condition_limits(condition)?;
    }
    if let Some(locale_key) = manifest
        .localization
        .as_ref()
        .and_then(|l| l.state_key.as_ref())
    {
        if event.trigger == CHANGED
            || event.trigger.strip_prefix(CHANGED_KEY) == Some(locale_key.as_str())
        {
            return Err("localization Settings may not drive state.changed rules".into());
        }
        if keys.contains(locale_key) {
            return Err("rule conditions may not read localization Settings".into());
        }
    }
    for action in &event.actions {
        match action {
            Action::Set { key, .. }
            | Action::Add { key, .. }
            | Action::Remove { key }
            | Action::Reset { key } => {
                if manifest
                    .localization
                    .as_ref()
                    .and_then(|l| l.state_key.as_ref())
                    == Some(key)
                {
                    return Err(
                        "rules may not write localization Settings; use a UI state.set action"
                            .into(),
                    );
                }
                keys.push(key.clone());
                match action {
                    Action::Set { value, .. } => value_limit(value)?,
                    Action::Add { amount, .. } => value_limit(amount)?,
                    _ => {}
                }
            }
            Action::Call { args, .. } => {
                for value in args {
                    value_limit(value)?;
                }
            }
            Action::Emit(signal) => {
                if signal.id.len() > 1024 {
                    return Err("signal ID exceeds 1024 bytes".into());
                }
                value_limit(&signal.value)?;
            }
        }
    }
    for key in keys {
        if key.len() > 512 {
            return Err("rule state keys must not exceed 512 bytes".into());
        }
        value_limit(state.default_value(&key).map_err(|e| e.to_string())?)?;
        if let Ok(value) = state.get(&key) {
            value_limit(value)?;
        }
        if manifest.save_schema.is_some()
            && state.scope(&key).map_err(|e| e.to_string())? != Scope::Persistent
        {
            return Err(format!(
                "save-enabled rules may reference only Persistent state, found `{key}`"
            ));
        }
    }
    Ok(())
}

fn load_script(
    root: &Path,
    manifest: &Manifest,
    state: &State,
    command: &CommandDocument,
    script: &ScriptDocument,
) -> Result<ScriptDefinition, String> {
    if !manifest
        .requires
        .as_ref()
        .is_some_and(|r| r.capabilities.iter().any(|c| c == "rules.rhai/1"))
    {
        return Err("script commands require rules.rhai/1".into());
    }
    let valid_name =
        !script.function.is_empty()
            && script.function.len() <= 128
            && script.function.bytes().enumerate().all(|(i, b)| {
                b.is_ascii_alphabetic() || b == b'_' || (i > 0 && b.is_ascii_digit())
            });
    if !valid_name {
        return Err("script function must be an ASCII identifier of at most 128 bytes".into());
    }
    for (label, keys) in [
        ("reads", &script.reads),
        ("writes", &script.writes),
        ("signals", &script.signals),
    ] {
        if keys.len() > 256 {
            return Err(format!("script {label} permission count exceeds 256"));
        }
        let mut unique = BTreeSet::new();
        for key in keys {
            weftora_core::state::validate_key(key).map_err(|e| e.to_string())?;
            if key.len() > 512 || !unique.insert(key) {
                return Err(format!(
                    "script {label} permissions must be unique and at most 512 bytes"
                ));
            }
            if label == "signals" && (key == CHANGED || key.starts_with(CHANGED_KEY)) {
                return Err(
                    "script signals may not impersonate state.changed notifications".into(),
                );
            }
            if matches!(label, "reads" | "writes")
                && manifest
                    .localization
                    .as_ref()
                    .and_then(|l| l.state_key.as_ref())
                    == Some(key)
            {
                return Err(format!(
                    "scripts may not {} localization Settings",
                    if label == "reads" { "read" } else { "write" }
                ));
            }
            if label != "signals" {
                let scope = state.scope(key).map_err(|e| e.to_string())?;
                value_limit(state.default_value(key).map_err(|e| e.to_string())?)?;
                if manifest.save_schema.is_some() && scope != Scope::Persistent {
                    return Err(format!(
                        "save-enabled scripts require Persistent permissions, found `{key}`"
                    ));
                }
            }
        }
    }
    if let Some(key) = &script.random {
        if !manifest
            .requires
            .as_ref()
            .is_some_and(|r| r.capabilities.iter().any(|c| c == "rules.random/1"))
        {
            return Err("script randomness requires rules.random/1".into());
        }
        if !script.reads.contains(key) || !script.writes.contains(key) {
            return Err("random state key requires both read and write permission".into());
        }
        if state.scope(key).map_err(|e| e.to_string())? != Scope::Persistent
            || !matches!(
                state.default_value(key).map_err(|e| e.to_string())?,
                Value::Int(_)
            )
        {
            return Err("random state key must be a Persistent int".into());
        }
    }
    let path = resolve_in_root(root, &script.file).map_err(|e| e.to_string())?;
    if std::fs::metadata(&path).map_err(|e| e.to_string())?.len() > 65536 {
        return Err("script source exceeds 65536 bytes".into());
    }
    let source = paths::read_text(path)?;
    if source.len() > 65536 {
        return Err("script source exceeds 65536 bytes".into());
    }
    Ok(ScriptDefinition {
        id: command.id.clone(),
        file: script.file.clone(),
        source,
        function: script.function.clone(),
        arity: command.parameters.len(),
        persistent_only: manifest.save_schema.is_some(),
        random: script.random.clone(),
        reads: script.reads.clone(),
        writes: script.writes.clone(),
        signals: script.signals.clone(),
    })
}

pub(crate) fn load(
    root: &Path,
    manifest: &Manifest,
    state: &State,
) -> Result<Rules, Vec<Diagnostic>> {
    if manifest.events.is_empty() {
        return Ok(Rules::default());
    }
    if !manifest
        .requires
        .as_ref()
        .is_some_and(|r| r.capabilities.iter().any(|c| c == "rules.events/1"))
    {
        return Err(vec![Diagnostic::new(
            MANIFEST_FILE,
            None,
            "/events: event files require rules.events/1",
        )]);
    }
    let mut events = Vec::new();
    let mut sources = BTreeMap::new();
    let mut ids = Catalog::default();
    let mut documents = Vec::new();
    let mut raw_commands = BTreeMap::new();
    let mut commands = BTreeMap::new();
    let mut action_sources = BTreeMap::new();
    let mut expanded_bytes = 0;
    let mut files = BTreeSet::new();
    let mut scripts = Vec::new();
    let mut script_bytes = 0usize;
    for (file_index, file) in manifest.events.iter().enumerate() {
        let fail = |message: String| vec![Diagnostic::new(file, None, message)];
        if !files.insert(file) {
            return Err(vec![Diagnostic::new(
                MANIFEST_FILE,
                None,
                format!("/events/{file_index}: duplicate rule source `{file}`"),
            )]);
        }
        let text = resolve_in_root(root, file)
            .map_err(|d| vec![d])
            .and_then(|p| paths::read_text(p).map_err(fail))?;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Header {
            rules_schema: u32,
        }
        let parse_error = |e: serde_json::Error| {
            vec![Diagnostic::new(
                file,
                u32::try_from(e.line()).ok().filter(|l| *l > 0),
                e.to_string(),
            )]
        };
        let header: Header = jsonc::from_str(&text).map_err(parse_error)?;
        if header.rules_schema != 1 {
            return Err(fail(format!(
                "/rulesSchema: unsupported rulesSchema {} (supports 1)",
                header.rules_schema
            )));
        }
        let document: Document = jsonc::from_str(&text).map_err(parse_error)?;
        debug_assert_eq!(document.rules_schema, 1);
        for (index, command) in document.commands.into_iter().enumerate() {
            let location = Location {
                file: file.clone(),
                path: format!("/commands/{index}"),
            };
            if !manifest
                .requires
                .as_ref()
                .is_some_and(|r| r.capabilities.iter().any(|c| c == "rules.commands/1"))
            {
                return Err(vec![location.error("commands require rules.commands/1")]);
            }
            validate_command_definition(&command).map_err(|e| vec![location.error(e)])?;
            if let Some(script) = &command.script {
                let definition = load_script(root, manifest, state, &command, script)
                    .map_err(|error| vec![location.error(error)])?;
                script_bytes = script_bytes.saturating_add(definition.source.len());
                if scripts.len() >= 64 || script_bytes > 1024 * 1024 {
                    return Err(vec![
                        location.error("scripts exceed 64 commands or 1 MiB total source"),
                    ]);
                }
                scripts.push(definition);
            }
            let id = command.id.clone();
            if raw_commands
                .insert(id.clone(), (command, location.clone()))
                .is_some()
            {
                return Err(vec![location.error(format!("duplicate command ID `{id}`"))]);
            }
            if raw_commands.len() > 256 {
                return Err(vec![location.error("command count exceeds 256")]);
            }
        }
        documents.push((file.clone(), document.events));
    }
    for (id, (command, location)) in &raw_commands {
        let bindings = command
            .parameters
            .iter()
            .map(|p| {
                (
                    p.name.clone(),
                    SlotValue::Argument {
                        argument: p.name.clone(),
                    },
                )
            })
            .collect();
        let types = command
            .parameters
            .iter()
            .map(|p| (p.name.clone(), p.kind))
            .collect();
        let root_actions = if command.script.is_some() {
            vec![ActionDocument::Call {
                command: id.clone(),
                args: command
                    .parameters
                    .iter()
                    .map(|p| SlotValue::Argument {
                        argument: p.name.clone(),
                    })
                    .collect(),
            }]
        } else {
            Vec::new()
        };
        let actions = expand(
            command.actions.as_deref().unwrap_or(&root_actions),
            location,
            &bindings,
            &types,
            &raw_commands,
            &mut vec![id.clone()],
            &mut 1024,
        )
        .map_err(|e| vec![e])?;
        charge_expansion(&actions, &mut expanded_bytes).map_err(|e| vec![e])?;
        validate_expanded(id, &actions, &types, manifest, state).map_err(|e| vec![e])?;
        commands.insert(
            id.clone(),
            Command {
                parameters: command.parameters.clone(),
                actions,
                location: location.clone(),
            },
        );
    }
    for (file, entries) in documents {
        let fail = |message: String| vec![Diagnostic::new(&file, None, message)];
        if events.len() + entries.len() > MAX_EVENTS {
            return Err(fail(format!("/events: event count exceeds {MAX_EVENTS}")));
        }
        for (index, entry) in entries.into_iter().enumerate() {
            let path = format!("/events/{index}");
            if entry.actions.len() > MAX_ACTIONS {
                return Err(fail(format!(
                    "{path}/actions: action count exceeds {MAX_ACTIONS}"
                )));
            }
            let condition = entry
                .condition
                .map(|c| {
                    let mut remaining = MAX_NODES;
                    c.build(1, &mut remaining)
                })
                .transpose()
                .map_err(|e| fail(format!("{path}/condition: {e}")))?;
            let location = Location {
                file: file.clone(),
                path: path.clone(),
            };
            let expanded = expand(
                &entry.actions,
                &location,
                &BTreeMap::new(),
                &BTreeMap::new(),
                &raw_commands,
                &mut Vec::new(),
                &mut 1024,
            )
            .map_err(|e| vec![e])?;
            charge_expansion(&expanded, &mut expanded_bytes).map_err(|e| vec![e])?;
            let event = Event {
                id: entry.id,
                trigger: entry.trigger,
                priority: entry.priority,
                condition,
                actions: materialize(&expanded, &BTreeMap::new()).map_err(|e| vec![e])?,
            };
            ids.insert(&event.id, (), &file, &format!("{path}/id"))
                .map_err(|d| vec![d])?;
            let mut single_sources = BTreeMap::new();
            single_sources.insert(event.id.clone(), (file.clone(), path.clone()));
            let location = Rules {
                scheduler: Arc::default(),
                sources: Arc::new(single_sources),
                action_sources: Arc::new(BTreeMap::from([(
                    event.id.clone(),
                    expanded.iter().map(|a| a.location.clone()).collect(),
                )])),
                ..Rules::default()
            };
            validate_scopes(&event, manifest, state).map_err(|e| fail(format!("{path}: {e}")))?;
            Scheduler::new(vec![event.clone()], state).map_err(|e| vec![location.diagnostic(e)])?;
            action_sources.insert(
                event.id.clone(),
                expanded.iter().map(|a| a.location.clone()).collect(),
            );
            sources.insert(event.id.clone(), (file.clone(), path));
            events.push(event);
        }
    }
    let scheduler = Scheduler::new(events, state)
        .map_err(|e| vec![Diagnostic::new(MANIFEST_FILE, None, e.to_string())])?;
    Ok(Rules {
        scheduler: Arc::new(scheduler),
        sources: Arc::new(sources),
        action_sources: Arc::new(action_sources),
        commands: Arc::new(commands),
        scripts: Arc::new(scripts),
        trace: None,
    })
}
