//! Runtime-owned checkpoint eligibility; no caller-provided safety flag.
use super::{DialogueEvent, DialogueRunner};
use weftora_core::Scope;
use weftora_save::{Entries, Snapshot};

#[derive(Debug, thiserror::Error)]
pub enum SaveRefusal {
    #[error("save is available only at the first line or choice after a checkpoint")]
    NotAtCheckpoint,
    #[error("persistent state changed after the checkpoint display")]
    Changed,
    #[error("checkpoint data cannot be saved: {0}")]
    Data(String),
}

pub(crate) struct Capture {
    node: String,
    snapshot: Result<Snapshot, String>,
    expected: Option<Entries>,
}

fn check_persistent_limits(state: &weftora_core::State) -> Result<(), String> {
    let mut bytes = 0usize;
    let mut entries = 0usize;
    for (key, value) in state.scope_entries(Scope::Persistent) {
        entries += 1;
        bytes = bytes
            .saturating_add(key.len() + value.map(weftora_api::Value::payload_bytes).unwrap_or(0));
        if entries > weftora_save::MAX_ENTRIES || bytes > weftora_save::MAX_SAVE_BYTES {
            return Err("persistent payload exceeds the save limit (4096 entries / 8 MiB)".into());
        }
        if matches!(value, Some(weftora_api::Value::String(text)) if text.len() > weftora_save::MAX_STRING_BYTES)
        {
            return Err("persistent string exceeds the 16 KiB save limit".into());
        }
    }
    Ok(())
}

impl DialogueRunner {
    pub(crate) fn disarm_after_progress(&mut self) {
        if self
            .capture
            .as_ref()
            .is_some_and(|capture| capture.expected.is_some())
        {
            self.capture = None;
        }
    }

    pub(crate) fn capture_checkpoint(&mut self, id: &str) {
        let snapshot = (|| {
            let state = self
                .state
                .read()
                .map_err(|_| "state lock poisoned".to_string())?;
            check_persistent_limits(&state)?;
            let snapshot = Snapshot {
                checkpoint: id.into(),
                state: state.snapshot_entries(Scope::Persistent),
                visits: self
                    .storage
                    .visit_snapshot()
                    .map_err(|error| error.to_string())?,
            };
            snapshot.validate().map_err(|error| error.to_string())?;
            Ok(snapshot)
        })();
        self.capture = Some(Capture {
            node: self.current_node.clone(),
            snapshot,
            expected: None,
        });
    }

    pub(crate) fn arm_checkpoint(&mut self, event: &DialogueEvent) {
        if matches!(event, DialogueEvent::End) {
            self.capture = None;
            return;
        }
        if !matches!(
            event,
            DialogueEvent::Line { .. } | DialogueEvent::Choices(_)
        ) {
            return;
        }
        let Some(capture) = &mut self.capture else {
            return;
        };
        if capture.node != self.current_node {
            self.capture = None;
            return;
        }
        if capture.snapshot.is_err() {
            capture.expected = Some(Entries::new());
            return;
        }
        match self.state.read() {
            Ok(state) => match check_persistent_limits(&state) {
                Ok(()) => capture.expected = Some(state.snapshot_entries(Scope::Persistent)),
                Err(error) => {
                    capture.snapshot = Err(error);
                    capture.expected = Some(Entries::new());
                }
            },
            Err(_) => {
                capture.snapshot = Err("state lock poisoned".into());
                capture.expected = Some(Entries::new());
            }
        }
    }

    /// Returns frozen node-entry data only while its first blocking display remains valid.
    pub fn save_point(&self) -> Result<Snapshot, SaveRefusal> {
        if self.failure.is_some() {
            return Err(SaveRefusal::NotAtCheckpoint);
        }
        let capture = self.capture.as_ref().ok_or(SaveRefusal::NotAtCheckpoint)?;
        let expected = capture
            .expected
            .as_ref()
            .ok_or(SaveRefusal::NotAtCheckpoint)?;
        let snapshot = capture
            .snapshot
            .as_ref()
            .map_err(|message| SaveRefusal::Data(message.clone()))?;
        let state = self
            .state
            .read()
            .map_err(|_| SaveRefusal::Data("state lock poisoned".into()))?;
        if !state.matches_snapshot(Scope::Persistent, expected) {
            return Err(SaveRefusal::Changed);
        }
        Ok(snapshot.clone())
    }

    /// Restore bookkeeping on a fresh candidate before it starts executing.
    pub fn restore_visits(
        &mut self,
        visits: &weftora_save::Visits,
    ) -> Result<(), super::DialogueError> {
        self.ensure_healthy()?;
        if self.started {
            return Err(super::DialogueError::Runtime(
                "visit restore requires a fresh runner".into(),
            ));
        }
        self.storage
            .restore_visits(visits, &self.program)
            .map_err(super::storage::map_storage_error)
    }
}

/// Build save compatibility metadata from the validated package and compiled checkpoint map.
pub fn context(
    story: &weftora_story::Story,
    dialogue: &super::CompiledDialogue,
) -> Result<weftora_save::Context, weftora_save::Error> {
    let schema = story.manifest().save_schema.ok_or_else(|| {
        weftora_save::Error::Invalid("Story does not declare saveSchema/save/1".into())
    })?;
    Ok(weftora_save::Context {
        story_id: story.manifest().id.clone(),
        story_version: story.manifest().story_version.clone(),
        save_schema: schema,
        writer_engine_api: weftora_story::compatibility::ENGINE_API.into(),
        writer_engine_release: env!("CARGO_PKG_VERSION").into(),
        checkpoints: dialogue.checkpoints().keys().cloned().collect(),
    })
}
