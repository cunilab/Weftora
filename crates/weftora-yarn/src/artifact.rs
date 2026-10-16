//! Versioned compiled dialogue, shared by authoring tools and the compiler-free player.
use std::collections::BTreeMap;
use std::io::Read;
use std::path::Path;

use prost::Message;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use weftora_story::DialogueSource;
use yarnspinner_core::prelude::{Library, OpCode, Operand, OperandValue, Program};

use crate::{CommandSite, CompiledDialogue, split_command};

pub const ARTIFACT_FORMAT: u32 = 1;
pub const ARTIFACT_FILE: &str = "compiled/dialogue.json";
pub const COMPILER_ID: &str = "yarnspinner-rust";
pub const COMPILER_VERSION: &str = "0.9.0";
pub const MAX_ARTIFACT_BYTES: usize = 16 * 1024 * 1024;
const MAX_NODES: usize = 1024;
const MAX_NODE_INSTRUCTIONS: usize = 4096;
const MAX_TOTAL_INSTRUCTIONS: usize = 65536;
const MAX_SUBSTITUTIONS: f32 = 64.0;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Artifact {
    dialogue_artifact_format: u32,
    compiler: CompilerIdentity,
    input_hash: String,
    body_sha256: String,
    body: Box<serde_json::value::RawValue>,
}

#[derive(Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CompilerIdentity {
    id: String,
    version: String,
}

