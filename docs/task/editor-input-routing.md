# Task: Editor Input Routing Rationale and Seams

Date: 2026-07-11

Status: open

## Why this note exists

Recent `bisket-vr-demo` debugging exposed that editor interaction is currently split across two
different routing models:

- scoped signal delivery through `RxWorld`
- ad hoc global bridges added by specific editor systems

That split is now a real seam in the architecture. Some interactions only work for objects inside
an `ED {}` subtree, while others partially work on arbitrary world geometry outside the editor
subtree.

This document records the rationale behind the current design, what the code actually does today,
and where the seams are that need to be cleaned up.

## Core routing rule in `RxWorld`

Scoped handlers in `RxWorld` are not "workspace-wide editor listeners".

They only run when the event scope is in the handler's ancestor chain:

- a handler is registered on `(signal_kind, scope_root)`
- when an event is dispatched with `scope = S`
- `RxWorld` computes the chain `S -> parent(S) -> parent(parent(S)) -> ...`
- only handlers attached to one of those nodes run

Relevant code:

- [src/engine/ecs/rx/rx_world.rs](/home/rei/_/cat-engine/src/engine/ecs/rx/rx_world.rs:70)
- [src/engine/ecs/rx/rx_world.rs](/home/rei/_/cat-engine/src/engine/ecs/rx/rx_world.rs:465)
- [src/engine/ecs/rx/rx_world.rs](/home/rei/_/cat-engine/src/engine/ecs/rx/rx_world.rs:507)

This is important because editor systems were originally installed as scoped handlers on the
`editor_root`, which only works if the clicked renderable is actually under that editor subtree.

## Original rationale for scoped editor handlers

The original scoped design had a reasonable goal:

- allow multiple independent `EditorComponent` subtrees
- let each editor react only to interactions within its own scene subtree
- keep selection, gizmo, and panel state editor-local

That model makes sense if "editor interaction targets" are defined as:

- only authored content inside `ED {}`
- only renderables parented under the `editor_root`

Under that assumption, a scoped handler rooted at `editor_root` is a clean fit.

## What changed in practice

The current intended behavior is broader than the original scoped model:

- editor UI inspection and gizmo/selection should be limited to `ED {}` content
- but event-driven tools such as `3D Cursor`, free draw, and other scene-facing tools should be
  able to work on arbitrary world objects

That means the interaction target set is now split:

- inspectable / gizmo-able content is editor-scoped
- hittable world surfaces are not necessarily editor-scoped

Once that changed, a purely scoped handler model stopped matching the product semantics.

## Current code behavior

### `EditorSystem`

`EditorSystem` selection is still scoped-only.

- installs `SignalKind::Click` on `editor_root`
- relies on `RxWorld` ancestor-chain dispatch
- therefore only receives clicks from renderables inside the editor subtree

Relevant code:

- [src/engine/ecs/system/editor_system.rs](/home/rei/_/cat-engine/src/engine/ecs/system/editor_system.rs:23)
- [src/engine/ecs/system/editor_system.rs](/home/rei/_/cat-engine/src/engine/ecs/system/editor_system.rs:35)

### `Cursor3dSystem`

`Cursor3dSystem` is already mixed-mode.

It installs:

- a scoped `Click` handler on `editor_root`
- a global `Click` bridge for non-editor hits

The global bridge does this:

- listens to every click
- resolves `resolve_world_scene_hit(...)`
- ignores hits already under some editor root
- routes non-editor world hits to the active editor cursor state

Relevant code:

- [src/engine/ecs/system/cursor_3d.rs](/home/rei/_/cat-engine/src/engine/ecs/system/cursor_3d.rs:23)
- [src/engine/ecs/system/cursor_3d.rs](/home/rei/_/cat-engine/src/engine/ecs/system/cursor_3d.rs:36)
- [src/engine/ecs/system/cursor_3d.rs](/home/rei/_/cat-engine/src/engine/ecs/system/cursor_3d.rs:51)

### `EditorPaintSystem`

`EditorPaintSystem` is still strongly rooted in editor-scoped eligibility rules.

Its scene-hit path still encodes "eligible scene hit" primarily in terms of editor ancestry and
panel exclusions, which is a separate semantic layer from plain ray hit delivery.

Relevant code:

- [src/engine/ecs/system/editor_paint_system.rs](/home/rei/_/cat-engine/src/engine/ecs/system/editor_paint_system.rs:501)
- [src/engine/ecs/system/editor_paint_system.rs](/home/rei/_/cat-engine/src/engine/ecs/system/editor_paint_system.rs:1796)

### `resolve_world_scene_hit(...)`

`resolve_world_scene_hit(...)` is already broader than the older editor-only model:

- it accepts hits with `editor_root: None`
- it rejects based on semantic blockers like `Selectable.off()` and gizmo ancestry
- it resolves a usable target transform for either editor-scoped or non-editor world hits

Relevant code:

- [src/engine/ecs/system/editor_scene_hit.rs](/home/rei/_/cat-engine/src/engine/ecs/system/editor_scene_hit.rs:31)

## The important seam

The main seam is this:

- event delivery is still partly structural and scoped
- interaction semantics are now partly workspace-global and tool-specific

That creates a mismatch between:

1. who receives the click
2. who is semantically allowed to act on the click

Right now, each editor subsystem answers that mismatch differently:

- `EditorSystem` says "only scoped clicks exist"
- `Cursor3dSystem` says "scoped clicks plus a global bridge for non-editor hits"
- paint uses its own eligibility layer

So the architecture has drifted from one routing model into several overlapping ones.

## Why this is fragile

### 1. Different tools disagree about what counts as a reachable scene hit

A world object can be:

