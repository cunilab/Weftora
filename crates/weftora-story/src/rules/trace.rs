//! Optional bounded diagnostics; never part of canonical state or cloned rule definitions.
use super::Rules;
use std::{
    collections::VecDeque,
    fmt::{self, Write},
    sync::Mutex,
};
use weftora_flow::events::{DispatchError, Trace};

const MAX_ROWS: usize = 128;
const MAX_CHARS: usize = 256;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuleTrace {
    pub rows: VecDeque<String>,
    pub dropped: usize,
}

#[derive(Default)]
struct Line {
    text: String,
    chars: usize,
}
impl Line {
    fn push_char(&mut self, c: char) -> fmt::Result {
        if self.chars == MAX_CHARS {
            self.text.pop();
            self.text.push('…');
            return Err(fmt::Error);
        }
        self.text.push(c);
        self.chars += 1;
        Ok(())
    }
}
impl Write for Line {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        for c in text.chars() {
            if matches!(c, '"' | '\\' | '\'') {
                self.push_char(c)?;
            } else {
                for escaped in c.escape_debug() {
                    self.push_char(escaped)?;
                }
            }
        }
        Ok(())
    }
}
impl RuleTrace {
    fn push(&mut self, args: fmt::Arguments<'_>) {
        let mut line = Line::default();
        let _ = fmt::write(&mut line, args);
        if self.rows.len() == MAX_ROWS {
            self.rows.pop_front();
            self.dropped = self.dropped.saturating_add(1);
        }
        self.rows.push_back(line.text);
    }
}

impl Rules {
    /// Starts a fresh diagnostic history. Cloned Rules never inherit this history.
    pub fn enable_trace(&mut self) {
        self.trace = Some(Mutex::default());
    }
    pub fn trace(&self) -> Option<RuleTrace> {
        self.trace
            .as_ref()
            .map(|trace| trace.lock().unwrap_or_else(|e| e.into_inner()).clone())
    }
    pub fn clear_trace(&self) {
        if let Some(trace) = &self.trace {
            *trace.lock().unwrap_or_else(|e| e.into_inner()) = RuleTrace::default();
        }
    }
    pub(super) fn record_trace(&self, result: &Result<Vec<Trace>, DispatchError>) {
        let Some(trace) = &self.trace else {
            return;
        };
        let mut history = trace.lock().unwrap_or_else(|e| e.into_inner());
        let rows = match result {
            Ok(rows) => rows,
            Err(error) => &error.trace,
        };
        for row in rows {
            match row {
                Trace::Signal(signal) => {
                    history.push(format_args!("signal {} = {:?}", signal.id, signal.value))
                }
                Trace::Event { id, matched } => history.push(format_args!(
                    "event {id}: {}",
                    if *matched { "matched" } else { "skipped" }
                )),
                Trace::Action { event, index } => {
                    history.push(format_args!("action {event}[{index}] completed"))
                }
            }
        }
        if let Err(error) = result {
            history.push(format_args!("error: {}", error.error));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn history_is_bounded_escaped_and_clearable_without_clone_leaks() {
        let mut rules = Rules::default();
        assert!(rules.trace().is_none());
        rules.enable_trace();
        let clone = rules.clone();
        assert!(clone.trace().is_none());
        let result = Ok(vec![
            Trace::Event {
                id: "test".into(),
                matched: false
            };
            MAX_ROWS + 7
        ]);
        rules.record_trace(&result);
        let trace = rules.trace().unwrap();
        assert_eq!(trace.rows.len(), MAX_ROWS);
        assert_eq!(trace.dropped, 7);
        assert_eq!(trace.rows[0], "event test: skipped");
        let mut line = Line::default();
        let _ = write!(&mut line, "é\n\t\r\u{1b}{}", "界".repeat(1000));
        assert_eq!(line.text.chars().count(), MAX_CHARS);
        assert!(line.text.starts_with(r"é\n\t\r\u{1b}"));
        assert!(line.text.ends_with('…'));
        assert!(!line.text.chars().any(char::is_control));
        clone.record_trace(&result);
        assert_eq!(rules.trace().unwrap(), trace);
        rules.clear_trace();
        assert_eq!(rules.trace(), Some(RuleTrace::default()));
    }
}