impl Default for CompilerIdentity {
    fn default() -> Self {
        Self {
            id: COMPILER_ID.into(),
            version: COMPILER_VERSION.into(),
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Body {
    program: Vec<u8>,
    sites: Vec<CommandSite>,
    strings: BTreeMap<String, String>,
}

/// Domain-tagged SHA-256 of the source count and length-prefixed paths/text in manifest order.
pub fn input_hash(sources: &[DialogueSource]) -> String {
    let mut hash = Sha256::new();
    hash.update(b"weftora-dialogue-input-v1\0");
    hash.update((sources.len() as u64).to_le_bytes());
    for source in sources {
        for part in [source.file.as_bytes(), source.contents.as_bytes()] {
            hash.update((part.len() as u64).to_le_bytes());
            hash.update(part);
        }
    }
    format!("{:x}", hash.finalize())
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

impl CompiledDialogue {
    /// Compiler entry point. Runtime consumers should load a versioned artifact instead.
    pub fn from_parts(
        program: Program,
        strings: BTreeMap<String, String>,
        sites: Vec<CommandSite>,
        input_hash: String,
    ) -> Result<Self, String> {
        let node_files = sites
            .iter()
            .filter(|site| !site.file.is_empty())
            .map(|site| (site.node.clone(), site.file.clone()))
            .collect();
        let dialogue = Self {
            node_files,
            program,
            strings,
            sites,
            input_hash,
        };
        dialogue.validate()?;
        Ok(dialogue)
    }

    pub fn input_hash(&self) -> &str {
        &self.input_hash
    }

    pub fn to_artifact_bytes(&self) -> Result<Vec<u8>, String> {
        let body = serde_json::value::to_raw_value(&Body {
            program: self.program.encode_to_vec(),
            strings: self.strings.clone(),
            sites: self.sites.clone(),
        })
        .map_err(|e| e.to_string())?;
        let body_sha256 = sha256(body.get().as_bytes());
        let mut bytes = serde_json::to_vec(&Artifact {
            dialogue_artifact_format: ARTIFACT_FORMAT,
            compiler: CompilerIdentity::default(),
            input_hash: self.input_hash.clone(),
            body_sha256,
            body,
        })
        .map_err(|e| e.to_string())?;
        bytes.push(b'\n');
        if bytes.len() > MAX_ARTIFACT_BYTES {
            return Err("compiled dialogue exceeds 16 MiB".into());
        }
        Ok(bytes)
    }

    pub fn from_artifact_bytes(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > MAX_ARTIFACT_BYTES {
            return Err("compiled dialogue exceeds 16 MiB".into());
        }
        // Probe independently so future bodies need not match today's schema.
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Header {
            dialogue_artifact_format: u32,
        }
        let header: Header =
            serde_json::from_slice(bytes).map_err(|e| format!("invalid dialogue artifact: {e}"))?;
        if header.dialogue_artifact_format != ARTIFACT_FORMAT {
            return Err(format!(
                "unsupported dialogueArtifactFormat {}; supported: {ARTIFACT_FORMAT}",
                header.dialogue_artifact_format
            ));
        }
        let artifact: Artifact =
            serde_json::from_slice(bytes).map_err(|e| format!("invalid dialogue artifact: {e}"))?;
        if artifact.compiler != CompilerIdentity::default() {
            return Err(format!(
                "unsupported dialogue compiler {} {}; expected {COMPILER_ID} {COMPILER_VERSION}",
                artifact.compiler.id, artifact.compiler.version
            ));
        }
        if artifact.input_hash.len() != 64
            || !artifact
                .input_hash
                .bytes()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
        {
            return Err("invalid dialogue inputHash".into());
        }
        if sha256(artifact.body.get().as_bytes()) != artifact.body_sha256 {
            return Err("dialogue bodySha256 mismatch".into());
        }
        let body: Body = serde_json::from_str(artifact.body.get())
            .map_err(|e| format!("invalid dialogue body: {e}"))?;
        Self::from_parts(
            Program::decode(body.program.as_slice())
                .map_err(|e| format!("invalid Yarn program: {e}"))?,
            body.strings,
            body.sites,
            artifact.input_hash,
        )
    }

    /// Bounded even when a file grows after opening.
    pub fn load_artifact(path: &Path) -> Result<Self, String> {
        let file = std::fs::File::open(path).map_err(|e| {
            format!(
                "{}: {e}; use `weftora run <story-dir>` to compile a preview",
                path.display()
            )
        })?;
        let mut bytes = Vec::new();
        file.take(MAX_ARTIFACT_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        Self::from_artifact_bytes(&bytes).map_err(|e| format!("{}: {e}", path.display()))
    }

    pub fn verify_sources(&mut self, sources: &[DialogueSource]) -> Result<(), String> {
        if self.input_hash != input_hash(sources) {
            return Err("stale dialogue artifact: inputHash does not match the Story sources; use `weftora run` or export again".into());
        }
        for source in sources {
            let mut body = false;
            for line in source.contents.lines() {
                let line = line.trim_start_matches('\u{FEFF}').trim();
                if line == "---" {
                    body = true;
                } else if line == "===" {
                    body = false;
                } else if !body
                    && let Some(name) = line.strip_prefix("title:").map(str::trim)
                    && self.program.nodes.contains_key(name)
                {
                    self.node_files.insert(name.into(), source.file.clone());
                }
            }
        }
        if sources.len() == 1 {
            for node in self.program.nodes.keys() {
                self.node_files
                    .entry(node.clone())
                    .or_insert_with(|| sources[0].file.clone());
            }
        }
        Ok(())
    }

    fn validate(&self) -> Result<(), String> {
        if self.program.nodes.is_empty() || self.program.nodes.len() > MAX_NODES {
            return Err("dialogue must contain 1..1024 nodes".into());
        }
        if self.program.initial_values.len() > 2048 {
            return Err("dialogue exceeds 2048 variable declarations".into());
        }
        for (id, text) in &self.strings {
            if id.len() > 256 || text.len() > yarnspinner_runtime::resource_limits::MAX_STRING_BYTES
            {
                return Err("dialogue string ID/text exceeds 256 bytes/16 KiB".into());
            }
        }
        for (name, value) in &self.program.initial_values {
            if name.len() > 512 {
                return Err("Yarn variable name exceeds 512 bytes".into());
            }
            if name.starts_with("$Yarn.Internal.")
                && (!name
                    .strip_prefix("$Yarn.Internal.Visiting.")
                    .is_some_and(|node| self.program.nodes.contains_key(node))
                    || !matches!(&value.value, Some(OperandValue::FloatValue(_))))
            {
                return Err("unsupported internal Yarn declaration".into());
            }
            validate_operand(value).map_err(|e| format!("Yarn declaration `{name}`: {e}"))?;
        }
        let mut library = Library::standard_library();
        library.extend(crate::queries::library());
        let mut total = 0;
        let mut commands = Vec::new();
        let mut checkpoints = std::collections::BTreeSet::new();
        let checkpoint_nodes: std::collections::BTreeSet<_> = self
            .sites
            .iter()
            .filter(|s| s.command.name == "checkpoint")
            .map(|s| s.node.as_str())
            .collect();
        for (name, node) in &self.program.nodes {
            if name.len() > 256
                || name != &node.name
                || node.instructions.is_empty()
                || node.instructions.len() > MAX_NODE_INSTRUCTIONS
            {
                return Err(format!(
                    "node `{name}`: mismatched name or instruction count outside 1..4096"
                ));
            }
            total += node.instructions.len();
            if total > MAX_TOTAL_INSTRUCTIONS {
                return Err("dialogue exceeds 65536 instructions".into());
            }
            for (label, offset) in &node.labels {
                if label.len() > 256 {
                    return Err("Yarn label exceeds 256 bytes".into());
                }
                if *offset < 0 || *offset as usize >= node.instructions.len() {
                    return Err(format!(
                        "node `{name}`: label `{label}` has invalid offset {offset}"
                    ));
                }
            }
            for (pc, instruction) in node.instructions.iter().enumerate() {
                let fail = |message: &str| format!("node `{name}`, instruction {pc}: {message}");
                let op =
                    OpCode::try_from(instruction.opcode).map_err(|_| fail("unknown opcode"))?;
                let expected: &[char] = match op {
                    OpCode::RunLine | OpCode::RunCommand => &['s', 'n'],
                    OpCode::AddOption => &['s', 's', 'n', 'b'],
                    OpCode::JumpTo
                    | OpCode::JumpIfFalse
                    | OpCode::PushString
                    | OpCode::CallFunc
                    | OpCode::PushVariable
                    | OpCode::StoreVariable => &['s'],
                    OpCode::PushFloat => &['n'],
                    OpCode::PushBool => &['b'],
                    OpCode::PushNull => return Err(fail("PushNull is unsupported")),
                    _ => &[],
                };
                if instruction.operands.len() != expected.len() {
                    return Err(fail("invalid operand count"));
                }
                for (operand, expected) in instruction.operands.iter().zip(expected) {
                    validate_operand(operand).map_err(|e| fail(&e))?;
                    if !matches!(
                        (&operand.value, expected),
                        (Some(OperandValue::StringValue(_)), 's')
                            | (Some(OperandValue::FloatValue(_)), 'n')
                            | (Some(OperandValue::BoolValue(_)), 'b')
                    ) {
                        return Err(fail("invalid operand type"));
                    }
                }
                let string = |i: usize| match &instruction.operands[i].value {
                    Some(OperandValue::StringValue(s)) => s.as_str(),
                    _ => unreachable!("checked above"),
                };
                if matches!(op, OpCode::RunLine | OpCode::AddOption)
                    && !self.strings.contains_key(string(0))
                {
                    return Err(fail("missing string ID"));
                }
                let label_index = match op {
                    OpCode::JumpTo | OpCode::JumpIfFalse => Some(0),
                    OpCode::AddOption => Some(1),
                    _ => None,
                };
                if let Some(i) = label_index
                    && !node.labels.contains_key(string(i))
                {
                    return Err(fail("missing destination label"));
                }
                if matches!(op, OpCode::PushVariable | OpCode::StoreVariable) {
                    let name = string(0);
                    if name.len() > 512 {
                        return Err(fail("variable name exceeds 512 bytes"));
                    }
                    if name.starts_with("$Yarn.Internal.")
                        && !name
                            .strip_prefix("$Yarn.Internal.Visiting.")
                            .is_some_and(|node| self.program.nodes.contains_key(node))
                    {
                        return Err(fail("unsupported internal Yarn variable"));
                    }
                }
                if matches!(op, OpCode::PushVariable | OpCode::StoreVariable)
                    && !self.program.initial_values.contains_key(string(0))
                    && (op == OpCode::PushVariable || !string(0).starts_with("$Yarn.Internal."))
                {
                    return Err(fail("undeclared variable"));
                }
                if op == OpCode::CallFunc
                    && !library.contains_function(string(0))
                    && !matches!(string(0), "visited" | "visited_count")
                {
                    return Err(fail("unknown function"));
                }
                let count_index = match op {
                    OpCode::RunLine | OpCode::RunCommand => Some(1),
                    OpCode::AddOption => Some(2),
                    _ => None,
                };
                if let Some(i) = count_index
                    && let Some(OperandValue::FloatValue(n)) = instruction.operands[i].value
                    && (n.fract() != 0.0 || !(0.0..=MAX_SUBSTITUTIONS).contains(&n))
                {
                    return Err(fail("invalid substitution count (0..=64 required)"));
                }
                if op == OpCode::CallFunc
                    && checkpoint_nodes.contains(name.as_str())
                    && matches!(
                        string(0),
                        "random" | "random_range" | "random_range_float" | "dice"
                    )
                {
                    return Err(fail(
                        "random functions are not supported in checkpoint nodes",
                    ));
                }
                if op == OpCode::RunCommand {
                    let mut parts = split_command(string(0));
                    if parts.first().is_some_and(|name| name == "checkpoint") {
                        if pc != 0 || parts.len() != 2 || instruction.read_operand::<f32>(1) != 0.0
                        {
                            return Err(fail(
                                "checkpoint must be a literal first instruction with one ID",
                            ));
                        }
                        let id = &parts[1];
                        if id.len() > 128 || weftora_core::state::validate_key(id).is_err() {
                            return Err(fail("invalid checkpoint ID"));
                        }
                        if !checkpoints.insert(id.clone()) {
                            return Err(fail("duplicate checkpoint ID"));
                        }
                    }
                    if !parts.is_empty() {
                        commands.push((
                            name.as_str(),
                            crate::StoryCommand {
                                name: parts.remove(0),
                                args: parts,
                            },
                        ));
                    }
                }
            }
        }
        if commands.len() != self.sites.len()
            || commands
                .iter()
                .zip(&self.sites)
                .any(|((node, command), site)| *node != site.node || command != &site.command)
        {
            return Err("dialogue command sites disagree with bytecode".into());
        }
        Ok(())
    }
}

fn validate_operand(operand: &Operand) -> Result<(), String> {
    match &operand.value {
        Some(OperandValue::FloatValue(n)) if !n.is_finite() => Err("non-finite number".into()),
        Some(OperandValue::StringValue(text))
            if text.len() > yarnspinner_runtime::resource_limits::MAX_STRING_BYTES =>
        {
            Err("string operand exceeds 16 KiB".into())
        }
        None => Err("missing operand value".into()),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const FIXTURE: &[u8] = include_bytes!("../tests/fixtures/budget.json");

    fn tamper(edit: impl FnOnce(&mut Program, &mut Body)) -> Vec<u8> {
        let mut artifact: Artifact = serde_json::from_slice(FIXTURE).unwrap();
        let mut body: Body = serde_json::from_str(artifact.body.get()).unwrap();
        let mut program = Program::decode(body.program.as_slice()).unwrap();
        edit(&mut program, &mut body);
        body.program = program.encode_to_vec();
        artifact.body = serde_json::value::to_raw_value(&body).unwrap();
        artifact.body_sha256 = sha256(artifact.body.get().as_bytes());
        serde_json::to_vec(&artifact).unwrap()
    }

    #[test]
    fn supported_artifact_round_trips_without_byte_changes() {
        let loaded = CompiledDialogue::from_artifact_bytes(FIXTURE).unwrap();
        assert_eq!(loaded.to_artifact_bytes().unwrap(), FIXTURE);
    }

    #[test]
    fn future_format_takes_priority_over_missing_current_fields() {
        let error = CompiledDialogue::from_artifact_bytes(
            br#"{"dialogueArtifactFormat":999,"futureBody":[]}"#,
        )
        .unwrap_err();
        assert!(error.contains("unsupported dialogueArtifactFormat 999"));
    }

    #[test]
    fn unknown_wrapper_fields_compiler_and_body_corruption_are_rejected() {
        let mut artifact: serde_json::Value = serde_json::from_slice(FIXTURE).unwrap();
        artifact["compiler"]["version"] = "99.0.0".into();
        assert!(
            CompiledDialogue::from_artifact_bytes(&serde_json::to_vec(&artifact).unwrap())
                .unwrap_err()
                .contains("unsupported dialogue compiler")
        );
        artifact["compiler"]["version"] = COMPILER_VERSION.into();
        artifact["unexpected"] = true.into();
        assert!(
            CompiledDialogue::from_artifact_bytes(&serde_json::to_vec(&artifact).unwrap())
                .unwrap_err()
                .contains("unknown field")
        );
        artifact.as_object_mut().unwrap().remove("unexpected");
        artifact["body"]["strings"]["budget-first"] = "tampered".into();
        assert!(
            CompiledDialogue::from_artifact_bytes(&serde_json::to_vec(&artifact).unwrap())
                .unwrap_err()
                .contains("bodySha256 mismatch")
        );
    }

    #[test]
    fn malformed_bytecode_is_rejected_even_with_a_recomputed_checksum() {
        for (case, expected) in [
            (0, "unknown opcode"),
            (1, "invalid operand count"),
            (2, "invalid operand type"),
            (3, "missing string ID"),
            (4, "invalid offset"),
            (5, "PushNull"),
            (6, "missing destination label"),
            (7, "undeclared variable"),
            (8, "invalid substitution count"),
            (9, "outside 1..4096"),
            (10, "command sites disagree"),
            (11, "unknown function"),
        ] {
            let bytes = tamper(|program, body| {
                let node = program.nodes.get_mut("Start").unwrap();
                let instruction = &mut node.instructions[0];
                match case {
                    0 => instruction.opcode = 999,
                    1 => instruction.operands.clear(),
                    2 => instruction.operands[0] = true.into(),
                    3 => body.strings.clear(),
                    4 => {
                        node.labels.insert("bad".into(), -1);
                    }
                    5 => {
                        instruction.opcode = OpCode::PushNull as i32;
                        instruction.operands.clear();
                    }
                    6 => {
                        instruction.opcode = OpCode::JumpTo as i32;
                        instruction.operands = vec!["missing".to_string().into()];
                    }
                    7 => {
                        instruction.opcode = OpCode::PushVariable as i32;
                        instruction.operands = vec!["$missing".to_string().into()];
                    }
                    8 => instruction.operands[1] = (-1.0f32).into(),
                    9 => node.instructions.clear(),
                    10 => instruction.opcode = OpCode::RunCommand as i32,
                    11 => {
                        instruction.opcode = OpCode::CallFunc as i32;
                        instruction.operands = vec!["missing".to_string().into()];
                    }
                    _ => unreachable!(),
                }
            });
            let error = CompiledDialogue::from_artifact_bytes(&bytes).unwrap_err();
            assert!(error.contains(expected), "case {case}: {error}");
        }
    }

    #[test]
    fn raw_body_checksum_is_independent_of_wrapper_key_order() {
        let mut artifact: Artifact = serde_json::from_slice(FIXTURE).unwrap();
        let body: Body = serde_json::from_str(artifact.body.get()).unwrap();
        artifact.body =
            serde_json::value::RawValue::from_string(serde_json::to_string_pretty(&body).unwrap())
                .unwrap();
        artifact.body_sha256 = sha256(artifact.body.get().as_bytes());
        let mut bytes = serde_json::to_vec(&artifact).unwrap();
        assert!(CompiledDialogue::from_artifact_bytes(&bytes).is_ok());
        // Whitespace is part of the raw body's checksum, but not a wrapper formatting requirement.
        bytes.insert(1, b' ');
        assert!(CompiledDialogue::from_artifact_bytes(&bytes).is_ok());
    }

    #[test]
    fn file_limit_and_source_identity_are_enforced() {
        assert!(
            CompiledDialogue::from_artifact_bytes(&vec![b' '; MAX_ARTIFACT_BYTES + 1])
                .unwrap_err()
                .contains("exceeds 16 MiB")
        );
        let mut loaded = CompiledDialogue::from_artifact_bytes(FIXTURE).unwrap();
        assert!(
            loaded
                .verify_sources(&[])
                .unwrap_err()
                .contains("stale dialogue artifact")
        );
        let a = DialogueSource {
            file: "a.yarn".into(),
            contents: "text".into(),
        };
        let b = DialogueSource {
            file: "b.yarn".into(),
            contents: "text".into(),
        };
        assert_ne!(
            input_hash(&[a.clone(), b.clone()]),
            input_hash(&[b, a.clone()])
        );
        assert_ne!(
            input_hash(&[a]),
            input_hash(&[DialogueSource {
                file: "a.yarn".into(),
                contents: "edited".into()
            }])
        );
    }
    #[test]
    fn checkpoint_policy_requires_review_when_functions_change() {
        let mut library = Library::standard_library();
        library.extend(crate::queries::library());
        let mut names: Vec<_> = library.names().filter(|name| !name.contains('.')).collect();
        names.sort_unstable();
        assert_eq!(
            names,
            [
                "bool",
                "ceil",
                "dec",
                "decimal",
                "dice",
                "floor",
                "format_invariant",
                "inc",
                "int",
                "number",
                "random",
                "random_range",
                "random_range_float",
                "round",
                "round_places",
                "state_list_contains",
                "state_list_len",
                "state_map_contains_key",
                "state_map_len",
                "string"
            ]
        );
    }
}
