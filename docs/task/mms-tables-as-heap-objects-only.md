# MMS tables as heap objects only

Status: in progress

## Goal

Make user-authored MMS tables a single runtime representation:

- authored tables should always lower to heap-backed objects
- they should not sometimes exist as inline `Value::Map` and sometimes as `Value::Object`

This is primarily about authored data structures:

- app state
- event payload shaping
- panel/view models
- future struct instances

These are not just temporary flags or evaluator scratch values. They are durable mutable data.

## Decision

For user-authored MMS code, tables should be heap objects only.

That means:

- table literals should allocate an object/map in the MMS heap
- bindings to tables should point at that heap object
- field reads and field writes should operate through that object identity
- future `struct` instances should lower onto the same heap-object model

Inline `Value::Map` should not remain the authored/runtime representation for mutable MMS table
state.

## Why this is the right model

### 1. Tables are data structures, not just value bags

Authored MMS tables are already being used as real state:

- `app_state`
- event payloads
- Rust-to-MMS structured models

Those want stable identity and mutation semantics.

### 2. It simplifies reassignment semantics

The current reassignment work in:

- [`src/meow_meow/evaluator.rs`](/home/rei/_/cat-engine/src/meow_meow/evaluator.rs:812)

needed:

- `assign_retarget(...)`
- `flatten_assign_path(...)`
- `assign_into_value(...)`

That code is not conceptually complex because dotted assignment is hard.

It is mostly paying for representation mismatch:

- plain identifier assignment is one write into the environment
- dotted assignment is root lookup + path walk + nested update
- the current implementation must also branch on both:
  - `Value::Map`
  - `Value::Object(id)`

If authored tables are heap objects only, dotted assignment still needs path walking, but it no
longer needs to support two different table storage models.

### 3. It gives closure capture and aliasing a coherent story

If tables are values sometimes copied inline and sometimes heap-backed, it becomes unclear when two
names refer to the same mutable state.

Heap-object tables make the model clearer:

- rebinding a variable changes which object it points to
- mutating a field changes the shared object behind that binding
- closures can capture object references naturally

This is not just theoretical. The original
[`examples/table-field-reassign.mms`](/home/rei/_/cat-engine/examples/table-field-reassign.mms:1)
demonstrated the failure mode:

- the `TextInputChanged` handler updates `app_state.draft_text`
- the `Click` handler on the send button still sees the older captured value
- so `status` and `send_count` can update while `text` / `draft_text` do not reflect what was
  typed

That is the exact kind of cross-handler mutable state sharing this task is meant to fix.

### 4. It aligns with future structs

We already want:

- named `struct` declarations
- typed fields
- Rust-like type information

The cleanest route is:

- anonymous tables and struct instances share one runtime object model
- structs add type metadata and syntax over that runtime

That is much cleaner than keeping authored tables as ad hoc inline maps and later introducing a
different object representation for structs.

## Runtime rule
For authored MMS evaluation:

1. evaluating a table literal allocates a heap map object
2. the expression result is `Value::Object(id)`
3. field access reads from that object
4. field reassignment mutates that object
5. passing a table to functions passes the object reference
6. returning a table returns the object reference
7. closures and runtime blocks must keep the referenced heap alive

This is now the normal rule for user-authored code.

## Current state

Implemented:

- authored table literals lower to `Value::Object(id)`
- dotted field reads/writes operate on object-backed tables
- exported/runtime closures keep a shared heap handle alive
- separately invoked closures can observe the same table mutation history

Still mixed / follow-up:

- some internal Rust helper paths still construct `Value::Map`
- host event payload shaping should be normalized onto the same object model
- reassignment code still has transitional `Value::Map` support for non-authored inputs

## What can still stay non-object

This decision is about authored MMS data structures.

It does not necessarily require every internal helper surface to disappear immediately.

Possible transitional allowance:

- internal Rust helpers may still temporarily construct `Value::Map`
- but authored table literals and authored structured values should normalize into heap objects at
  the evaluator boundary

Longer term, even internal structured payloads should prefer the same object-backed model where
practical.

## Implementation phases

### Phase 1: normalize authored table literals onto heap objects

Work:

- change authored table-literal evaluation so it allocates heap map objects
- audit field access paths so they treat heap-object tables as the primary model
- update tests that currently expect authored table literals to evaluate as `Value::Map`

Exit criteria:

- authored table literals no longer surface as inline `Value::Map`

Status:

- done

### Phase 2: simplify reassignment around object-backed tables

Work:

- keep the existing `identifier` vs `dot path` assignment split
- remove or reduce `Value::Map` branching from table-field assignment
- ensure nested `a.b.c = x` updates object-backed tables predictably

Exit criteria:

- dotted field assignment works without dual-representation branching for authored tables

Status:

- mostly done for authored tables
- transitional `Value::Map` support remains for non-authored/internal paths

### Verification after heap-object migration

Retest:

- [`examples/table-field-reassign.mms`](/home/rei/_/cat-engine/examples/table-field-reassign.mms:1)

Expected behavior after the migration:

- typing into the `TextInput` updates `app_state.draft_text`
- rerender shows the updated `draft_text` immediately
- clicking `send` copies `app_state.draft_text` into `app_state.text`
- the click handler sees the same shared table object state that the text-input handler mutated

If that example still shows stale handler-local table state after the migration, the heap-object
change is incomplete.

### Phase 3: normalize structured host payloads

Work:

- decide whether host event payloads and Rust-provided structured values should also allocate heap
  objects before entering authored MMS
- reduce remaining “sometimes map, sometimes object” behavior at host boundaries

Exit criteria:

- structured payloads exposed to authored MMS follow the same model as authored tables

### Phase 4: build structs on top of the same runtime

Work:

- add `struct` syntax and type metadata
- lower struct instances onto the same heap-object table runtime

Exit criteria:

- structs do not introduce a second data-object runtime

## Non-goals

This task does not require:

- a VM
- transpiling MMS to Rust
- full static typing first
- a complete copy-on-write or persistent-data design

The immediate goal is just to stop splitting authored table state across two runtime
representations.

## Related

- [docs/meow_meow/draft/mms-types-phases-and-language-server.md](/home/rei/_/cat-engine/docs/meow_meow/draft/mms-types-phases-and-language-server.md:1)
- [docs/draft/mms-records-and-rust-interop.md](/home/rei/_/cat-engine/docs/draft/mms-records-and-rust-interop.md:1)
- [docs/task/mms-structs-for-event-payloads-and-data-modeling.md](/home/rei/_/cat-engine/docs/task/mms-structs-for-event-payloads-and-data-modeling.md:1)
