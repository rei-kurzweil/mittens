# MMS Types, Data Modeling, And Language Server Phases

Status: draft

## Why this doc exists

There are several related MMS efforts in flight:

- user-authored tables
- named structs
- event payload shaping
- optional/default parameters
- typed functions
- call-site validation
- editor diagnostics in VSCode

These are connected, but they should not be conflated into one giant task.

This doc separates:

1. what already works
2. what still has hard gaps
3. which pieces depend on which others
4. a phased plan that can drive both language work and language-server work

## Current implementation status

### Already working

#### Parser / evaluator side

- anonymous table literals parse
  - see `src/meow_meow/tests.rs` `parse_table_literal_binding`
- nested table literals parse
- dot field access parses
  - `settings.theme.label`
- module evaluation supports table literals and field reads
  - see `src/meow_meow/evaluator.rs` tests
- `for entry in { ... }` over table fields works in evaluator/module mode
- host/runtime already has object/map support underneath tables

#### Existing docs already pointing in this direction

- `docs/task/mms-structs-for-event-payloads-and-data-modeling.md`
- `docs/draft/mms-struct-syntax.md`
- `docs/draft/mms-records-and-rust-interop.md`
- `docs/meow_meow/draft/type-system.md`
- `docs/analysis/mms-language-server.md`

### Partially working / inconsistent

- event payloads still have mixed surfaces
  - some host/event work already uses structured internal values
  - XR payload history still shows positional-array legacy in docs/task notes
- user-authored tables work in evaluator/module mode
  - but not every component/materialization path accepts them yet
- tables are usable as read-mostly data
  - but not yet as fully mutable structured state

### Not implemented yet

#### Plain-data authoring gaps

- no table field reassignment syntax like `state.foo = 1`
- no general table mutation API
- no named `struct` declarations in source
- no struct allocation syntax in source
- no typed field declarations in source

#### Type-system gaps

- no `type_analyzer.rs`
- no function parameter type annotations
- no function return type annotations
- no static call validation
- no union type implementation
- no signature help / typed diagnostics

#### Tooling gaps

- no shipped TextMate grammar
- no shipped VSCode extension
- no shipped LSP server
- no workspace symbol/type index for `.mms`

## Important current inconsistency

One seam is especially relevant:

- the evaluator understands table literals and field access
- `src/meow_meow/component_registry.rs::expression_to_value(...)` still rejects
  `Expression::Table(_)`

That means:

- “tables exist in MMS” is true in some paths
- “tables are fully accepted everywhere values flow” is false

This should be called out explicitly in planning, because it affects:

- component factory args
- Rust→MMS interop
- future struct allocation lowering

## Recommended dependency model

These efforts should be staged as a graph, not one monolith.

### Foundation layer: plain data

1. anonymous tables everywhere values are accepted
2. field access everywhere evaluator values are read
3. host event payloads normalized onto table/object values
4. Rust→MMS structured value transport normalized

Without this, typed structs are mostly syntax over an unstable runtime.

### Data-modeling layer

5. table mutation / field reassignment semantics
6. named structs
7. struct allocation
8. struct-to-table runtime lowering rules

Without this, typed authoring cannot express durable app/panel state cleanly.

### Function/API layer

9. optional/default parameters
10. typed function params and returns
11. built-in constructor/method signature metadata

Without this, type analysis cannot validate real-world call sites.

### Static-analysis layer

12. `meow_meow/type_analyzer.rs`
13. arity/type checking for function calls
14. constructor/method signature checking
15. field-access checking on structs/tables where types are known

### Tooling layer

16. TextMate grammar
17. VSCode extension shell
18. LSP diagnostics from parser + type analyzer
19. completions/signature help from shared metadata

## Proposed phases

### Phase A — normalize plain-data runtime

Goal:

- make tables a real, consistent runtime surface before adding more type syntax

Scope:

- allow table literals in all relevant value-lowering/materialization paths
- document current semantics for table iteration and field access
- choose the runtime naming (`Map` vs `Table`) consistently
- convert event payload docs to the table-first model

Concrete current blockers:

- `component_registry.rs::expression_to_value(...)` rejects tables
- some docs still describe event payloads as positional arrays

Exit criteria:

- a table can be authored, passed through module calls, used in component
  factory args where intended, and read consistently

### Phase B — mutable table state

Goal:

- make tables useful for authored state, not just payload snapshots

Scope:

- add table field assignment target syntax
  - likely `state.foo = bar`
