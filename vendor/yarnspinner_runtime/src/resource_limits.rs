//! Fixed bounds for the Weftora runtime fork; instruction budgets alone do not bound memory.
/// Maximum values retained on the VM stack.
pub const MAX_STACK: usize = 1024;
/// Maximum options accumulated before selection.
pub const MAX_OPTIONS: usize = 256;
/// Maximum events accumulated before a yielding instruction.
pub const MAX_EVENTS: usize = 1024;
/// Maximum UTF-8 bytes in a value, template or expanded line/command.
pub const MAX_STRING_BYTES: usize = 16 * 1024;
