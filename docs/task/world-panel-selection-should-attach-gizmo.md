# Task: World Panel Selection Should Attach Gizmo to Transform Targets

Date: 2026-06-10

Status: investigation / behavior-fix plan.

## Problem

Selecting an item from the world panel appears to update editor selection context, but does not
reliably place the transform gizmo onto the selected transform component.

Scene clicks already go through `select_editor_target(...)`, which:

1. resolves/spawns the gizmo
2. attaches it to the selected transform
3. updates `EditorComponent.selected`
4. emits editor `SelectionChanged`

World-panel selection seems to update selection state without going through the same path.

## Observed behavior

- clicking/selecting from the world panel updates panel/editor context state
- inspector/world panel content can follow the selection
- transform gizmo does not necessarily move to the chosen transform

This suggests selection state and gizmo attachment are currently separate codepaths.

## Relevant codepaths

### Scene selection path that does attach gizmo

- `select_editor_target(...)`
  [src/engine/ecs/system/editor_system.rs](../../src/engine/ecs/system/editor_system.rs)

### World panel selection path

- `apply_world_panel_semantic_selection(...)`
- `sync_world_panel_selection(...)`
- panel click and selection handlers in
  [src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs](../../src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs)

Related note:

- [docs/task/post-mittens-world-panel-remake.md](./post-mittens-world-panel-remake.md)

That task already notes that no `select_editor_target(...)` call happens in this path.

## Goal

Make world-panel selection of a transform-equivalent target produce the same gizmo attachment
behavior as scene selection.

## Required behavior

When the user selects a world-panel row whose semantic target resolves to a `TransformComponent`
inside an editor tree:

1. determine the owning `editor_root`
2. resolve the semantic target transform
3. call the same gizmo-selection path used by scene interaction
4. keep editor context / world panel / inspector state consistent

If the semantic target is not a transform, behavior should be explicit:

- either select the nearest transform ancestor
- or intentionally select without gizmo attachment

But that policy must be consistent and documented.

## Questions to answer

1. Does world-panel selection currently resolve to authored target, payload target, or row root?
2. If the selected payload is not itself a transform, what is the intended gizmo target?
3. Should selecting the `EditorComponent` row itself ever attach a gizmo?
4. Should this path update REPL cwd the same way scene selection currently does?

## Likely root cause

The world panel is probably doing one or more of:

- updating only editor context / selection state
- writing `EditorComponent.selected` directly
- emitting selection events without calling `select_editor_target(...)`

That is enough for panel content to refresh, but not enough for gizmo attach/spawn.

## Proposed fix shape

### Option A: reuse `select_editor_target(...)`

Preferred first pass.

When world-panel semantic selection resolves a transform target and owning editor root:

- call `select_editor_target(world, emit, editor_root, target_transform, update_repl_cwd)`

This preserves one canonical gizmo-attachment path.

### Option B: factor out a narrower gizmo-attach helper

If world-panel selection must avoid some side effects of scene selection, split:

- gizmo attach + editor selected update
- selection event emission
- REPL cwd update

Then world panel can call the same shared attach helper without inheriting unrelated behavior.

## Instrumentation

Before changing behavior, log for world-panel selection:

- selected row root
- selected payload
- semantic target
- resolved editor root
- nearest transform chosen for gizmo, if any
- whether `select_editor_target(...)` ran

This will make the mismatch obvious.

## Validation

Repro:

1. select a transformable object by scene click
2. verify gizmo attaches
3. select the same object by world panel row
4. verify gizmo attaches in the same way

Also test:

- selecting non-transform leaf rows
- selecting editor root row
- selecting painted icon / painted asset instances

## Related

- [docs/task/post-mittens-world-panel-remake.md](./post-mittens-world-panel-remake.md)
- [docs/bugs/world-panel-does-not-follow-scene-selection-from-clicked-geometry.md](../bugs/world-panel-does-not-follow-scene-selection-from-clicked-geometry.md)
