# Data Renderer System For Editor UI

## Status

Proposal / architecture task.

No implementation yet.

## Motivation

The current stopgap editor UI path mixes:

- reducer-owned logical state
- panel model building
- MMS materialization
- subtree attach/remove logic
- rerender decisions
- local interaction state

That makes the editor UI hard to reason about and hard to evolve incrementally.

The current inspector detail experiments suggest the problem is not only the layout math. A large part of the difficulty is how live MMS-rendered subtrees are being built, attached, replaced, and updated from the stopgap adapter.

We need a cleaner system boundary for:

- taking structured UI data
- choosing how that data should be rendered
- projecting it into live ECS component trees
- eventually updating that projection incrementally

## Core idea

Introduce a dedicated `data_renderer_system` for editor UI.

Its job is:

- accept a list of items or one detail item
- map that data into an authored MMS view or a Rust render function
- attach the resulting live component tree into a target root or slot
- own the rerender/update policy for that rendered subtree

This separates:

- reducer logic and workspace state
- model/data preparation
- view projection into live components

## What this system is for

The first intended uses are:

- world panel content rows
- inspector sidebar rows
- inspector detail view
- asset panel lists
- paint/tool selection lists

The important point is that these should stop being ad hoc one-off subtree builders inside the stopgap adapter.

## What this system is not

This should not begin as:

- a generic framework for all runtime UI in the engine
- a full virtual DOM
- a diff engine from day one
- a replacement for reducer/state logic

The first version should stay small and editor-UI-focused.

## Proposed boundary

The system should own:

- the mapping from data payloads to rendered subtree instances
- the target slot/root where the subtree is attached
- the rerender policy for that subtree
- later, keyed diff/patch behavior

The system should not own:

- editor workspace reducer logic
- cross-panel coordination rules
- semantic selection decisions
- scene mutation logic

Those belong in reducer/state/effect layers above it.

## First useful abstraction

There are two concrete rendering cases we need immediately.

### 1. List rendering

Input:

- a list of items
- a target root/slot
- a renderer spec

Output:

- a live rendered subtree representing the list content

Examples:

- world panel rows
- inspector sidebar rows
- asset panel item lists

### 2. Detail rendering

Input:

- one selected item or detail payload
- a target root/slot
- a renderer spec

Output:

- a live rendered subtree representing the detail view

Examples:

- inspector details
- later, asset/property detail views

## Renderer spec

The system should support both MMS-driven and Rust-driven renderers.

Suggested shape:

```rust
enum RendererSpec {
    Mms {
        asset_path: &'static str,
        export_name: &'static str,
    },
    Rust {
        render_fn: fn(&mut World, &mut dyn SignalEmitter, &RenderPayload) -> Result<ComponentId, String>,
    },
}
```

The important architectural decision is:

- do not assume everything must be authored in MMS
- do not force Rust-only rendering either
- keep the projection boundary flexible

## Payload model

The renderer system should not expose raw `DataComponent` shape as its main contract.

`DataComponent` may be part of the implementation or transport path, but the system should have an explicit renderer-facing payload model.

Suggested direction:

```rust
struct UiItem {
    key: String,
    kind: String,
    label: String,
    selected: bool,
    target_ref: Option<String>,
    meta: UiItemMeta,
}

struct UiDetailItem {
    key: String,
    view_kind: String,
    fields: Vec<UiField>,
}
```

or, if the first version should stay simpler:

```rust
struct DetailViewSpec {
    export_name: String,
    args: Vec<Value>,
}
```

The critical requirement is that the payload shape be explicit and not be implicit in ad hoc subtree construction code.

## On item metadata and interactivity

This system needs enough item metadata to make rendered UI useful and interactive.

Examples:

- stable key
- display label
- selected state
- target ref or payload ref
- row kind
- expanded/collapsed flags
- detail-view constructor args

This is why a dedicated item payload model is more useful than raw strings or only using `DataComponent` directly.

## Stable keys

Stable keys should be required from the first version.

Why:

- rerender correctness
- future keyed patching
- query/debug naming
- preserving local state later

Even if phase 1 uses full rerender only, keys should exist now so later incremental patching has a stable identity basis.

## Recommended first rendering policy

Start with full rerender.

For phase 1:

- if the renderer input model changed, delete the previous rendered subtree and rebuild it
- do not try to patch row-by-row or field-by-field yet

This is the simplest trustworthy baseline.

The point of the first version is not to be maximally efficient. It is to establish a clean data-to-view projection boundary.

## Local state and non-rerendering interactions

Some interactions should not go through a full renderer rebuild once the system is in place.

