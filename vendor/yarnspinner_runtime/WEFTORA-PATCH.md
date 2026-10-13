# Weftora runtime patch

Vendored from yarnspinner_runtime 0.9.0 in the Cargo registry. The published
crate contains no LICENSE files; its unmodified manifests retain the upstream
`MIT OR Apache-2.0` license declaration. The root workspace selects this copy with `[patch.crates-io]`.

The VM counts instructions per continue call, including jumps between nodes.
The default budget is 100,000 and `Dialogue::set_instruction_budget` configures
it (zero permits no instructions). Exhaustion stops the VM, clears batched
events, and returns `DialogueError::InstructionBudgetExceeded` with the node
name and budget. Both continue APIs share this VM loop. Weftora treats runtime
failures as terminal to avoid resuming partially executed state.

Why: a node containing only `<<jump Start>>` never reaches a presentation event
and previously hung the caller indefinitely. A wall-clock timeout cannot safely
interrupt the synchronous VM.

Upstream issue to file (not yet filed): YarnSpinnerTool/YarnSpinner-Rust —
“Bound synchronous VM execution with a configurable per-continue instruction
budget”. Include the self-jump reproducer and request this API/error upstream.
