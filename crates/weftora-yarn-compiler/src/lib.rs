//! Yarn source compilation for authoring tools; never a player dependency.
use weftora_api::{Diagnostic, StoryCommand};
use weftora_story::DialogueSource;
use weftora_yarn::{CommandSite, CompiledDialogue, input_hash, split_command};
use yarnspinner_compiler::prelude::{Compiler, DiagnosticSeverity, File as YarnFile};

/// Scan commands using the pinned bytecode opcode.
const OPCODE_RUN_COMMAND: i32 = yarnspinner_core::prelude::OpCode::RunCommand as i32;

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
        .map(|(id, info)| (id.to_string(), info.text))
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

    CompiledDialogue::from_parts(program, strings, sites, input_hash(sources))
        .map_err(|error| vec![Diagnostic::new("", None, error)])
}
