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

- MMS lacks a first-class plain-data struct / record surface
- event payload APIs therefore fall back to arrays
- editor/panel model data also falls back to arrays, strings, or ad-hoc host
  encodings
- MMS still has unresolved syntax ambiguity between component constructors and
  authored data construction

This task reframes the issue as:

- MMS needs general plain-data structs/records
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
  - already argues that event payloads should become MMS objects/records
- `docs/draft/panel-model-view-contract.md`
  - explicitly says MMS lacks an authored record/struct/dict literal surface
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

Should MMS plain-data structs use a source form that is intentionally distinct
from component constructors, instead of trying to parse two unrelated concepts
from the same `Type { ... }` surface?

The motivating idea from recent discussion is:

- component constructors remain the current uppercase / engine-facing form
- plain-data structs use `struct` plus lowercase snake_case names
- struct allocation uses the same lowercase snake_case constructor name

Example:

```mms
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

That would make this unambiguous in theory:

- `T { ... }` => component expression
- `Data { ... }` => component expression
- `xr_button_event { ... }` => plain-data struct allocation

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

or equivalent field access syntax once plain-data objects are live.

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

### 1. Are structs nominal types, records, or both?

Possible layers:

- plain anonymous record/map literals
- named `struct` declarations
- named struct allocation

We may want:

- records first as runtime values
- named structs second as type-level sugar / declarations

### 2. What is the exact field access surface?

Desired direction:

```mms
event.control
event.value
item.target_ref
```

This implies:

- runtime object/record allocation
- dot-field read support on plain data values

### 3. Do we want lowercase struct names as a rule?

This note treats lowercase snake_case struct names as the leading candidate
because it separates data construction from component construction.

But that should be an explicit language choice, not an accidental convention.

Questions:

- should named plain-data types be `snake_case`?
- should component constructors remain `UpperCamel` / symbolic component names?
- should the parser enforce the distinction or merely allow it?

### 4. Do we need anonymous record literals too?

For event conversion and panel models, anonymous record literals may be enough:

```mms
let item = {
    label: "Head",
    selected: true,
    target_ref: "@uuid:..."
}
```

But if MMS wants explicit declarations, transpilation, and better type docs,
named `struct` declarations are still useful.

### 5. Should events become records first, before full user-authored structs?

One staged approach:

1. add runtime record/object field access
2. convert host event payloads from arrays to records
3. later add authored `struct` declarations + allocations

That may be lower-risk than implementing the full language feature in one pass.

## Proposed direction for this task

This task should investigate and clarify:

1. the desired MMS plain-data value model
2. whether named structs, anonymous records, or both should exist
3. whether lowercase/snake_case data constructors are the right answer to the
   component-vs-data ambiguity
4. how event payload APIs should be re-expressed once structured data exists

The recommendation to test against is:

- component expressions stay as-is
- records/structs become the general MMS data surface
- event payloads move to structured values
- handler code stops using positional array indexing for named payloads

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
let item = world_panel_item {
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

Choose among:

- anonymous records only
- named structs only
- anonymous records + named structs

Also decide:

- field access syntax
- naming rules
- whether lowercase/snake_case is a language rule for plain-data types

### Step 3. Decide staged implementation order

Likely best order:

1. runtime record/object field access
2. host event payload conversion to records
3. authored record literals and/or named struct allocation
4. optional type-checker integration

### Step 4. Update event API plans

Revise event-payload tasks so they target:

- named record payloads
- not positional arrays

XR should be treated as the first concrete migration target, not the only one.

## Exit criteria

This task is complete when:

- the repo has a clear decision on MMS plain-data structs/records
- the component-vs-data construction ambiguity is resolved at the design level
- the event payload API direction is rewritten around structured values
- existing struct/event drafts are either aligned with that decision or clearly
  superseded

## Related

- `docs/task/mms-event-payloads-and-runtime-attach.md`
- `docs/draft/mms-struct-syntax.md`
- `docs/meow_meow/draft/type-system.md`
- `docs/draft/panel-model-view-contract.md`
