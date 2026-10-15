---
id: WFT-RULES-001
title: Story Rules
status: proposed
---

# Story Rules

Phase 4 starts with typed scalar conditions in the headless `weftora-flow` crate, following [architecture §5](./architecture.md#5-runtime-model). This slice is an in-memory Rust API, not a Story file format or an exposed CLI/player feature. Event scheduling, Story-defined commands, collections and Rhai remain to implement. Existing Stories and saves are unchanged.

## Scalar conditions

`Operand` is a literal canonical `Value` or a canonical state key. `Condition` is a comparison, `And`, `Or` or `Not`. Conditions never write state or consume notifications.

| Operation | Valid operands | Result |
| --- | --- | --- |
| `Eq`, `Ne` (`==`, `!=`) | Two values of the same scalar type | Equality or inequality |
| `Lt`, `Le`, `Gt`, `Ge` (`<`, `<=`, `>`, `>=`) | Two ints, two finite floats or two strings | Ordered comparison |
| `And`, `Or` | One or more conditions | Declaration-order short-circuit evaluation |
| `Not` | One condition | Boolean negation |

Mixed types are errors, including int versus float. Int comparisons retain full i64 precision. Float operands must be finite, including directly constructed literals; negative and positive zero compare equal. String ordering uses Rust's bytewise UTF-8 order, without locale collation. Bool ordering and empty `And`/`Or` are errors.

`Condition::validate` checks all branches against declared state defaults for key existence and type. Removed keys retain their declarations, so they can validate; a runtime read of a removed key fails rather than substituting its default. Undeclared keys and invalid operands fail validation even in a branch that would be skipped.

`Condition::evaluate` first validates the entire tree, then evaluates live state with short-circuiting. A declared but removed key in a skipped branch is not read. The caller receives `ConditionError`; file/location mapping belongs to the future Story rule loader. The evaluator returns the first error. Validation and evaluation share an immutable state borrow, so types cannot change between them.

Validation permits at most 32 condition levels (root is level one) and 256 condition nodes (root included; operands excluded). All branches count, including skipped ones. These are execution limits; this slice does not parse untrusted trees. A future Story loader must bound input size and parsing as well.

## Verification and remaining work

Run `cargo test -p weftora-flow --locked` for operator/type tables, nested boolean logic, invalid skipped branches, nonfinite values, live state and removal, i64 precision, no state mutation, and depth/node boundaries. `cargo test -p xtask --locked` includes a negative dependency fixture for Flow; `cargo xtask deps` checks the actual graph. Flow must never depend on Bevy, Yarn, Rhai, apps or Stories.

These cover the headless portion of P4-01 in the [success criteria](./success-criteria.md#phase-4--story-defined-rules). They do not claim completed Story authoring, static file diagnostics, events, or Phase 4 acceptance. Windows CI runs workspace tests; the macOS headless job also includes Flow. Native Windows checks remain queued in [issue #5](https://github.com/cunilab/Weftora/issues/5) as relevant features land, without blocking headless development.
