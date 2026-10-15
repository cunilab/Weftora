//! FIFO nested signals, with subscribers in registration order and a dispatch budget.

use crate::State;
use serde::Serialize;
use std::collections::VecDeque;
use weftora_api::Value;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Signal {
    pub id: String,
    pub value: Value,
}
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SignalError {
    #[error("invalid signal ID `{0}`")]
    InvalidId(String),
    #[error("signal payload must be finite")]
    NonFinite,
    #[error("signal dispatch budget ({0}) exceeded")]
    Budget(usize),
    #[error("signal handler failed: {0}")]
    Handler(String),
}
type Handler = Box<dyn FnMut(&Signal, &mut State) -> Result<Vec<Signal>, SignalError>>;
struct Subscription {
    token: u64,
    id: String,
    handler: Handler,
}
#[derive(Default)]
pub struct SignalBus {
    subscriptions: Vec<Subscription>,
    next_token: u64,
}

impl SignalBus {
    /// Handlers use the supplied State; they must not reacquire its SharedState lock during dispatch.
    pub fn subscribe(
        &mut self,
        id: &str,
        handler: impl FnMut(&Signal, &mut State) -> Result<Vec<Signal>, SignalError> + 'static,
    ) -> Result<u64, SignalError> {
        crate::state::validate_key(id).map_err(|_| SignalError::InvalidId(id.into()))?;
        let token = self.next_token;
        self.next_token = self
            .next_token
            .checked_add(1)
            .ok_or_else(|| SignalError::Handler("subscription IDs exhausted".into()))?;
        self.subscriptions.push(Subscription {
            token,
            id: id.into(),
            handler: Box::new(handler),
        });
        Ok(token)
    }
    pub fn unsubscribe(&mut self, token: u64) -> bool {
        let before = self.subscriptions.len();
        self.subscriptions.retain(|s| s.token != token);
        before != self.subscriptions.len()
    }
    /// Nested signals returned by handlers run only after all subscribers of the current signal.
    /// Both deliveries and queued emissions count against the budget, bounding loops and fan-out.
    pub fn emit(
        &mut self,
        signal: Signal,
        state: &mut State,
        budget: usize,
    ) -> Result<Vec<Signal>, SignalError> {
        validate(&signal)?;
        if budget == 0 {
            return Err(SignalError::Budget(budget));
        }
        let mut queue = VecDeque::from([signal]);
        let mut trace = Vec::new();
        let mut work = 1;
        while let Some(signal) = queue.pop_front() {
            trace.push(signal.clone());
            for subscription in &mut self.subscriptions {
                if subscription.id == signal.id {
                    if work >= budget {
                        return Err(SignalError::Budget(budget));
                    }
                    work += 1;
                    let nested = (subscription.handler)(&signal, state)?;
                    for next in nested {
                        validate(&next)?;
                        if work >= budget {
                            return Err(SignalError::Budget(budget));
                        }
                        work += 1;
                        queue.push_back(next);
                    }
                }
            }
        }
        Ok(trace)
    }
}
/// Validates a signal without delivering it or changing state.
pub fn validate(signal: &Signal) -> Result<(), SignalError> {
    crate::state::validate_key(&signal.id)
        .map_err(|_| SignalError::InvalidId(signal.id.clone()))?;
    if let Value::Float(f) = &signal.value
        && !f.is_finite()
    {
        return Err(SignalError::NonFinite);
    }
    Ok(())
}
