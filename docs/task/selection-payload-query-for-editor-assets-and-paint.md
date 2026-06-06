# Task: Selection Payload Queries for Editor Assets and Paint

## Status

Planned.

Companion authoring note:

- [`docs/draft/mms-selection-payload-selector.md`](../draft/mms-selection-payload-selector.md)

## Goal

Teach editor-side `SelectionComponent`s to optionally resolve a semantic payload
from the selected `Option` via a query string, then use that mechanism to verify
what the asset-panel selection should expose to the paint panel / paint system.

The immediate problem is that paint currently derives its brush source from the
selected asset title string. That is fragile. The editor should instead be able
to carry a payload component id resolved relative to the selected asset option.

---

## Motivation

Current asset selection has two distinct objects:

- the selected UI option shell (`asset_item`)
- the meaningful nested object for downstream systems

For painting, the downstream system does not want "the row that was clicked."
It wants the asset-related payload represented by that row, ideally without
re-walking the row topology outside the selection boundary.

The intended direction is:

- `SelectionComponent` may carry an optional payload query string
- the query runs relative to the selected `Option` root
- `SelectionChanged` carries the resolved payload component id
- paint consumes that payload instead of using the display title as its primary
  identity

---

## Proposed API Direction

Conceptually:

```rust
pub struct SelectionComponent {
    pub mode: SelectionMode,
    pub payload_selector: Option<String>,
    ...
}
```

And on emission:

```rust
EventSignal::SelectionChanged {
    selection_root: ComponentId,
    mode: SelectionMode,
    selected_entries: Vec<SelectionEntry>,
    selected_component: Option<ComponentId>,
    selected_payload: Option<ComponentId>,
}
```

Query semantics:

- the query is evaluated from the selected option root
- it should resolve to at most one payload component
- if it resolves to nothing, payload is `None`
- behavior on multiple matches must be decided explicitly

Recommended authoring shape in MMS:

```mms
Selection.payload_selector("[name='option_value']") {
    ...
}
```

or another stable semantic marker rooted under each option.

---

## Editor-Facing Scope

This task is specifically about editor/UI selection scopes first, not all
selection in the engine at once.

Relevant editor selections:

- assets panel selection
- paint tool selection
- panel layout selection
- world panel selection, if useful later

The highest-value first target is assets panel selection because it feeds the
paint workflow.

---

## Main Question to Verify

For a selected asset item, what should the selection payload query return so the
paint system can use it correctly?

There are at least three plausible answers:

1. the preview subtree root under `#preview_slot`
2. an explicit semantic payload marker under the asset option
3. a component/reference node that identifies the asset template to instantiate

These are not equivalent.

### Option 1: preview subtree root

Pros:

- literally corresponds to the visible selected asset preview
- aligns with the intuition "paint what I selected"

Cons:

- preview content may be wrapped/scaled for panel presentation
- some previews may be skipped or degraded
- preview subtree may not be the same as the actual template root desired for
  scene instantiation

### Option 2: explicit payload marker under the option

Pros:

- separates visible preview structure from semantic payload
- stable across UI refactors
- can point at preview, template metadata, or another anchor

Cons:

- requires adding/authoring that marker

### Option 3: asset identity/reference node

Pros:

- best match if paint should instantiate an asset template, not clone the live
  preview subtree
- avoids tying paint semantics to preview presentation

Cons:

- requires a concrete representation for asset identity in the option subtree

At the moment, option 3 is probably the strongest semantic fit for paint, while
option 1 is the most literal UI interpretation. The implementation work should
verify which model the editor actually wants.

---

## Investigation Targets

Inspect and verify the current topology and selection flow across:

- `assets/components/asset_item.mms`
- `assets/components/assets_content.mms`
- `src/engine/ecs/system/asset_system.rs`
- `src/engine/ecs/system/selection_system.rs`
- `src/engine/ecs/system/editor_paint_system.rs`

Questions to answer:

1. What is the selected option root for an asset click today?
2. What nested node under that option represents the preview root today?
3. Is that preview root stable and always present?
4. Is the preview root actually the right payload for paint, or does paint need
   asset identity/template identity instead?
5. Should the asset option author a stable semantic marker specifically for
   selection payload resolution?

---

## Proposed Work Plan

### 1. Extend `SelectionComponent` with an optional payload selector

Add storage for a query string evaluated relative to the selected option root.

Requirements:

- generic, not panel-specific
- optional
- no callbacks required for the first version

### 2. Extend `SelectionChanged` with optional payload output

When selection changes:

- resolve the payload selector from the selected option root
- attach the resolved component id to the event

The payload should be computed before outside systems observe the event.

### 3. Author payload queries for editor selections

At minimum:

- asset panel `Selection`
- possibly paint tool `Selection`

For paint tools, payload may be unnecessary or may just equal the selected
option root. The asset panel is the real driver for this task.

### 4. Verify the asset payload query target

Determine whether the payload query should target:

- preview subtree root
- semantic payload anchor
- asset identity/reference anchor

This must be verified against the current asset item topology, not guessed.

### 5. Update paint to consume payload first

Paint should prefer:

- selected payload / asset identity

over:

- selected title string

The title may remain useful for status text, but should not be the primary
brush key if a payload exists.

---

## Acceptance Criteria

1. `SelectionComponent` can optionally declare a payload query string.
2. `SelectionChanged` can carry a resolved payload component id.
3. Asset-panel selection resolves a payload from the selected option via that
   query.
4. The team has verified exactly what query target should represent the
   paintable selected asset.
5. Paint no longer depends primarily on selected label/title text when a
   payload is available.

---

## Open Decisions

1. What should happen if the payload query matches multiple nodes?

Options:

- first match wins
- no payload, with warning
- hard error in debug builds

2. Should the assets panel payload query target preview content directly?

This depends on whether paint wants:

- "clone what is visually previewed"

or:

- "instantiate the asset template represented by this option"

3. Should different options in one selection scope eventually support different
payload selectors?

If yes, `Selection.payload_selector(...)` may be insufficient long-term and the
engine may later want `Option`-level payload markers/selectors.

---

## Recommended First Verification

Before changing paint semantics broadly, inspect one selected asset item in the
live editor topology and answer:

- what component under `asset_item` is the actual preview root?
- is that node suitable as `selected_payload`?
- if not, what stable marker/reference should be added instead?

That verification should drive the final authored query string for the asset
panel `Selection`.
