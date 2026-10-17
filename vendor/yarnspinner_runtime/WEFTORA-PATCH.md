# Weftora runtime patch

Vendored from yarnspinner_runtime 0.9.0 in the Cargo registry. LICENSE-MIT and LICENSE-APACHE were copied from the upstream repository (YarnSpinnerTool/YarnSpinner-Rust) because the published
crate contains no LICENSE files; its unmodified manifests retain the upstream
`MIT OR Apache-2.0` license declaration. The root workspace selects this copy with `[patch.crates-io]`.

The VM counts instructions per continue call, including jumps between nodes.
The default budget is 100,000 and `Dialogue::set_instruction_budget` configures
it (zero permits no instructions). Exhaustion stops the VM, clears batched
events, and returns `DialogueError::InstructionBudgetExceeded` with the node
name and budget. Both continue APIs share this VM loop. Weftora treats runtime
failures as terminal to avoid resuming partially executed state.

`Dialogue::instruction_budget` and `Dialogue::last_instruction_count` expose
configuration and instruction accounting. The VM records instructions attempted
by each continue call, including on error. Weftora uses this count to carry a
100,000-instruction budget across consecutive commands, resetting only when a
Line, Choices, or End is returned. It also caps consecutive commands and
non-blocking continue calls at 1,000. This prevents command/jump loops from
escaping the per-call VM budget. The per-call budget is restored after each call.

Why: a node containing only `<<jump Start>>` never reaches a presentation event
and previously hung the caller indefinitely. A wall-clock timeout cannot safely
interrupt the synchronous VM.

Upstream issue to file (not yet filed): YarnSpinnerTool/YarnSpinner-Rust —
“Bound synchronous VM execution with a configurable per-continue instruction
budget”. Include the self-jump reproducer and request this API/error upstream.

The fixed memory guards in resource_limits.rs also cap stack values at 1024,
pending options at 256, batched events at 1024 and UTF-8 values/expanded text at
16 KiB. ResourceBudgetExceeded and InvalidExecution identify the executing node;
ExecutionError preserves that node before VM cleanup for other failures.
Function arity and available arguments are checked before collecting them.
Substitution output is bounded before every replacement; empty stack accesses
fail before popping. String.Add may allocate at most 32 KiB from two bounded
inputs before its result is checked. These guards complement the adapter's
storage/notification limits; they do not impose a process RSS budget.

`Dialogue::continue_with_function_handler` adds an optional fallible host function
handler. `None` uses the registered implementation; an error stops the VM before
pushing a return value or executing later instructions and clears batched events.
Returned values must match the registered return type (or Any). Existing continue
APIs delegate directly to their registered functions. Weftora uses this hook for
read-only canonical collection queries, avoiding error sentinels and partial VM
execution after a failed query. Compiler/artifact signatures remain in the adapter;
this runtime patch knows no Story, Core or collection types.
