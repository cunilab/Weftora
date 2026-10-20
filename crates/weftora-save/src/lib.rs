//! Save-format validation and durable storage. Runtime adapters own safe-point eligibility.
pub mod directory;
pub mod storage;

use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::io::{self, Write};
use weftora_api::Value;
use weftora_core::{Scope, State};

pub const SAVE_FORMAT: u32 = 1;
pub const MAX_SAVE_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_ENTRIES: usize = 4096;
pub const MAX_STRING_BYTES: usize = 16 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("unsupported saveFormat {0}; supported: 1")]
    UnsupportedFormat(u32),
    #[error("save belongs to another Story")]
    OtherStory,
    #[error(
        "save was written by newer Engine API {writer}; current reader is {reader}; use Engine API {writer} or newer to load it"
    )]
    NewerEngineApi { writer: String, reader: String },
    #[error(
        "save schema downgrade from {from} to {to} is unsupported; restore the Story revision with saveSchema {from} that wrote this save"
    )]
    StoryDowngrade { from: u32, to: u32 },
    #[error("no save migration from schema {from} to {to}")]
    NoMigration { from: u32, to: u32 },
    #[error("save checkpoint '{0}' is not available")]
    UnknownCheckpoint(String),
    #[error("{resource} exceeds {limit} bytes or entries")]
    TooLarge {
        resource: &'static str,
        limit: usize,
    },
    #[error("invalid save data: {0}")]
    Invalid(String),
    #[error("corrupt save: {0}")]
    Corrupt(String),
    #[error("no save file exists")]
    Missing,
    #[error("another save writer is busy")]
    Busy,
    #[error("save IO operation {operation}: {source}")]
    Io {
        operation: &'static str,
        #[source]
        source: io::Error,
    },
    #[error(transparent)]
    State(#[from] weftora_api::StateError),
}

pub type Entries = BTreeMap<String, Option<Value>>;
pub type Visits = BTreeMap<String, u32>;

/// Current reader/writer API, package identity, release diagnostics and declared checkpoints.
pub struct Context {
    pub story_id: String,
    pub story_version: String,
    pub save_schema: u32,
    pub writer_engine_api: String,
    pub writer_engine_release: String,
    pub checkpoints: BTreeSet<String>,
}

