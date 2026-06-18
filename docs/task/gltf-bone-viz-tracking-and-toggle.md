# GLTF Bone Viz Tracking and Editor Toggle

Date: 2026-06-18

## Context

The old framing here as "separate armature bone viz tree" is no longer the right task shape.

What we actually need is:

- bone visualization nodes for a spawned `GLTFComponent` to be tracked explicitly at runtime
- those nodes to be removable and re-addable without rebuilding the authored glTF subtree
- an editor-scoped toggle that emits a per-GLTF intent when the user changes the setting in the
  editor settings panel

The important idea is not "the viz must live in a separate tree" as an end in itself.

The important idea is: editor-only bone viz is helper runtime state, and the engine should know
which helper nodes belong to which glTF so it can tear them down and rebuild them cleanly.

## Goal

Introduce a per-GLTF runtime intent:

`GLTF_TOGGLE_BONE_VIZ`

This intent is emitted on the scope of each relevant `GLTFComponent` when the editor settings panel
toggles the bone-visibility setting for that editor.

The runtime should then:

- discover or reuse the GLTF's bone list
- remove existing bone viz helper nodes for that GLTF when toggled off
- recreate them when toggled on
- leave the authored/imported glTF subtree intact

## Non-goal

This task is not primarily about committing to a permanent "separate helper tree" architecture.

If some viz nodes remain parented under real bones because that is the simplest way to inherit
transforms and preserve selection/gizmo routing semantics, that is fine.

The architectural requirement is tracking and toggleability, not topological purity.

## Current problem

Today bone visualization is too entangled with glTF spawn behavior.

That causes three problems:

1. the editor cannot reliably toggle bone visualization on and off at runtime
2. removing viz often implies rebuilding or mutating more of the glTF subtree than intended
3. editor-owned helper state is not modeled explicitly enough to manage per-editor visibility

This is especially awkward now that the desired UX is editor-scoped:

- editor A may want bones shown
- editor B may want them hidden
- the setting should fan out as explicit runtime intents to the GLTFs under that editor

## Proposed runtime contract

Add a dedicated intent/signal:

```rust
// naming sketch; exact enum/constant placement can follow existing signal conventions
GLTF_TOGGLE_BONE_VIZ {
    gltf_component_id: ComponentId,
    enabled: bool,
}
```

The exact Rust representation can be an `IntentValue` variant, a named signal constant, or both.
The important contract is semantic:

- this is a GLTF-scoped toggle
- it targets already-spawned runtime state
- it does not mean "respawn the glTF"

## GLTF-owned bone viz tracking

Each spawned `GLTFComponent` should have enough runtime bookkeeping to answer:

- which components are the active bone viz nodes for this GLTF
- whether viz is currently attached
- optionally, which bone transforms are the stable source targets for rebuilding viz

Possible shapes:

```rust
pub struct GltfBoneVizState {
    pub active_viz_roots: Vec<ComponentId>,
    pub bone_targets: Vec<ComponentId>,
    pub enabled: bool,
}
```

This state may live:

- inside `GLTFSystem`
- inside a small dedicated `BoneVisualizationSystem`
- or in another runtime-owned map keyed by `gltf_component_id`

The exact owner matters less than the invariant:

- given a GLTF component id, the engine can remove all of that GLTF's current bone viz nodes
- given a GLTF component id, the engine can rebuild them without rediscovering the whole world

## Toggle semantics

### Toggle on

When `GLTF_TOGGLE_BONE_VIZ(enabled = true)` is received for a spawned GLTF:

1. if viz is already active, no-op
2. resolve the bone transform targets for that GLTF
3. spawn the helper viz nodes for those targets
4. record the spawned helper node ids in the GLTF-owned tracking state
5. mark viz enabled

### Toggle off

When `GLTF_TOGGLE_BONE_VIZ(enabled = false)` is received:

1. if viz is already inactive, no-op
2. remove the tracked helper nodes using the normal subtree-removal path
3. clear the tracked helper node ids
4. mark viz disabled

## Editor integration

The editor settings panel should own a `show bones` toggle for each editor scope.

When the checkbox / checkmark / toggle is changed:

1. editor context state flips `show_bones`
2. the settings UI refreshes its visible indicator
3. the editor system enumerates GLTFs under that editor
4. for each GLTF in that editor scope, emit `GLTF_TOGGLE_BONE_VIZ`

That fan-out is the core behavior change.

The toggle is not a global renderer flag and not a single world-wide armature switch.

It is:

- editor scoped at the UI/state level
- GLTF scoped at the runtime intent level

## Scope rules

The emitted toggle should only affect GLTFs that belong to the editor whose settings changed.

That means the implementation should:

- find the active editor or the editor that owns the settings interaction
- enumerate descendant `GLTFComponent`s under that editor subtree
- emit one `GLTF_TOGGLE_BONE_VIZ` per GLTF

This preserves editor isolation and avoids hidden cross-editor coupling.

## Relationship to `with_visualized_transforms`

`GLTFComponent.with_visualized_transforms` is now ambiguous.

It currently sounds like:

- a spawn-time instruction
- and maybe a persisted authored property
- and maybe an editor-view preference

Those should be separated conceptually.

For this task:

- do not rely on `with_visualized_transforms` as the main runtime toggle mechanism
- prefer the explicit runtime signal/intent for attach/detach behavior

If the field remains, it should be treated carefully:

- either as a legacy spawn hint
- or as persisted default state for a specific GLTF

But the editor checkbox should drive `GLTF_TOGGLE_BONE_VIZ`, not depend on respawn-time behavior.

## Topology guidance

We do not need to over-constrain the exact helper topology in this task.

Acceptable:

- viz nodes parented under the real bones they visualize
- viz nodes in a helper subtree, if routing/follow behavior is already solved cleanly

Not acceptable:

- any design that requires destroying and recreating the authored glTF subtree just to hide bones
- any design that makes it unclear which helper nodes belong to which GLTF

## Suggested implementation stages

### Stage 1: runtime tracking

- identify the current bone-viz spawn path
- stop treating bone viz as anonymous incidental children
- record the helper nodes created for each GLTF

### Stage 2: toggle intent

- add `GLTF_TOGGLE_BONE_VIZ` to the runtime signal/intent layer
- route it to the runtime owner that manages bone viz for a GLTF
- implement clean attach/detach semantics

### Stage 3: editor settings hookup

- add or finalize `show_bones` in editor context/settings state
- wire the settings row toggle interaction
- emit `GLTF_TOGGLE_BONE_VIZ` for each GLTF under the affected editor

### Stage 4: cleanup and naming

- rename any code/docs that still imply this task is mainly about "separating the tree"
- tighten terminology around:
  - editor-scoped setting
  - GLTF-scoped toggle
  - tracked bone viz helper nodes

## Acceptance criteria

- [ ] Toggling the editor settings panel's bone-visibility control updates the editor-local state
- [ ] That toggle emits `GLTF_TOGGLE_BONE_VIZ` once for each GLTF in the affected editor scope
- [ ] Toggling off removes only tracked bone viz helper nodes
- [ ] Toggling on recreates only the needed bone viz helper nodes
- [ ] The authored/imported glTF subtree is not rebuilt as part of the toggle
- [ ] Runtime ownership of bone viz nodes is explicit enough to support repeated remove/re-add cycles
- [ ] Multiple editors can toggle their own GLTF bone viz independently

## Related docs

- `docs/task/editor-settings-panel.md`
- `docs/task/armature-visualization-toggle.md`
- `docs/task/serialize-component-and-armature-viz-save-plan.md`
- `docs/task/transform-parent-component-ref-routing.md`
