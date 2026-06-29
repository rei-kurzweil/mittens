# Task: MMS Structs For Event Payloads And Plain Data

Date: 2026-06-29

Status: active design task

## Why this task exists

The immediate symptom is ugly XR handler payloads in MMS:

```mms
on(xr_gamepad, "XrButtonDown", fn(event) {
    let hand = event[0]
    let control = event[1]
    let value = event[2]
})
```

Today that `event` argument is not a named data shape. It is a positional
`Value::Array` assembled by the Rust host bridge in
`src/meow_meow/runner.rs`.

That is functional, but it is the wrong long-term surface.

The real problem is broader than XR:

- MMS lacks a first-class plain-data table / struct surface
- event payload APIs therefore fall back to arrays
- editor/panel model data also falls back to arrays, strings, or ad-hoc host
  encodings
- MMS still has unresolved syntax ambiguity between component constructors and
  authored data construction

This task reframes the issue as:

- MMS needs general plain-data tables, with named structs layered on top
- event payloads should become one consumer of that feature, not a bespoke
  exception

## Current evidence

### 1. Event payloads are positional arrays today

In `src/meow_meow/runner.rs`, XR host events are bridged into MMS as:

- button events:
  - `[hand, control, value]`
- axis events:
  - `[hand, control, [x, y]]`

That means authored MMS has to know index positions instead of field names.

### 2. Existing task/doc work already points toward structured data

Relevant existing docs:

- `docs/task/mms-event-payloads-and-runtime-attach.md`
  - already argues that event payloads should become MMS objects/tables
- `docs/draft/panel-model-view-contract.md`
  - explicitly says MMS lacks an authored table/struct/dict literal surface
- `docs/meow_meow/draft/type-system.md`
  - already reserves a "Structs" concept in the type system draft
- `docs/draft/mms-struct-syntax.md`
  - proposes `struct AppState { ... }` and `AppState { ... }`

### 3. The existing struct draft still carries a syntax collision

The current draft in `docs/draft/mms-struct-syntax.md` uses Rust-style
UpperCamelCase struct names plus brace allocation:

```mms
struct AppState {
    blinking_light_blocked: bool
}

let app_state = AppState {
    blinking_light_blocked: false
}
```

That collides with the existing MMS component expression surface:

```mms
T { ... }
Data { ... }
Style { ... }
```

The current draft suggests resolving this by peeking inside the brace body and
disambiguating based on colon-style fields.

That is possible, but it may not be the cleanest language direction.

## Main question

Should MMS plain-data values be framed as generic tables first, with named
structs as an additional typed/authored layer, instead of treating event
payloads as a bespoke special-case data system?

The motivating idea from recent discussion is:

- anonymous table literals should be the first parser/AST/runtime target
- component constructors remain the current uppercase / engine-facing form
- named structs use `struct` plus lowercase snake_case names
- named struct allocation uses the same lowercase snake_case constructor name

Example:

```mms
let event = {
    hand = "Right"
    control = "ButtonB"
    value = 1.0
}

struct xr_button_event {
    hand: Str
    control: Str
    value: Double
}

let event = xr_button_event {
    hand: "Right"
    control: "ButtonB"
    value: 1.0
}
```

That would make the language split clearer:

- `T { ... }` => component expression
- `Data { ... }` => component expression
- `{ ... }` => anonymous table literal
- `xr_button_event { ... }` => named struct allocation

## Why this direction is attractive

### A. It solves more than XR

This would help:

- XR event payloads
- editor selection / click / drag event payloads
- panel item models
- general function return values that want named fields
- future MMS-to-Rust transpilation

### B. It makes handler code readable

Instead of:

```mms
let control = event[1]
let value = event[2]
```

we want:

```mms
let control = event.control
let value = event.value
```

or equivalent field access syntax once plain-data tables are live.

### C. It avoids parser cleverness where possible

Using the same `UpperType { ... }` syntax for both:

- component construction
- plain-data struct allocation

means the parser must infer intent from local body shape.

Using a distinct naming convention for authored data makes the source language
clearer and reduces parser ambiguity.

### D. It matches existing MMS split-brain reality

MMS already mixes:

- engine/component authoring
- general scripting/evaluation

Those are not the same kind of thing.

It is reasonable for the language to reflect that distinction at the syntax
level.

## Open design questions

### 1. Are structs nominal types, tables, or both?

Possible layers:

- plain anonymous table literals
- named `struct` declarations
- named struct allocation

We want:

- tables as the runtime value model
- anonymous tables first
- named structs on top of tables as declaration/type sugar