/// Frozen node-entry data supplied by a runtime's verified save point.
#[derive(Debug, Clone, PartialEq)]
pub struct Snapshot {
    pub checkpoint: String,
    pub state: Entries,
    pub visits: Visits,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Checkpoint {
    id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Dialogue {
    visits: Visits,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Body {
    story_id: String,
    story_version: String,
    save_schema: u32,
    writer_engine_api: String,
    writer_engine_release: String,
    checkpoint: Checkpoint,
    state: Entries,
    dialogue: Dialogue,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SavedGame {
    body: Body,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Envelope {
    save_format: u32,
    body_sha256: String,
    body: Box<RawValue>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct EnvelopeRef<'a> {
    save_format: u32,
    body_sha256: &'a str,
    body: &'a RawValue,
}

fn version(field: &str, value: &str) -> Result<semver::Version, Error> {
    if value.len() > 64 {
        return Err(Error::Invalid(format!("invalid {field}")));
    }
    semver::Version::parse(value).map_err(|_| Error::Invalid(format!("invalid {field}")))
}

impl Context {
    fn validate(&self) -> Result<(), Error> {
        if self.story_id.is_empty() || self.story_id.len() > 128 || self.save_schema == 0 {
            return Err(Error::Invalid("Story identity/schema".into()));
        }
        version("storyVersion", &self.story_version)?;
        version("writerEngineApi", &self.writer_engine_api)?;
        version("writerEngineRelease", &self.writer_engine_release)?;
        if self.checkpoints.is_empty() || self.checkpoints.len() > 1024 {
            return Err(Error::Invalid(
                "context must declare 1..1024 checkpoints".into(),
            ));
        }
        for id in &self.checkpoints {
            checkpoint_id(id)?;
        }
        Ok(())
    }
}

fn checkpoint_id(id: &str) -> Result<(), Error> {
    if id.len() > 128 {
        return Err(Error::TooLarge {
            resource: "checkpoint ID",
            limit: 128,
        });
    }
    weftora_core::state::validate_key(id)
        .map_err(|error| Error::Invalid(format!("checkpoint ID: {error}")))
}

impl Snapshot {
    pub fn validate(&self) -> Result<(), Error> {
        validate_snapshot(&self.checkpoint, &self.state, &self.visits)
    }
}

fn validate_snapshot(checkpoint: &str, state: &Entries, visits: &Visits) -> Result<(), Error> {
    checkpoint_id(checkpoint)?;
    if state.len() > MAX_ENTRIES {
        return Err(Error::TooLarge {
            resource: "state entries",
            limit: MAX_ENTRIES,
        });
    }
    for (key, value) in state {
        if key.len() > 512 {
            return Err(Error::TooLarge {
                resource: "state key",
                limit: 512,
            });
        }
        weftora_core::state::validate_key(key).map_err(|e| Error::Invalid(e.to_string()))?;
        if let Some(value) = value {
            value.validate()?;
        }
        match value {
            Some(Value::String(s)) if s.len() > MAX_STRING_BYTES => {
                return Err(Error::TooLarge {
                    resource: "string",
                    limit: MAX_STRING_BYTES,
                });
            }
            Some(Value::Float(f)) if !f.is_finite() => {
                return Err(Error::Invalid("non-finite float".into()));
            }
            _ => {}
        }
    }
    if visits.len() > 1024 {
        return Err(Error::TooLarge {
            resource: "visit counters",
            limit: 1024,
        });
    }
    for (node, count) in visits {
        if node.is_empty() || node.len() > 256 || *count > 1 << 24 {
            return Err(Error::Invalid("visit counter".into()));
        }
    }
    Ok(())
}

impl SavedGame {
    pub fn new(context: &Context, snapshot: Snapshot) -> Result<Self, Error> {
        context.validate()?;
        snapshot.validate()?;
        if !context.checkpoints.contains(&snapshot.checkpoint) {
            return Err(Error::UnknownCheckpoint(snapshot.checkpoint.clone()));
        }
        let game = Self {
            body: Body {
                story_id: context.story_id.clone(),
                story_version: context.story_version.clone(),
                save_schema: context.save_schema,
                writer_engine_api: context.writer_engine_api.clone(),
                writer_engine_release: context.writer_engine_release.clone(),
                checkpoint: Checkpoint {
                    id: snapshot.checkpoint,
                },
                state: snapshot.state,
                dialogue: Dialogue {
                    visits: snapshot.visits,
                },
            },
        };
        Ok(game)
    }

    pub fn checkpoint(&self) -> &str {
        &self.body.checkpoint.id
    }
    pub fn state(&self) -> &Entries {
        &self.body.state
    }
    pub fn visits(&self) -> &Visits {
        &self.body.dialogue.visits
    }

    /// Release numbers are diagnostic; refuse newer writer APIs and unsupported save contracts.
    pub fn check(&self, context: &Context) -> Result<(), Error> {
        context.validate()?;
        self.validate()?;
        if self.body.story_id != context.story_id {
            return Err(Error::OtherStory);
        }
        let writer = version("writerEngineApi", &self.body.writer_engine_api)?;
        let reader = version("writerEngineApi", &context.writer_engine_api)?;
        if writer.cmp_precedence(&reader).is_gt() {
            return Err(Error::NewerEngineApi {
                writer: self.body.writer_engine_api.clone(),
                reader: context.writer_engine_api.clone(),
            });
        }
        if self.body.save_schema > context.save_schema {
            return Err(Error::StoryDowngrade {
                from: self.body.save_schema,
                to: context.save_schema,
            });
        }
        if self.body.save_schema != context.save_schema {
            return Err(Error::NoMigration {
                from: self.body.save_schema,
                to: context.save_schema,
            });
        }
        if !context.checkpoints.contains(self.checkpoint()) {
            return Err(Error::UnknownCheckpoint(self.checkpoint().into()));
        }
        Ok(())
    }

    fn validate(&self) -> Result<(), Error> {
        if self.body.story_id.is_empty()
            || self.body.story_id.len() > 128
            || self.body.save_schema == 0
        {
            return Err(Error::Invalid("Story identity/schema".into()));
        }
        version("storyVersion", &self.body.story_version)?;
        version("writerEngineApi", &self.body.writer_engine_api)?;
        version("writerEngineRelease", &self.body.writer_engine_release)?;
        validate_snapshot(self.checkpoint(), self.state(), self.visits())
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        self.validate()?;
        let body = bounded_json(&self.body)?;
        let raw = RawValue::from_string(
            String::from_utf8(body).map_err(|e| Error::Invalid(e.to_string()))?,
        )
        .map_err(|e| Error::Invalid(e.to_string()))?;
        let hash = checksum(raw.get().as_bytes());
        let mut bytes = bounded_json(&EnvelopeRef {
            save_format: SAVE_FORMAT,
            body_sha256: &hash,
            body: &raw,
        })?;
        if bytes.len() >= MAX_SAVE_BYTES {
            return Err(Error::TooLarge {
                resource: "save",
                limit: MAX_SAVE_BYTES,
            });
        }
        bytes.push(b'\n');
        Ok(bytes)
    }

    pub fn from_bytes(bytes: &[u8], context: &Context) -> Result<Self, Error> {
        if bytes.len() > MAX_SAVE_BYTES {
            return Err(Error::TooLarge {
                resource: "save",
                limit: MAX_SAVE_BYTES,
            });
        }
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Probe {
            save_format: u32,
        }
        let probe: Probe =
            serde_json::from_slice(bytes).map_err(|e| Error::Corrupt(e.to_string()))?;
        if probe.save_format != SAVE_FORMAT {
            return Err(Error::UnsupportedFormat(probe.save_format));
        }
        let envelope: Envelope =
            serde_json::from_slice(bytes).map_err(|e| Error::Corrupt(e.to_string()))?;
        if envelope.save_format != SAVE_FORMAT {
            return Err(Error::Corrupt("format mismatch".into()));
        }
        if checksum(envelope.body.get().as_bytes()) != envelope.body_sha256 {
            return Err(Error::Corrupt("checksum mismatch".into()));
        }
        let body: Body = weftora_core::json::from_str(envelope.body.get())
            .map_err(|e| Error::Corrupt(e.to_string()))?;
        let game = Self { body };
        game.check(context)?;
        Ok(game)
    }

    /// Build on a fully initialized current Story template; callers activate only after dialogue/view validation.
    pub fn prepare_state(
        &self,
        context: &Context,
        template: &State,
        current: &State,
    ) -> Result<State, Error> {
        self.check(context)?;
        let mut candidate = template.clone();
        let settings = current
            .snapshot_entries(Scope::Settings)
            .into_iter()
            .filter(|(key, _)| template.scope(key) == Ok(Scope::Settings))
            .collect();
        candidate
            .apply_snapshot(Scope::Settings, &settings)
            .map_err(Error::State)?;
        candidate
            .apply_snapshot(Scope::Persistent, self.state())
            .map_err(Error::State)?;
        Ok(candidate)
    }
}

fn checksum(bytes: &[u8]) -> String {
    let mut hash = Sha256::new();
    hash.update(b"weftora-save-format-1\0");
    hash.update(bytes);
    format!("{:x}", hash.finalize())
}

fn bounded_json(value: &impl Serialize) -> Result<Vec<u8>, Error> {
    struct Buffer {
        bytes: Vec<u8>,
        overflow: bool,
    }
    impl Write for Buffer {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if bytes.len() > MAX_SAVE_BYTES.saturating_sub(self.bytes.len()) {
                self.overflow = true;
                return Err(io::Error::other("save exceeds 8 MiB"));
            }
            self.bytes.extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let mut buffer = Buffer {
        bytes: Vec::new(),
        overflow: false,
    };
    if let Err(error) = serde_json::to_writer(&mut buffer, value) {
        return Err(if buffer.overflow {
            Error::TooLarge {
                resource: "save",
                limit: MAX_SAVE_BYTES,
            }
        } else {
            Error::Invalid(format!("serialization: {error}"))
        });
    }
    Ok(buffer.bytes)
}
