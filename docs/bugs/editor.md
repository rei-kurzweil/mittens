# Editor bugs

## GLTF armature visualization should render in overlay

### Status

Open bug / regression note.

### Symptom

When a GLTF is viewed under an editor, armature or bone visualization does not consistently render in the overlay phase.

This makes the visualization compete with ordinary scene depth instead of behaving like other editor affordances.

### Expected behavior

Editor-owned armature visualization should render like gizmos and other editor helpers:

- always in the overlay phase
- visually grouped with the active editor tool layer
- not hidden by ordinary scene geometry just because the underlying model is occluded

### Likely cause

The spawned armature visualization subtree is probably not inheriting `OverlayComponent` from the editor presentation path, or it is being spawned under a branch that bypasses the usual overlay wrapping used by gizmos.

Relevant systems to inspect:

- `src/engine/ecs/system/gltf_system.rs`
- `src/engine/ecs/system/gizmo_system.rs`
- `src/engine/ecs/system/editor_system.rs`
- `src/engine/ecs/system/renderable_system.rs`

### Investigation checklist

- identify where armature/bone debug visuals are spawned
- confirm whether those renderables sit under an `OverlayComponent` ancestor
- compare their ancestry to transform gizmo visuals, which already force overlay rendering
- verify whether editor ancestry alone should imply overlay for these debug visuals, or whether the spawning code should wrap them explicitly

### Likely fix direction

Match the gizmo pattern: wrap editor-only armature debug visuals in an overlay-marked subtree at spawn time.

That keeps the rule explicit and avoids making `RenderableSystem` infer overlay from broad editor ancestry alone.