### 2. What is the exact field access surface?

Desired direction:

```mms
event.control
event.value
item.target_ref
```

This implies:

- runtime table allocation
- dot-field read support on plain data values

### 3. Do we want lowercase struct names as a rule?

This note treats lowercase snake_case struct names as the leading candidate
because it separates data construction from component construction.

But that should be an explicit language choice, not an accidental convention.

Questions:

- should named plain-data types be `snake_case`?
- should component constructors remain `UpperCamel` / symbolic component names?
- should the parser enforce the distinction or merely allow it?

### 4. Do we need anonymous table literals too?

Yes. They should be the first thing implemented in tokenizer/parser/AST so the
table model is testable before named structs or optional typing land.

For event conversion and panel models, anonymous table literals may be enough:

```mms
let item = {
    label = "Head"
    selected = true
    target_ref = "@uuid:..."
}
```

An anonymous table is effectively a generic key/value table. Structs can later
be treated as typed tables.

Named `struct` declarations are still useful for:

- reusable model shapes
- clearer docs
- eventual transpilation / static analysis

### 5. Should events become tables first, before full user-authored structs?

Yes. The right first move is a generic table model for events.

One staged approach:

1. add anonymous table literals to tokenizer / parser / AST
2. add runtime table allocation and field access
3. convert host event payloads from arrays to generic tables
4. add named `struct` declarations + allocations
5. later add optional typing to functions and wider type-checker integration

That may be lower-risk than implementing the full language feature in one pass.

## Proposed direction for this task

This task should investigate and clarify:

1. the desired MMS plain-data value model
2. how anonymous tables and named structs fit together
3. whether lowercase/snake_case data constructors are the right answer to the
   component-vs-data ambiguity
4. how event payload APIs should be re-expressed once structured data exists
5. when optional type annotations on functions should enter the plan

The recommendation to test against is:

- component expressions stay as-is
- generic tables become the initial MMS plain-data runtime surface
- anonymous tables and named structs both exist in the final design
- structs are typed/authored tables, not a separate runtime universe
- event payloads move to generic structured table values first
- handler code stops using positional array indexing for named payloads
- optional function typing is deferred until after the core table/event model
  is working, likely as a later phase rather than phase 1

## Concrete target examples

### XR button event

Desired authored shape:

```mms
on(xr_gamepad, "XrButtonDown", fn(event) {
    if event.control == "ButtonA" {
        status.set_text(event.hand + " A down")
    }
})
```

### XR axis event

Desired authored shape:

```mms
on(xr_gamepad, "XrAxisChanged", fn(event) {
    if event.control == "LeftStick" {
        left_dot.set_position(event.value[0], event.value[1], 0.0)
    }
})
```

### Panel item model

Desired authored shape:

```mms
let item = {
    key = "head"
    label = "Head"
    selected = true
    target_ref = "@uuid:..."
}

let typed_item = world_panel_item {
    key: "head"
    label: "Head"
    selected: true
    target_ref: "@uuid:..."
}
```

## Suggested work breakdown

### Step 1. Audit and align existing docs

- compare `docs/draft/mms-struct-syntax.md`
- compare `docs/task/mms-event-payloads-and-runtime-attach.md`
- compare `docs/draft/panel-model-view-contract.md`
- decide whether the struct draft should be revised away from UpperCamel
  allocation syntax

### Step 2. Decide the plain-data surface

Lock in:

- anonymous tables as the base runtime model
- named structs as an additional authored/type surface

Also decide:

- field access syntax
- naming rules
- whether lowercase/snake_case is a language rule for plain-data types

### Step 3. Decide staged implementation order

Likely best order:

1. tokenizer/parser/AST support for anonymous table literals
2. runtime table allocation + field access
3. host event payload conversion to generic tables
4. named struct declarations / allocation
5. optional function typing and wider type-checker integration

### Step 4. Update event API plans

Revise event-payload tasks so they target:

- named table payloads
- not positional arrays

XR should be treated as the first concrete migration target, not the only one.

## Exit criteria

This task is complete when:

- the repo has a clear decision on MMS plain-data tables/structs
- the component-vs-data construction ambiguity is resolved at the design level
- the event payload API direction is rewritten around structured values
- existing struct/event drafts are either aligned with that decision or clearly
  superseded

## Related

- `docs/task/mms-event-payloads-and-runtime-attach.md`
- `docs/draft/mms-struct-syntax.md`
- `docs/meow_meow/draft/type-system.md`
- `docs/draft/panel-model-view-contract.md`
