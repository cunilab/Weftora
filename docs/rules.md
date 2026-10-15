---
id: WFT-RULES-001
title: Story Rules
status: proposed
---

# Story Rules

Phase 4 uses typed scalar conditions, deterministic events and ordered state actions in the headless `weftora-flow` crate, following [architecture §5](./architecture.md#5-runtime-model). [ADR 0008](./adr/0008-declarative-events.md) defines their JSONC Story format and shared dialogue notification boundary. Story-defined commands, external UI/host-write bridging, collections and Rhai remain to implement. Existing Stories without rules retain their behavior.

## Story files

Manifest v1 declares `"events": ["events.jsonc"]` and requires `rules.events/1`. Each source has `rulesSchema: 1` and an event array:

```json
{
  "rulesSchema": 1,
  "events": [{
    "id": "award",
    "trigger": "state.changed.input",
    "condition": {
      "op": "compare", "operator": "==",
      "left": {"state": "input"}, "right": {"literal": true}
    },
    "actions": [{"do": "state.add", "key": "points", "amount": 10}]
  }]
}
```

Declare referenced state keys in the manifest. Conditions use `compare` (the six operators below), `and`/`or` with a `conditions` array, or `not` with a `condition`. Operands are either `{"state":"key"}` or `{"literal":value}`. Actions are `state.set` (`key`, `value`), `state.add` (`key`, `amount`), `state.remove`/`state.reset` (`key`), or `signal.emit` (`id`, `value`). Event `priority` defaults to zero and `condition` may be omitted. Unknown fields and duplicate keys are rejected.

`weftora check` loads and validates rule files. Ordinary play and headless replay share the Story-aware runner; see [rules-demo](../examples/rules-demo/README.md) for a runnable save-enabled example. Rules react to notifications produced while dialogue runs, after the current VM event and before checkpoint capture/display arming. Already interpolated text retains its prior value; later dialogue reads the settled state. The source/scope/resource restrictions and candidate publication contract are defined in ADR 0008.

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

## Events and ordered actions

`events::Scheduler::new` validates all definitions against the supplied State before constructing an immutable trigger index. Event IDs are unique canonical keys; trigger IDs use the same grammar and may be shared. Exact signal IDs select matching events. Higher signed priorities run first, with declaration order breaking ties. Priority extremes do not wrap. Definitions are bounded to 256 events and 256 actions per event; condition limits also apply.

At `dispatch`, all definitions are checked again against the supplied State before any write, so passing a different state schema cannot partially execute an invalid ruleset. Each matching event evaluates its condition against current state immediately before its actions. Earlier writes are visible to subsequent conditions. A false condition skips its actions and is recorded in the trace.

Actions execute in declaration order:

| Action | Behavior |
| --- | --- |
| `Set` | Write a literal of the key's declared type |
| `Add` | Use Core's checked numeric addition: int to int, or int/float to float |
| `Remove` | Remove a key's current value, retaining its declaration; an absent value is an error |
| `Reset` | Restore the declared default, including a removed value |
| `Emit` | Queue a validated Core signal, including its scalar payload |

State actions require declared keys. Types and literal finiteness are validated using Core's state operations; addition is type-checked against numeric zero so a large default does not wrongly reject a valid later addition. Integer overflow and nonfinite float results are runtime errors.

Nested signals use a local FIFO queue. They are delivered after every matching event for the current signal, including events of lower priority. Signals without handlers are valid. The scheduler shares Core's signal type and validation, and tests pin its nested delivery order against `SignalBus`; it does not register hidden bus subscriptions or callbacks.

The caller supplies one dispatch budget. One unit is charged before each signal is enqueued (including the initial signal), each matching event is evaluated (including false conditions), and each action is attempted. `Emit` costs both an action unit and an emission unit. Emissions are charged at enqueue time to bound queued work as well as cycles. Each event's condition evaluation is additionally bounded by its AST limits. Definition validation has its own event/action/condition bounds and occurs before the dispatch budget is used.

On the first error, remaining actions/events and queued signals are discarded. `DispatchError` contains the error and the completed trace prefix; action failures identify the event and zero-based action index. Budget errors also identify the next event/action when available. A completed `Emit` trace means the signal was queued; only a `Trace::Signal` entry records its delivery. Earlier writes **and their queued state-change notifications remain**. This is not a transaction. A new dispatch starts with a fresh queue and budget.

Low-level dispatch only mutates its supplied `&mut State`; definitions contain no external handlers or persistent script state. It never drains notifications or automatically generates `state.changed`. The Story `Rules::settle` bridge handles new notifications and their feedback under one shared budget. The dialogue runner applies it equally to initial execution and isolated checkpoint candidates; failed candidates publish no state or notifications.

## Verification and remaining work

Run `cargo test -p weftora-flow --locked` for operator/type tables, nested boolean logic, invalid skipped branches, nonfinite values, live state and removal, i64 precision, no condition mutation, and depth/node boundaries. Event tests replay the priority/condition/action/FIFO golden fixture in ten fresh runs and pin a cycle's trace prefix; they also cover Core bus ordering parity, action/condition errors, notification preservation, schema drift, dispatch on a cloned state, repeated dispatches and definition/budget boundaries. `cargo test -p xtask --locked` includes a negative dependency fixture for Flow; `cargo xtask deps` checks the actual graph. Flow must never depend on Bevy, Yarn, Rhai, apps or Stories.

Story/CLI rules tests cover authoring diagnostics, repeated real execution and save replay; the renderer candidate test covers live-state/file preservation on rule failure. [Phase 4 evidence](./evidence/phase-4.md) records results for the automated portions of P4-01/P4-02 and initial ordered-action semantics in the [success criteria](./success-criteria.md#phase-4--story-defined-rules). Generic Story commands, UI/host-write bridging and Rhai remain separate work; no Phase 4 acceptance is claimed. Windows CI runs workspace tests; native checks remain queued in [issue #5](https://github.com/cunilab/Weftora/issues/5) without blocking development.