Examples:

- hover state
- local selection highlight driven by `SelectionComponent`
- text caret movement
- text input draft editing
- scroll offset updates

Those should remain:

- component-local state
- panel-local state
- or reducer-owned local state

The renderer system should rerender only when its input payload/model changes.

## How this helps remove the stopgap adapter

Today the stopgap adapter does too much:

- build list rows
- spawn detail tree
- remove previous subtree
- choose when to rebuild
- manually attach children into slots

With a `data_renderer_system`, that becomes:

- reducer/effect layer decides that world panel rows changed
- reducer/effect layer sends a new item list to the renderer system
- renderer system owns projection into the target slot

That shrinks the stopgap adapter and gives a path to delete it.

## Relationship to existing docs

This direction should be aligned with:

- [docs/draft/nested-reducers-for-panels.md](/home/rei/_/cat-engine/docs/draft/nested-reducers-for-panels.md:1)
- [docs/draft/panel-model-view-contract.md](/home/rei/_/cat-engine/docs/draft/panel-model-view-contract.md:1)
- [docs/draft/inspector-panel-multi-instance-and-v2.md](/home/rei/_/cat-engine/docs/draft/inspector-panel-multi-instance-and-v2.md:1)
- [docs/task/editor-ui-rerender-audit-and-clean-reducer-boundary.md](/home/rei/_/cat-engine/docs/task/editor-ui-rerender-audit-and-clean-reducer-boundary.md:1)

Nested reducers define state ownership.

The data renderer system defines how reducer/view-model output becomes live UI.

## Proposed phases

### Phase 1: define the boundary

Goals:

- define `RendererSpec`
- define initial `UiItem` / `UiDetailItem` or equivalent payload shapes
- define target-slot/root ownership
- define full-rerender semantics

Deliverables:

- task/spec for the API
- one small implementation path for editor UI only

### Phase 2: implement list rendering

Goals:

- render a list payload into a target slot
- support MMS-backed list/content rendering
- own subtree replacement in one place

First candidates:

- world panel content rows
- inspector sidebar rows

Deliverables:

- list renderer path replacing at least one current ad hoc subtree builder

### Phase 3: implement detail rendering

Goals:

- render a single selected item/detail payload into a target slot
- support MMS-backed detail views
- replace the current inspector detail subtree spawn logic

First candidate:

- `inspector_details(name, id, guid)`

Deliverables:

- detail renderer path used by inspector panel

### Phase 4: integrate with reducer/effect flow

Goals:

- move rerender decisions out of stopgap subtree code
- make reducer/effect layers decide when renderer input changed
- keep renderer system focused on projection only

Deliverables:

- explicit “state changed -> renderer input changed -> rerender” flow for at least one panel

### Phase 5: support hybrid renderers

Goals:

- support Rust-rendered views alongside MMS-rendered views
- allow the same rendering system to dispatch by renderer spec

Deliverables:

- one MMS-backed case
- one Rust-backed case
- shared projection ownership

### Phase 6: keyed incremental patching

Goals:

- preserve stable items by key
- patch only changed rows/items/fields where useful
- avoid full subtree rebuild when model deltas are small

Important:

- this phase should only happen after the boundary and full-rerender path are working cleanly

Deliverables:

- keyed diff/update path for at least one list renderer

### Phase 7: retire stopgap panel-specific subtree builders

Goals:

- remove ad hoc row/detail subtree construction from the stopgap adapter
- move panel projection responsibilities into the renderer system

Deliverables:

- smaller stopgap adapter
- clearer path toward deleting it entirely

## Suggested first implementation target

The recommended first target is the inspector panel.

Why:

- it has both a list view and a detail view
- it has cross-panel selection-driven rerender triggers
- it has local sidebar selection state
- it currently shows the cost of mixed responsibilities clearly

So the likely first sequence is:

1. inspector sidebar list rendering
2. inspector detail rendering
3. then world panel row rendering if needed

## Open questions

- What exact payload shape should the first version expose?
- Should the first version pass payload data into MMS via positional args only, or do we want an early structured-record bridge?
- Should the renderer system own selection/highlight helper components, or should those remain panel-specific?
- Should renderer targets always be explicit slots, or can the system also own top-level panel roots?
- How much local UI state should be preserved across keyed patching later?

## Practical next step

Write a narrow v1 API note for the first editor-only renderer path:

- one list renderer input shape
- one detail renderer input shape
- one MMS-backed renderer spec
- one target-slot attachment model
- full rerender only

Then replace one existing inspector subtree path with it before generalizing further.
