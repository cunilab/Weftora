//! Explicit signal triggers, live conditions and ordered synchronous state actions.

use crate::{Condition, ConditionError};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use weftora_api::{StateError, Value};
use weftora_core::{
    State,
    signal::{self, Signal, SignalError},
    state::validate_key,
};

pub const MAX_EVENTS: usize = 256;
pub const MAX_ACTIONS: usize = 256;

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Set { key: String, value: Value },
    Add { key: String, amount: Value },
    Remove { key: String },
    Reset { key: String },
    Emit(Signal),
}

#[derive(Debug, PartialEq, thiserror::Error)]
pub enum ActionError {
    #[error(transparent)]
    State(#[from] StateError),
    #[error(transparent)]
    Signal(#[from] SignalError),
}

impl Action {
    fn validate(&self, state: &State) -> Result<(), ActionError> {
        let mut candidate = State::new();
        let key = match self {
            Self::Set { key, .. }
            | Self::Add { key, .. }
            | Self::Remove { key }
            | Self::Reset { key } => Some(key),
            Self::Emit(_) => None,
        };
        if let Some(key) = key {
            let default = state.default_value(key)?;
            // Type-check addition independently of the default's arithmetic result.
            let value = match (self, default) {
                (Self::Add { .. }, Value::Int(_)) => Value::Int(0),
                (Self::Add { .. }, Value::Float(_)) => Value::Float(0.0),
                _ => default.clone(),
            };
            candidate.set(key, value)?;
        }
        self.apply(&mut candidate).map(|_| ())
    }

    fn apply(&self, state: &mut State) -> Result<Option<Signal>, ActionError> {
        match self {
            Self::Set { key, value } => state.set(key, value.clone())?,
            Self::Add { key, amount } => state.add(key, amount)?,
            Self::Remove { key } => {
                state.remove(key)?;
            }
            Self::Reset { key } => state.reset(key)?,
            Self::Emit(value) => {
                signal::validate(value)?;
                return Ok(Some(value.clone()));
            }
        }
        Ok(None)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Event {
    pub id: String,
    pub trigger: String,
    /// Higher numbers run first; equal priorities preserve declaration order.
    pub priority: i32,
    pub condition: Option<Condition>,
    pub actions: Vec<Action>,
}

impl Event {
    fn validate(&self, state: &State) -> Result<(), FlowError> {
        validate_key(&self.id)?;
        validate_key(&self.trigger)?;
        if self.actions.len() > MAX_ACTIONS {
            return Err(FlowError::Actions(self.id.clone()));
        }
        if let Some(condition) = &self.condition {
            condition
                .validate(state)
                .map_err(|source| FlowError::Condition {
                    event: self.id.clone(),
                    source,
                })?;
        }
        for (index, action) in self.actions.iter().enumerate() {
            action.validate(state).map_err(|source| FlowError::Action {
                event: self.id.clone(),
                index,
                source,
            })?;
        }
        Ok(())
    }
}

#[derive(Debug, PartialEq, thiserror::Error)]
pub enum FlowError {
    #[error(transparent)]
    State(#[from] StateError),
    #[error(transparent)]
    Signal(#[from] SignalError),
    #[error("event count exceeds {MAX_EVENTS}")]
    Events,
    #[error("duplicate event ID `{0}`")]
    Duplicate(String),
    #[error("event `{0}` action count exceeds {MAX_ACTIONS}")]
    Actions(String),
    #[error("event `{event}` condition: {source}")]
    Condition {
        event: String,
        #[source]
        source: ConditionError,
    },
    #[error("event `{event}` action {index}: {source}")]
    Action {
        event: String,
        index: usize,
        #[source]
        source: ActionError,
    },
    #[error("event dispatch budget ({limit}) exceeded (event {event:?}, action {action:?})")]
    Budget {
        limit: usize,
        event: Option<String>,
        action: Option<usize>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Trace {
    Signal(Signal),
    Event {
        id: String,
        matched: bool,
    },
    /// Zero-based index of a successfully completed action.
    Action {
        event: String,
        index: usize,
    },
}

#[derive(Debug, PartialEq, thiserror::Error)]
#[error("{error}")]
pub struct DispatchError {
    #[source]
    pub error: FlowError,
    pub trace: Vec<Trace>,
}

/// Immutable definitions; queues and traces belong to each dispatch, never to the scheduler.
#[derive(Debug, Default)]
pub struct Scheduler {
    events: BTreeMap<String, Vec<Event>>,
}

impl Scheduler {
    pub fn new(mut events: Vec<Event>, state: &State) -> Result<Self, FlowError> {
        if events.len() > MAX_EVENTS {
            return Err(FlowError::Events);
        }
        let mut ids = BTreeSet::new();
        for event in &events {
            if !ids.insert(&event.id) {
                return Err(FlowError::Duplicate(event.id.clone()));
            }
            event.validate(state)?;
        }
        // Stable sort keeps declaration order for ties, including extreme priorities.
        events.sort_by_key(|event| std::cmp::Reverse(event.priority));
        let mut by_trigger: BTreeMap<String, Vec<Event>> = BTreeMap::new();
        for event in events {
            by_trigger
                .entry(event.trigger.clone())
                .or_default()
                .push(event);
        }
        Ok(Self { events: by_trigger })
    }

    /// One unit per signal enqueued (initial included), event evaluated and action attempted.
    /// Errors discard pending signals but retain earlier state writes and notifications.
    pub fn dispatch(
        &self,
        signal: Signal,
        state: &mut State,
        budget: usize,
    ) -> Result<Vec<Trace>, DispatchError> {
        self.dispatch_budgeted(signal, state, &mut Budget::new(budget))
    }

    pub fn has_trigger(&self, trigger: &str) -> bool {
        self.events.contains_key(trigger)
    }

    /// Shares one budget across related dispatches, including state-change feedback.
    pub fn dispatch_budgeted(
        &self,
        signal: Signal,
        state: &mut State,
        budget: &mut Budget,
    ) -> Result<Vec<Trace>, DispatchError> {
        let mut trace = Vec::new();
        match self.run(signal, state, budget, &mut trace) {
            Ok(()) => Ok(trace),
            Err(error) => Err(DispatchError { error, trace }),
        }
    }

    fn run(
        &self,
        signal: Signal,
        state: &mut State,
        budget: &mut Budget,
        trace: &mut Vec<Trace>,
    ) -> Result<(), FlowError> {
        signal::validate(&signal)?;
        // Callers may supply a different State; reject schema drift before performing any write.
        for event in self.events.values().flatten() {
            event.validate(state)?;
        }
        budget.spend(None, None)?;
        let mut queue = VecDeque::from([signal]);
        while let Some(signal) = queue.pop_front() {
            let events = self.events.get(&signal.id);
            trace.push(Trace::Signal(signal));
            for event in events.into_iter().flatten() {
                budget.spend(Some(&event.id), None)?;
                let matched = match &event.condition {
                    Some(condition) => {
                        condition
                            .evaluate(state)
                            .map_err(|source| FlowError::Condition {
                                event: event.id.clone(),
                                source,
                            })?
                    }
                    None => true,
                };
                trace.push(Trace::Event {
                    id: event.id.clone(),
                    matched,
                });
                if !matched {
                    continue;
                }
                for (index, action) in event.actions.iter().enumerate() {
                    budget.spend(Some(&event.id), Some(index))?;
                    if matches!(action, Action::Emit(_)) {
                        budget.spend(Some(&event.id), Some(index))?;
                    }
                    let next = action.apply(state).map_err(|source| FlowError::Action {
                        event: event.id.clone(),
                        index,
                        source,
                    })?;
                    if let Some(next) = next {
                        queue.push_back(next);
                    }
                    trace.push(Trace::Action {
                        event: event.id.clone(),
                        index,
                    });
                }
            }
        }
        Ok(())
    }
}

pub struct Budget {
    limit: usize,
    remaining: usize,
}

impl Budget {
    pub fn new(limit: usize) -> Self {
        Self {
            limit,
            remaining: limit,
        }
    }

    fn spend(&mut self, event: Option<&str>, action: Option<usize>) -> Result<(), FlowError> {
        self.remaining = self
            .remaining
            .checked_sub(1)
            .ok_or_else(|| FlowError::Budget {
                limit: self.limit,
                event: event.map(str::to_owned),
                action,
            })?;
        Ok(())
    }
}
