# Task: vtuber mirror selection triage and stopgap adapter unwind

Date: 2026-06-20

Status: triage / investigation

## Goal

Track two connected editor issues:

1. `examples/vtuber-mirror-example.mms` scene content inside `ED {}` does not appear to select
   reliably from the editor settings panel's `Select` mode.
2. The current stopgap MMS adapter still owns too much editor workspace behavior, making it hard
   to reason about selection, focus, and panel updates as one shared event system.

This note is for bug triage and decomposition planning. It is not implementation approval yet.

## Files inspected

- [examples/vtuber-mirror-example.mms](/home/rei/_/cat-engine/examples/vtuber-mirror-example.mms:1)
- [src/engine/ecs/system/editor_system.rs](/home/rei/_/cat-engine/src/engine/ecs/system/editor_system.rs:1)
- [src/engine/ecs/system/editor/context.rs](/home/rei/_/cat-engine/src/engine/ecs/system/editor/context.rs:1)
- [src/engine/ecs/system/editor_scene_hit.rs](/home/rei/_/cat-engine/src/engine/ecs/system/editor_scene_hit.rs:1)
- [src/engine/ecs/system/selection_system.rs](/home/rei/_/cat-engine/src/engine/ecs/system/selection_system.rs:1)
- [src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs](/home/rei/_/cat-engine/src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs:1)
- [docs/task/editor-stopgap-adapter-decomposition.md](/home/rei/_/cat-engine/docs/task/editor-stopgap-adapter-decomposition.md:1)

## Current observed architecture

There are currently two selection/event lanes:

1. Scene/editor-object selection
   - `EditorSystem` listens for `DragStart` on each `editor_root`.
   - It resolves a scene hit with `resolve_editor_scene_hit(...)`.
   - It calls `select_editor_target(...)`.
   - That emits `SelectionChanged` directly on the `editor_root`.

2. Shared editor panel selection/focus
   - `SelectionSystem` listens globally for `Click`.
   - It resolves `Selection` / `Option` scopes for panel UI.
   - `EditorContextSystem` listens under the shared panel query root for `SelectionChanged`.
   - It derives context events like panel focus changes, world-panel selection changes, asset
     selection changes, and editor-settings interaction-mode changes.

This means `Select` mode is not implemented by the settings panel itself. The settings panel only
mutates editor context and `EditorComponent.interaction_mode`. Actual scene picking still comes
from `EditorSystem`.

## Important findings

### 1. `ED {}` content is not enough by itself

`vtuber-mirror-example.mms` does place the temple geometry under `ED {}`. That makes the content
editor-owned, but selection still depends on the editor-scene hit path:

- the clicked renderable must resolve to an `editor_root`
- it must not be under `Selectable.off()`
- it must not be part of the transform gizmo
- `EditorSystem` must receive the `DragStart`
- `resolve_editor_scene_hit(...)` must find a transform ancestor worth selecting

So the authored `ED {}` block is necessary, but it is not the whole contract.

### 2. Scene selection and panel selection do not currently share one routing surface

Right now:

- panel UI selection is `Click`-driven and shared-root-driven
- scene object selection is `DragStart`-driven and editor-root-driven

That split makes the runtime harder to reason about, especially when focus and active-editor state
must stay in sync across:

- settings panel interaction mode
- panel focus
- world panel semantic selection
- direct scene clicks

This is very close to the "jank" suspicion: the system behaves as if there are separate trees with
separate owners, even though the user expectation is one editor workspace with one active context.

### 3. `EditorContextSystem` already wants to be the shared source of truth

`EditorContextState` already carries:

- `active_editor`
- `selected_component`
- `focused_panel`
- `interaction_mode`
- asset selection and cursor state

That is the right shape for a shared editor workspace context. The problem is that scene selection
still enters from a separate lane and only partially rejoins the shared state later.

### 4. The stopgap adapter still owns selection semantics that should not live there

`editor_inspector_system_stopgap_mms_adapter.rs` still performs behavior that is broader than
"render some MMS-backed panels":

- world-panel semantic selection application
- editor-context mutation
- `select_editor_target(...)` calls for world-panel clicks
- inspector refresh coordination
- panel spawn/reconcile and shared runtime UI bootstrap

This file is acting as a workspace controller, panel layout manager, selection bridge, and
panel-specific behavior host all at once.

## Likely bug shape for `vtuber-mirror-example`

The first likely issue is not that `Select` mode is absent. It is that active editor and scene-hit
routing are not robustly unified.

Most likely failure modes to verify:

1. `EditorSystem` receives the pointer gesture, but `resolve_editor_scene_hit(...)` resolves to a
   different editor root than the one the settings panel considers active.
2. The relevant temple pieces are editor-owned but do not have the transform ancestry or
   raycastable shape that the scene-hit path expects.
