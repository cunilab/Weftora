//! JSONC Story rules and the notification boundary shared by dialogue hosts.
use crate::{MANIFEST_FILE, Manifest, jsonc, paths, resolve_in_root};
use serde::Deserialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
    sync::Arc,
};
use weftora_api::{Diagnostic, Value};
use weftora_core::{Scope, State, catalog::Catalog, signal::Signal};
use weftora_flow::{
    CompareOp, Condition, MAX_DEPTH, MAX_NODES, Operand,
    events::{Action, Budget, Event, FlowError, MAX_ACTIONS, MAX_EVENTS, Scheduler},
};

pub const DISPATCH_BUDGET: usize = 1024;
const CHANGED: &str = "state.changed";
const CHANGED_KEY: &str = "state.changed.";

#[derive(Debug, Clone, Default)]
pub struct Rules {
    scheduler: Arc<Scheduler>,
    sources: Arc<BTreeMap<String, (String, String)>>,
}

impl Rules {
    pub fn is_empty(&self) -> bool {
        self.sources.is_empty()
    }

    fn diagnostic(&self, error: FlowError) -> Diagnostic {
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
        match source {
            Some((file, path)) => Diagnostic::new(file, None, format!("{path}{suffix}: {error}")),
            None => Diagnostic::new(MANIFEST_FILE, None, format!("/events: {error}")),
        }
    }

    /// Settle new notifications under the caller's exclusive State borrow; preserve the full queue.
    /// The starting index is captured before one VM call, which never drains this queue.
    pub fn settle(&self, state: &mut State, from: usize) -> Result<(), Diagnostic> {
        check_notifications(state, from)?;
        let mut budget = Budget::new(DISPATCH_BUDGET);
        let mut cursor = from;
        while let Some(change) = state.pending_changes().get(cursor) {
            let key = change.key.clone();
            cursor += 1;
            for trigger in [CHANGED.to_owned(), format!("{CHANGED_KEY}{key}")] {
                if self.scheduler.has_trigger(&trigger) {
                    self.scheduler
                        .dispatch_budgeted(
                            Signal {
                                id: trigger,
                                value: Value::String(key.clone()),
                            },
                            state,
                            &mut budget,
                        )
                        .map_err(|error| self.diagnostic(error.error))?;
                    check_notifications(state, from)?;
                }
            }
        }
        Ok(())
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
            if let Value::String(text) = value {
                bytes = bytes.saturating_add(text.len());
            }
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
    events: Vec<EventDocument>,
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

#[derive(Deserialize)]
#[serde(tag = "do", deny_unknown_fields)]
enum ActionDocument {
    #[serde(rename = "state.set")]
    Set { key: String, value: Value },
    #[serde(rename = "state.add")]
    Add { key: String, amount: Value },
    #[serde(rename = "state.remove")]
    Remove { key: String },
    #[serde(rename = "state.reset")]
    Reset { key: String },
    #[serde(rename = "signal.emit")]
    Emit { id: String, value: Value },
}
impl From<ActionDocument> for Action {
    fn from(action: ActionDocument) -> Self {
        match action {
            ActionDocument::Set { key, value } => Self::Set { key, value },
            ActionDocument::Add { key, amount } => Self::Add { key, amount },
            ActionDocument::Remove { key } => Self::Remove { key },
            ActionDocument::Reset { key } => Self::Reset { key },
            ActionDocument::Emit { id, value } => Self::Emit(Signal { id, value }),
        }
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
    for action in &event.actions {
        match action {
            Action::Set { key, .. }
            | Action::Add { key, .. }
            | Action::Remove { key }
            | Action::Reset { key } => {
                keys.push(key.clone());
                match action {
                    Action::Set { value, .. } => value_limit(value)?,
                    Action::Add { amount, .. } => value_limit(amount)?,
                    _ => {}
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
    let mut files = BTreeSet::new();
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
        if events.len() + document.events.len() > MAX_EVENTS {
            return Err(fail(format!("/events: event count exceeds {MAX_EVENTS}")));
        }
        for (index, entry) in document.events.into_iter().enumerate() {
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
            let event = Event {
                id: entry.id,
                trigger: entry.trigger,
                priority: entry.priority,
                condition,
                actions: entry.actions.into_iter().map(Into::into).collect(),
            };
            ids.insert(&event.id, (), file, &format!("{path}/id"))
                .map_err(|d| vec![d])?;
            let mut single_sources = BTreeMap::new();
            single_sources.insert(event.id.clone(), (file.clone(), path.clone()));
            let location = Rules {
                scheduler: Arc::default(),
                sources: Arc::new(single_sources),
            };
            validate_scopes(&event, manifest, state).map_err(|e| fail(format!("{path}: {e}")))?;
            Scheduler::new(vec![event.clone()], state).map_err(|e| vec![location.diagnostic(e)])?;
            sources.insert(event.id.clone(), (file.clone(), path));
            events.push(event);
        }
    }
    let scheduler = Scheduler::new(events, state)
        .map_err(|e| vec![Diagnostic::new(MANIFEST_FILE, None, e.to_string())])?;
    Ok(Rules {
        scheduler: Arc::new(scheduler),
        sources: Arc::new(sources),
    })
}
