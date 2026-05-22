# Editor Panels Reimplementation Draft

## Current State

The Rust editor-panel runtime has been intentionally removed.

- `InspectorSystem::setup_panels_for_editor(...)` is now a no-op.
- There is no live world panel or inspector panel subtree.
- There are no panel-specific Rust component types.

This is deliberate. The previous implementation mixed too many concerns:

- panel layout ownership
- panel rendering ownership
- row identity
- click behavior
- editor selection/gizmo integration

That made it hard to simplify or replace incrementally.

## Goal

Bring panels back as an MMS-first system with a much thinner Rust host layer.

Desired ownership split:

- MMS owns panel structure and visual composition.
- Rust owns editor state, scene queries, selection, gizmo control, and other engine actions.

## Non-Goals For The Next Pass

- Do not revive the old Rust-authored row tree.
- Do not reintroduce ad hoc panel-specific components just to hold UI bookkeeping.
- Do not make scroll, hover, or pointer motion trigger panel rerenders.

## Proposed Runtime Shape

### 1. Rust builds a panel view model

Rust should compute a plain data snapshot for each panel.

For the world panel, each item likely needs:

- stable item key
- display label
- depth
- selected flag
- action payload owned by Rust

For the inspector panel, each item likely needs:

- stable item key
- display label
- optional section kind
- optional action payload owned by Rust

The action payload should stay in Rust, not in MMS.

### 2. MMS renders the entire panel from that view model

The panel factory should produce the full subtree, including all rows.

Important requirement:

- each interactive row needs a deterministic node name or selector derived from the stable item key

That gives Rust a reliable way to query the rendered nodes after the tree is spawned.

### 3. Rust binds host-side behavior after render

After spawning the MMS panel subtree, Rust should:

1. query the rendered row nodes
2. map those row nodes back to item keys
3. register scoped click handlers in `RxWorld`

Those handlers should close over Rust-owned action payloads.

Examples:

- world row click selects a `ComponentId`
- inspector row click switches a section or focuses a property target
- toolbar/button click changes editor mode or gizmo state

This keeps MMS out of the editor API surface while still letting MMS own structure.

## Rerender Semantics

Panels should rerender only when their source view model changes.

Good rerender triggers:

- selection changed
- scene topology changed in a way that affects the world panel
- inspector target changed
- editor mode changed if that changes visible panel content

Bad rerender triggers:

- scroll offset changed
- pointer moved
- hover changed
- drag moved unless the visible item data itself changed

## Suggested Implementation Steps

### Phase 1: data contract

Define a Rust-side panel view model type and a stable item-key scheme.

Keep it small and explicit.

### Phase 2: rendered node identity

Update MMS panel assets so every interactive item gets a predictable node name derived from its key.

Example direction:

- item key: `node_42`
- row node name: `item_node_42`

### Phase 3: post-render binding

Add a Rust-side binding pass:

1. render the MMS subtree
2. query named row nodes
3. install click handlers in `RxWorld`

### Phase 4: lifecycle

Track only the minimum runtime state needed to replace the old subtree and refresh handler bindings.

Prefer one small Rust record per live panel instance over many panel-specific UI components.

## Open Questions

### Where should panel instance state live?

Options:

- keep it in `InspectorSystem`
- keep it in a small editor-owned runtime map keyed by `editor_root`
- store one small opaque host record component on the editor root

The current preference is to avoid introducing more panel-specific ECS components unless the lifecycle demands it.

### How should MMS receive item data?

Options:

- richer object-like values if the language surface is ready
- generated script source with parallel arrays or explicit row calls
- a host helper that builds the panel subtree from a more structured value

The simplest acceptable choice is the one that preserves stable item keys without growing the language in a rushed way.

### How should topology changes trigger rerender?

Likely direction:

- Rust listens for the existing editor-relevant signals
- Rust marks the relevant panel instance dirty
- the panel refresh happens in one controlled rebuild point

## Status

Panels are intentionally absent from the runtime until this redesign is implemented.

That is preferable to carrying forward a partially removed panel system with unclear ownership.