3. Shared panel focus or other handler routing is not wrong by itself, but it obscures which lane
   currently owns the "real" selection.
4. Multiple editor roots or shared runtime UI state make the active-editor fallback look valid in
   panel state while scene-object selection still routes elsewhere.

## Ownership conclusion

We should stop treating panel tree selection and scene tree selection as separate conceptual
systems.

Recommended model:

- panel UI, world panel, assets panel, paint panel, and direct scene hits should all feed one
  editor workspace context reducer
- the reducer should own `active_editor`, `focused_panel`, `selected_component`, and interaction
  mode
- panel-specific systems should observe that shared context rather than privately reconstructing it
- direct scene hit selection should be expressed as a first-class editor-context event, not as a
  special side path that later happens to mutate some overlapping state

This does not require one literal tree in the world topology, but it should behave like one shared
selection/event surface.

### Make "current editor root" irrelevant for selection resolution

Selection should be derived from the clicked target and its ancestry, not gated by whichever editor
root is currently considered active.

That means:

- direct scene hits should resolve `editor_root` from the hit target itself
- panel payloads that refer to scene targets should resolve `editor_root` from the referenced
  target itself
- world-panel and inspector actions should not need an already-correct `active_editor` just to
  decide what object they mean
- `active_editor` should become a reduced consequence of selection/focus, not a prerequisite for
  selection to work

In other words:

- `current editor root` is useful as derived workspace state
- `current editor root` should not be an input dependency for hit-to-selection correctness

This should remove a whole class of bugs where the UI looks like one editor is active while the
selection path silently depends on some different fallback editor root.

## Stopgap adapter decomposition map

To kill the stopgap adapter safely, split responsibilities by function:

### 1. Keep in generic editor workspace infrastructure

- runtime editor UI root creation
- shared panel layout spawn/find
- mounted panel registry
- panel shell discovery
- shared editor workspace context state
- selection/focus event fan-in
- editor-root resolution from semantic targets

Natural homes:

- `src/engine/ecs/system/editor/workspace.rs`
- `src/engine/ecs/system/editor/context.rs`
- generic panel/runtime layout helpers

### 2. Keep in scene/editor interaction systems

- scene hit resolution
- transform target resolution
- gizmo attach/update
- scene selection event emission

Natural homes:

- `src/engine/ecs/system/editor_scene_hit.rs`
- `src/engine/ecs/system/editor_system.rs`

But the emitted output should become an editor-context event contract, not a private side channel.

### 3. Keep panel-specific semantics in panel modules

- world tree modeling and row payload meaning
- inspector workspace reducer and detail models
- grid row actions
- paint tool semantics
- editor settings option semantics

Natural homes:

- `src/engine/ecs/system/editor/world_panel.rs`
- `src/engine/ecs/system/editor/inspector_panel.rs`
- `src/engine/ecs/system/editor/grid_panel.rs`
- `src/engine/ecs/system/editor/paint_panel.rs`
- `src/engine/ecs/system/editor/settings_panel.rs`

### 4. Remove from the stopgap adapter over time

- direct editor-context mutation for selection
- world-panel-to-gizmo bridging
- shared panel handler ownership
- runtime workspace bootstrap ownership
- shared selection synchronization

After that, the remaining adapter surface should be:

- shell/materialization helpers during migration
- slot projection / rerender helpers while panel infra is being extracted

The target end state should be that the stopgap adapter is either deleted, or reduced to a very
thin panel-rendering wrapper with no selection, focus, workspace, or panel-domain behavior in it.

## Recommended next verification steps

1. Add targeted tracing around `EditorSystem` scene-hit selection for `vtuber-mirror-example`.
   - log resolved editor root, hit renderable, target transform, and current `EditorContextState.active_editor`
2. Add a focused regression test for "settings panel in `Select` mode + direct click on `ED {}`
   object updates shared editor context and selected transform".
3. Trace whether the clicked temple pieces resolve to the expected transform ancestor.
4. Audit whether multiple editor roots are registered in this example and whether the fallback
   active editor in `EditorContextSystem` matches the scene-hit editor root.
5. Move any shared selection-state mutation out of
   `editor_inspector_system_stopgap_mms_adapter.rs` before changing selection behavior further.

## Implementation direction after verification

If the tracing confirms the suspected split-brain routing, the first refactor should be:

1. define a shared "editor workspace selection event" contract
2. have direct scene hits emit that contract
3. have panel selections emit the same contract class where appropriate
4. let `EditorContextSystem` reduce all of them
5. make panel systems react to reduced context instead of partially owning selection side effects

That should make `Select` mode behavior easier to debug and also reduce the amount of logic trapped
inside the stopgap adapter.
