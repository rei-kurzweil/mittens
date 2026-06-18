# Armature Visualization Toggle

Date: 2026-06-18

## Context

We need a way to toggle GLTF bone markers/locators in the editor without disrupting the authored
glTF runtime subtree.

The older framing of this problem leaned too heavily on helper-tree topology. The more accurate
requirement is:

- bone viz nodes are tracked as runtime helper state per `GLTFComponent`
- the editor owns a `show_bones` setting per editor scope
- toggling that setting emits a per-GLTF runtime intent:
  `GLTF_TOGGLE_BONE_VIZ`

That lets the runtime remove and re-add bone viz cleanly without treating the imported glTF content
itself as disposable.

## Goal

Design and implement a smooth runtime toggle for editor bone visualization with these semantics:

- editor-scoped setting
- GLTF-scoped intent emission
- tracked helper-node attach/detach
- no glTF subtree rebuild as part of the toggle

## Required behavior

When the editor settings control changes:

1. the owning editor flips `show_bones`
2. the UI updates its visible checkbox/checkmark/toggle state
3. the runtime enumerates `GLTFComponent`s under that editor
4. the runtime emits `GLTF_TOGGLE_BONE_VIZ` for each GLTF in that editor scope

When a GLTF receives `GLTF_TOGGLE_BONE_VIZ`:

- `enabled = true` attaches or rebuilds tracked bone viz helper nodes for that GLTF
- `enabled = false` removes the tracked bone viz helper nodes for that GLTF

## Constraints

### Runtime safety

- toggling bone viz must not rebuild the authored glTF subtree
- toggling bone viz must not reset unrelated runtime state such as animation wiring, IK wiring, or
  selection state on the actual imported content

### Ownership clarity

- the engine must know which helper nodes belong to which GLTF
- repeated toggle cycles must remain clean and idempotent

### Scope isolation

- one editor can show bones while another hides them
- the toggle should affect only GLTFs in the editor whose setting changed

### Performance

- no per-frame whole-world scanning just to maintain the toggle
- attach/detach should be event-driven

## Recommended design

Use the model described in
`docs/task/gltf-bone-viz-tracking-and-toggle.md`:

- maintain runtime bookkeeping for bone viz nodes per `GLTFComponent`
- use `GLTF_TOGGLE_BONE_VIZ` as the attach/detach trigger
- treat bone viz as helper state, not authored glTF content

This leaves topology flexible:

- viz nodes may remain parented under real bones if that is the simplest runtime behavior
- a separate helper tree is optional, not the core requirement

## Not recommended as the primary model

These ideas may still be useful implementation details, but they should not be the main contract:

- global hide/show flags disconnected from editor scope
- rebuilding every glTF tree when the setting changes
- relying on `with_visualized_transforms` as the editor toggle mechanism
- treating "separate viz tree" as the goal rather than tracked toggleable helper state

## Task breakdown

- [ ] identify the existing GLTF bone-viz spawn path
- [ ] make bone-viz helper ownership explicit per `GLTFComponent`
- [ ] add runtime `GLTF_TOGGLE_BONE_VIZ` handling
- [ ] wire editor settings state so `show_bones` fans out to per-GLTF toggle intents
- [ ] ensure toggle-off removes only tracked helper nodes
- [ ] ensure toggle-on recreates only the needed helper nodes
- [ ] confirm authored glTF content is not recreated during the toggle path

## Acceptance criteria

- [ ] the editor exposes a `show bones` control
- [ ] toggling it emits `GLTF_TOGGLE_BONE_VIZ` for each GLTF in that editor scope
- [ ] toggling off removes only tracked bone viz helper nodes
- [ ] toggling on recreates only tracked bone viz helper nodes
- [ ] the authored glTF subtree remains intact
- [ ] multiple editors can control bone viz independently

## Related

- [docs/task/gltf-bone-viz-tracking-and-toggle.md](./gltf-bone-viz-tracking-and-toggle.md)
- [docs/task/editor-settings-panel.md](./editor-settings-panel.md)
