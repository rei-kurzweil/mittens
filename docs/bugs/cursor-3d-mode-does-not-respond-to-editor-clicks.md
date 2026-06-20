# Cursor 3D mode does not respond to editor clicks

## Status

Open bug / investigation.

## Symptom

`3D Cursor` mode does not appear to work at all in current testing.

Switching the editor interaction mode to `3D Cursor` and clicking in the editor scene does not
place or update the cursor as expected.

This is distinct from the older cursor issues already documented around GLTF coverage and grid
alignment. This bug is about the mode failing to respond to scene interaction at all.

## Repro

1. Open an editor scene where scene selection normally works.
2. Switch the editor interaction mode to `3D Cursor`.
3. Click editor-scene geometry that should be usable for cursor placement.
4. Observe that the cursor does not appear or update.

## Expected behavior

In `3D Cursor` mode, clicking a valid editor-scene surface should update the shared editor cursor
pose and show/move the cursor marker.

## Actual behavior

The mode appears unresponsive.

The most likely user-facing interpretation is that the click/drag path is no longer reaching
`Cursor3dSystem`, or the system now rejects the event before cursor placement runs.

## Likely root cause

`Cursor3dSystem` still listens specifically for `DragStart`:

- [src/engine/ecs/system/cursor_3d.rs](/home/rei/_/cat-engine/src/engine/ecs/system/cursor_3d.rs:40)
  installs only `SignalKind::DragStart`
- [src/engine/ecs/system/cursor_3d.rs](/home/rei/_/cat-engine/src/engine/ecs/system/cursor_3d.rs:69)
  handles only `EventSignal::DragStart`

That means any recent change that altered:

- which renderable wins the initial gesture hit
- whether `DragStart` is emitted for the relevant click
- whether the event still bubbles under the expected `editor_root`
- whether `resolve_editor_scene_hit(...)` still accepts the hit

could make `3D Cursor` appear dead without changing the cursor code directly.

Potentially relevant wiring:

- [src/engine/ecs/system/system_world.rs](/home/rei/_/cat-engine/src/engine/ecs/system/system_world.rs:849)
  installs `Cursor3dSystem` handlers per editor
- [src/engine/ecs/system/editor_system.rs](/home/rei/_/cat-engine/src/engine/ecs/system/editor_system.rs:40)
  also listens for `DragStart` under the editor subtree
- [src/engine/ecs/system/editor_scene_hit.rs](/home/rei/_/cat-engine/src/engine/ecs/system/editor_scene_hit.rs:10)
  resolves the editor-scene hit used by both selection and cursor behavior

## Why this should be tracked separately

There is already an older bug note:

- [docs/bugs/editor-cursor-3d-gltf-and-grid-alignment.md](/home/rei/_/cat-engine/docs/bugs/editor-cursor-3d-gltf-and-grid-alignment.md:1)

That document covers:

- partial cursor coverage on some objects
- cursor/grid orientation mismatch

This new bug is broader and more basic:

- `3D Cursor` mode seems not to respond at all

So it should be triaged separately to avoid mixing a likely event-routing regression with the older
cursor-pose/alignment issues.

## Investigation targets

- `GestureSystem`
  - confirm `DragStart` is still emitted for the relevant pointer interactions
- `Cursor3dSystem`
  - confirm handlers are installed on the expected `editor_root`
  - log whether `handle_cursor_signal(...)` runs at all
- `resolve_editor_scene_hit(...)`
  - confirm the clicked renderable still resolves to the expected editor hit
- recent editor selection/context cleanup
  - confirm no routing change now causes `DragStart` to be consumed or filtered before cursor
    handling can act on it

## Recommended first debugging step

Add targeted tracing around:

1. `GestureSystem` `DragStart` emission
2. `Cursor3dSystem::handle_cursor_signal(...)`
3. `resolve_editor_scene_hit(...)`
4. the interaction mode seen by `update_editor_cursor(...)`

That should quickly distinguish:

- no `DragStart`
- `DragStart` exists but cursor handler never runs
- cursor handler runs but scene-hit resolution fails
- scene-hit resolution succeeds but placement/update fails later