- define mutation semantics
  - mutate in place vs copy-on-write
- define closure-capture behavior for mutable tables

Why this phase matters:

- without field reassignment, tables are awkward for real app state
- panel state, reducers, and event payload enrichment all want this

Exit criteria:

- authored scripts can mutate table-backed state intentionally and predictably

### Phase C — structs as typed/authored tables

Goal:

- add user-facing named data shapes on top of the table runtime

Scope:

- `struct` declarations
- struct allocation syntax
- nominal-vs-structural type decision
- field declaration syntax

Recommendation:

- keep structs as a typed layer over tables, not a separate runtime universe

Exit criteria:

- event payloads and panel models can be authored as named data shapes

### Phase D — optional/default parameters

Goal:

- make library and component APIs ergonomic before full type checking

Scope:

- add optional/default params for functions
- keep omitted-arg binding semantics explicit
- define interaction with `null`
- decide whether `param?` means nullable, omittable, or both

Dependency note:

- this phase is related to types, but it does not require the full type system
- it does benefit from Phase A, and often benefits from Phase C

Exit criteria:

- user-authored MMS APIs no longer need repetitive manual `if arg == null`
  fallback logic

### Phase E — type expressions and typed functions

Goal:

- introduce a usable, gradual type layer

Scope:

- type annotations on params, returns, and bindings
- type expression grammar
- nullable shorthand
- decide whether general union types ship now or later

Recommendation on unions:

- implement nullable `T?` early
- defer arbitrary `A | B` unions unless a concrete use case forces them

Why:

- `Any` already covers many early heterogeneous cases
- full unions add complexity to inference, field narrowing, and diagnostics

Exit criteria:

- typed functions can describe real MMS APIs

### Phase F — static analysis

Goal:

- move common MMS errors earlier than runtime

Scope:

- add `src/meow_meow/type_analyzer.rs`
- function arity/type validation
- constructor/method signature validation
- field access validation for typed structs/tables
- produce stable diagnostics with source spans

Shared dependency:

- needs a canonical signature model for built-in constructors/methods

Exit criteria:

- common call-site mistakes are caught before scene execution

### Phase G — VSCode / language server

Goal:

- surface syntax and type errors in editors

Scope:

- TextMate grammar
- extension manifest
- `tower-lsp` language server
- parser diagnostics first
- type diagnostics second
- signature help/completions from shared metadata

Important point:

- LSP does not need to wait for full types
- syntax highlighting and parse diagnostics can ship much earlier

Exit criteria:

- `.mms` files get parse errors, then type errors, directly in VSCode

## Suggested deliverables split

This should be at least two tracks, not one mega-task.

### Track 1 — language/runtime

- tables everywhere
- table mutation
- structs
- optional/default params
- type expressions
- type analyzer

### Track 2 — tooling/editor

- grammar
- extension shell
- diagnostics bridge
- signature metadata
- hover/completion

They should share:

- parser
- AST
- type analyzer
- built-in signature metadata

## Recommended immediate next steps

1. Finish the plain-data runtime story before adding more syntax.
   Specifically: close the gap where evaluator tables work but
   `expression_to_value(...)` still rejects them.

2. Decide whether table field reassignment is the next data-modeling milestone.
   This is the biggest functional gap for user-authored state.

3. Treat optional/default params as a separate design seam, not as a hidden
   component-registry behavior.

4. Keep structs layered over tables.
   Do not invent a separate runtime representation for structs.

5. Create `type_analyzer.rs` only after the runtime/value story is coherent
   enough that the analyzer is not chasing moving targets.

6. Start the editor tooling track in parallel at the syntax layer.
   TextMate grammar and parse-error LSP diagnostics can start before types.

## Working conclusions

### On tables

User-authored tables are not purely hypothetical anymore.

They already work in parser/evaluator/module contexts, including field access
and table iteration. But they are not yet a universally accepted value shape
across all MMS seams, and they are not yet fully mutable authored state.

### On structs

Structs should remain a later, typed layer over the table runtime.

They are important, but they should not ship before the underlying table model
is consistent and mutable enough to support real authored state.

### On optionals and types

Optional/default params are related to types, but should be planned as their
own phase. They can likely land before full type checking, while still feeding
future signature metadata and LSP support.

### On LSP

The language server should be treated as incremental:

1. syntax highlighting
2. parse diagnostics
3. signature metadata
4. static type diagnostics
5. completions/hover/go-to-def

That keeps editor support moving without waiting for the entire type system to
finish.
