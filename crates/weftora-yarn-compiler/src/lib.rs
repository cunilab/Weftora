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
    compiler.extend_library(weftora_yarn::collection_query_library());
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
        let mut diagnostics = Vec::new();
        // Retry only to locate a parser panic; cross-file type failures are irrelevant here.
        for source in sources {
            let mut compiler = Compiler::new();
            compiler.extend_library(weftora_yarn::collection_query_library());
            compiler.add_file(YarnFile {
                file_name: source.file.clone(),
                source: source.contents.clone(),
            });
            if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| compiler.compile()))
                .is_err()
            {
                diagnostics.push(Diagnostic::new(&source.file, None, "the Yarn compiler crashed on malformed input (node unknown); check the dialogue syntax"));
            }
        }
        if diagnostics.is_empty() {
            diagnostics.push(Diagnostic::new(
                "<file unknown>",
                None,
                "the Yarn compiler crashed (node unknown); check the dialogue syntax",
            ));
        }
        return Err(diagnostics);
    };
    let compilation = match outcome {
        Ok(c) => c,
        Err(e) => {
            let diags: Vec<Diagnostic> = e
                .0
                .iter()
                .filter(|d| d.severity == DiagnosticSeverity::Error)
                .map(|d| {
                    let line = d
                        .range
                        .as_ref()
                        .map(|r| r.start.line)
                        .unwrap_or(d.start_line);
                    let file = d
                        .file_name
                        .clone()
                        .unwrap_or_else(|| "<file unknown>".into());
                    let line = line
                        .checked_add(1)
                        .and_then(|line| u32::try_from(line).ok())
                        .filter(|line| *line > 0);
                    let node = line
                        .and_then(|line| {
                            sources
                                .iter()
                                .find(|s| s.file == file)
                                .and_then(|s| weftora_yarn::source_node_at_line(s, line))
                        })
                        .unwrap_or("unknown");
                    Diagnostic::new(file.clone(), line, format!("{} (node '{node}')", d.message))
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

    let provenance: Vec<_> = program
        .nodes
        .keys()
        .filter_map(|name| {
            compilation
                .debug_info
                .get(name)
                .map(|info| (name.clone(), info.file_name.clone()))
        })
        .collect();
    let mut dialogue = CompiledDialogue::from_parts(program, strings, sites, input_hash(sources))
        .map_err(|error| vec![Diagnostic::new("<file unknown>", None, error)])?;
    for (node, file) in provenance {
        dialogue.set_node_file(&node, &file);
    }
    Ok(dialogue)
}
