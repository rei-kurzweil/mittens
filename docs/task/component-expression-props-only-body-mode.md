# Task: Component Expression `props_only` Body Mode

Date: 2026-06-13

Status: planned follow-up / evaluator cleanup task

## Intro

We fixed an immediate regression in MMS `Data { ... }` materialization by
making component-body assignments like:

```mms
Data {
    row_name = row_name
    mode_value = mode_value
}
```

always materialize as component named properties while evaluating a component
expression body.

That fixed the editor settings payload loss:

- authored `Data` fields such as `row_name`, `label`, and `mode_value` now
  survive materialization
- editor settings mode selection can recover its intended semantic payload from
  `DataComponent`

However, the current fix is broader than the actual intended language rule.

Right now the behavior applies to all component expression bodies.
That is too wide.

The actual semantic distinction is narrower:

- some component expression bodies are effectively property bags / leaf nodes
- others are structural blocks that should keep normal statement semantics

We need an explicit component-expression body mode for that distinction.

## Goal

Introduce an explicit component-expression body mode for leaf/property-bag
components so assignment statements inside those bodies are interpreted as
component properties only when that mode is enabled.

Intended end state:

1. leaf-like components opt into a `props_only` body mode
2. structural component bodies keep normal block / variable assignment
   semantics
3. authored `Data { row_name = row_name }` style payloads keep working
4. evaluator behavior is no longer globally overloaded for every component body

## Problem

The current evaluator behavior conflates two different concepts:

- `name = expr` as a normal reassignment in a scoped block
- `name = expr` as a named property inside a component property bag

For `Data` / `Style` / similar leaf-like components, the second meaning is what
we want.

For structural component expressions, that is not always correct.

The recent bug made the distinction visible:

- `row_name = row_name`
- `label = label`
- `mode_value = mode_value`

were previously treated as lexical reassignments because the RHS bindings
already existed in scope, so those fields were not emitted as named component
properties

The temporary fix solved that by making CE-body reassignments always become
named properties, but that should only happen for a subset of component types.

## Proposed design

Use an explicit body-mode flag on component expressions.

Suggested shorter name:

- `props_only`

Reason:

- shorter than `component_property_assignment_only`
- still precise enough for evaluator behavior
- reads naturally as a CE/body semantic mode

Conceptually:

- `props_only = true`
  - body assignment statements become named component properties
  - body is treated as a property bag / leaf body
- `props_only = false`
  - body uses normal block semantics
  - assignment statements remain lexical reassignments unless explicitly
    captured by some other language rule

## Where the flag should live

The key idea is:

- once we know which component AST node we are constructing, we also know which
  body mode it needs

So the component-expression materialization path should carry a CE body-mode
flag derived from the component type.

High-level flow:

1. parse a `ComponentExpression`
2. identify the component type being constructed
3. determine whether that component uses `props_only`
4. evaluate its body with that mode available in the CE builder / eval context
5. interpret `Statement::Reassign` according to that mode

## Candidate component classes

### `props_only = true`

These are the obvious first candidates:

- `Data`
- `Style`

Potential later candidates depending on authoring conventions:

- `Text`
- other property-bag / leaf-like ECS components that are not meant to own
  structural child trees in authored MMS

### `props_only = false`

Structural/authored tree nodes should stay on normal body semantics:

- `T`
- `Option`
- `Selection`
- panel/container/layout-related components
- any component expression intended to own meaningful descendant structure

## Desired evaluator behavior

For `props_only` component bodies:

```mms
Data {
    row_name = row_name
    mode_value = mode_value
    interactive = true
}
```

should produce named component properties for all assignments.

For non-`props_only` component bodies:

```mms
T {
    let label = "hello"
    label = "goodbye"
}
```

should continue to behave as normal scoped code rather than implicit named
property assignment.

## Relevant files

### Evaluator / CE execution path

- [src/meow_meow/evaluator.rs](/home/rei/_/cat-engine/src/meow_meow/evaluator.rs)
  - `eval_ce()`
  - `EvalContext`
  - `CeBuilder`
  - `Statement::Reassign` handling inside component-expression evaluation

### CE materialization / spawn path

- [src/meow_meow/component_registry.rs](/home/rei/_/cat-engine/src/meow_meow/component_registry.rs)
  - component-type creation logic
  - likely home for a helper mapping component type -> CE body mode
  - `apply_named_assignment()` remains relevant because `props_only` bodies are
    still expected to land in named component properties

### CE data shape

- [src/meow_meow/object.rs](/home/rei/_/cat-engine/src/meow_meow/object.rs)
  - `MaterializedCE`
  - if the body mode needs to survive beyond immediate evaluation, this is the
    likely structure that would carry it

### Existing authored repro / consumer path

- [assets/components/panels.mms](/home/rei/_/cat-engine/assets/components/panels.mms)
  - current repro shape:
    - `row_name = row_name`
    - `label = label`
    - `mode_value = mode_value`

- [src/engine/ecs/system/editor/context.rs](/home/rei/_/cat-engine/src/engine/ecs/system/editor/context.rs)
  - downstream consumer that depends on the authored settings payload surviving
    as `DataComponent`

- [src/engine/ecs/system/editor/settings_panel.rs](/home/rei/_/cat-engine/src/engine/ecs/system/editor/settings_panel.rs)
  - semantic keys currently read from settings payloads

## Suggested implementation shape

### 1. Add an explicit CE body mode

Possible shape in evaluator state:

```rust
enum ComponentBodyMode {
    Normal,
    PropsOnly,
}
```

Attach it to the CE evaluation path rather than using a global rule.

### 2. Add component-type classification

Create a helper keyed by component type:

```rust
fn component_body_mode(component_type: &str) -> ComponentBodyMode
```

Initial recommended mapping:

- `Data` -> `PropsOnly`
- `Style` -> `PropsOnly`
- everything else -> `Normal`

### 3. Restrict `Statement::Reassign` capture

In CE builder mode:

- if current body mode is `PropsOnly`, `name = expr` pushes into CE named props
- otherwise, fall back to normal reassignment semantics

### 4. Keep the current regression covered

The current evaluator regression test should remain, but it should become
explicitly about `Data` / `PropsOnly` behavior rather than generic CE bodies.

## Acceptance criteria

This task is complete when:

1. `Data { row_name = row_name }` still materializes the named property
2. `Style` property-bag bodies behave consistently under the same model
3. structural component bodies no longer inherit global “assignment means named
   property” behavior
4. evaluator tests cover both `PropsOnly` and normal-body cases explicitly
5. the language rule is encoded in one clear place rather than implied by the
   presence of a CE builder alone

## Open question

Should the body mode live:

- only in evaluator-time context derived from `component_type`, or
- directly on `MaterializedCE` / component AST-derived structures?

Recommendation:

- start with evaluator-time derivation from component type
- only persist it onto `MaterializedCE` if a later phase genuinely needs it

That keeps the first cleanup smaller while preserving the intended semantics.