- ray-hittable
- accepted by `resolve_world_scene_hit(...)`
- usable by `Cursor3dSystem`
- but still invisible to `EditorSystem` selection because selection never saw the click

That inconsistency is exactly the sort of bug that surfaced in terrain testing.

### 2. Scope and semantics are tangled

There are really two separate questions:

1. should the editor subsystem receive this event at all?
2. if it does receive the event, should this tool act on this target?

Scoped routing answered both at once when editor targets were entirely inside `ED {}`.
That is no longer true.

### 3. Behavior now depends on which subsystem happened to grow a global bridge

The current system is not governed by one policy. It is governed by which file was patched most
recently:

- cursor has a global bridge
- selection does not
- paint has separate filters

That is not a stable architecture.

## Rationale for a better split

The cleaner model is not "everything editor-related should be global".

It is:

- editor-scoped handlers decide editor-local context
- global handlers drive workspace-global world tools
- gizmo interaction stays scoped to the gizmo

That separates:

- editor activation and editor-authored selection
- world-surface tools like cursor and paint
- direct gizmo manipulation

Under that model:

- `RxWorld` scoped dispatch remains the right fit for editor subtree ownership
- global handlers stop pretending they need to be routed "through" a specific editor subtree
- `resolve_world_scene_hit(...)` or a successor becomes the semantic boundary for world tools

## Proposed architectural direction

### 1. Scoped editor handlers establish editor context

Scoped handlers on each `EditorComponent` subtree should do only the editor-local work:

- determine that a click belonged to this editor tree
- update which editor is active
- update editor-authored selection state
- provide the insertion/editor context for later actions

They should not be the main transport path for workspace-global tools like cursor placement or
paint.

### 2. Global handlers drive world-facing tools

These should be global listeners:

- `Cursor3dSystem`
- paint / free-draw / other world-surface tools

Those systems can then use:

- active editor
- focused panel state
- tool mode
- `resolve_world_scene_hit(...)`

to decide what to do, without requiring the clicked object to be inside the editor subtree.

Importantly, these global tools do not need the click to be "routed through" a specific editor
tree first. They only need enough editor context to know where new things should go.

### 3. Gizmos remain scoped

Gizmo interaction is the deliberate exception.

There should be one active gizmo, and its handlers should remain scoped to the gizmo subtree /
gizmo target path rather than being promoted to global routing.

This preserves the nice property that gizmo manipulation is only possible when the gizmo itself is
what was hit.

## Suggested policy split

The current product semantics appear to want this:

### Workspace-global tools

These should be allowed to act on any world object:

- `3D Cursor`
- free draw / paint / surface placement tools
- event tools that are about hit surfaces rather than editor ownership

### Editor-scoped context handlers

These should only manage editor-local state:

- active editor changes
- editor-authored selection
- insertion destination / editing context
- inspector/world-panel inspectability

### Scoped gizmo handlers

These should remain attached to the active gizmo path:

- gizmo hit-testing
- gizmo drag/manipulation
- transform edits driven by direct gizmo interaction

## Concrete seams to track

### Seam 1: scoped delivery vs global delivery

- [src/engine/ecs/rx/rx_world.rs](/home/rei/_/cat-engine/src/engine/ecs/rx/rx_world.rs:465)
- [src/engine/ecs/system/editor_system.rs](/home/rei/_/cat-engine/src/engine/ecs/system/editor_system.rs:35)
- [src/engine/ecs/system/cursor_3d.rs](/home/rei/_/cat-engine/src/engine/ecs/system/cursor_3d.rs:51)

### Seam 2: structural ancestry vs semantic scene-hit acceptance

- [src/engine/ecs/system/editor_scene_hit.rs](/home/rei/_/cat-engine/src/engine/ecs/system/editor_scene_hit.rs:31)

### Seam 3: active editor ownership vs arbitrary world target

- [src/engine/ecs/system/editor/context.rs](/home/rei/_/cat-engine/src/engine/ecs/system/editor/context.rs:39)
- [src/engine/ecs/system/cursor_3d.rs](/home/rei/_/cat-engine/src/engine/ecs/system/cursor_3d.rs:73)

### Seam 4: editor selection semantics vs cursor / paint / surface-placement semantics

- [src/engine/ecs/system/editor_system.rs](/home/rei/_/cat-engine/src/engine/ecs/system/editor_system.rs:52)
- [src/engine/ecs/system/cursor_3d.rs](/home/rei/_/cat-engine/src/engine/ecs/system/cursor_3d.rs:114)
- [src/engine/ecs/system/editor_paint_system.rs](/home/rei/_/cat-engine/src/engine/ecs/system/editor_paint_system.rs:1796)

### Seam 5: gizmo-local manipulation vs editor-global world tools

- [src/engine/ecs/system/gizmo_system.rs](/home/rei/_/cat-engine/src/engine/ecs/system/gizmo_system.rs:1390)
- [src/engine/ecs/system/editor_system.rs](/home/rei/_/cat-engine/src/engine/ecs/system/editor_system.rs:152)

## Recommended next steps

- [ ] Narrow `EditorSystem` scoped handlers so they are explicitly about editor activation and
      editor-authored selection context
- [ ] Make `Cursor3dSystem` and paint-style tools consistently global-first
- [ ] Define one explicit policy boundary for "workspace-global world hit" vs "editor-scoped
      editing context"
- [ ] Keep gizmo interaction scoped and document that as an intentional exception
- [ ] Remove remaining assumptions that all editor interaction must be delivered through editor
      subtree ancestry

## Non-goals for this task

- not a full pointer/raycast refactor
- not a fix for terrain raycast authoring by itself
- not a world-panel performance fix

This note is specifically about the routing seam between raw click delivery and editor interaction
semantics.
