# Editor UI

Date: 2026-06-27

Status: proposed v1 spec.

## Goal

Define the first explicit `EditorUI` authored/runtime boundary.

The first version is intentionally narrow:

- `EditorUI` should expose the shared editor panel `LayoutRoot` as authored topology
- that topology should be placeable from MMS / scene graph code like any other subtree
- editor initialization should auto-provide a default `EditorUI` when editor scopes exist but no authored `EditorUI` is present
- shared editor workspace state, default panel composition, and panel refresh logic should remain in Rust for now

This is not yet a panel-preset or panel-subset spec.

## Problem

Today the editor panel shells are authored in MMS, but the shared editor UI mount is assembled in Rust.

Current shape in
[src/engine/ecs/system/panel_system.rs](/home/rei/_/cat-engine/src/engine/ecs/system/panel_system.rs:245)
and
[src/engine/ecs/system/panel_system.rs](/home/rei/_/cat-engine/src/engine/ecs/system/panel_system.rs:557):

- Rust builds `T.position(...)`
- Rust wraps that in `Overlay`
- Rust creates the shared `LayoutRoot`
- Rust inserts the panel component expressions as `LayoutRoot` children

That means MMS can currently author panel internals, but it cannot author the editor UI mount as scene topology.

## Decision

Introduce an authored `EditorUI(...)` component export.

`EditorUI` owns:

- the outer transformable editor UI mount
- the `Overlay`
- the shared `LayoutRoot`
- the stable authored selector/name contract for that subtree

Rust still owns:

- which panel shells are instantiated by default
- panel shell materialization inputs
- default `EditorUI` bootstrap when none is authored
- runtime discovery of panel roots, slots, and controls
- projection/rerender of dynamic content
- shared editor workspace state and reducers

## V1 scope

`EditorUI` v1 is only about authored placement and authored topology exposure.

It should enable:

- wrapping editor UI in `T {}` or other authored scene graph structure
- spawning editor UI from MMS as a regular component subtree
- automatic fallback insertion of a default transform-wrapped `EditorUI` when needed
- querying the shared panel layout root and panel shells through stable selectors

It should not yet decide:

- default panel subsets
- workspace presets
- panel ordering policies exposed to users
- panel-state persistence or per-instance panel configuration

## Required topology contract

The authored `EditorUI` subtree must provide stable nodes that Rust can resolve after spawn.

Minimum required nodes:

- `#editor_panel_layout_mount`
- `#editor_panel_layout_root`
- `#editor_panel_layout_selection`

The subtree must also still contain the panel shell roots and existing panel-local selectors that runtime code already uses, such as:

- `#world_panel_root`
- `#paint_panel_root`
- `#assets_root`
- `#grid_panel_root`
- `#pose_capture_panel_root`
- `#editor_settings_panel_root`

The exact internal wrapper structure may change, but the selector contract must stay stable across refreshes.

## Bootstrap rule

When editor systems initialize:

- if there is one or more `Editor {}` component in the world
- and there is no `EditorUI` component already present in the world
- runtime should add one automatically

The automatically inserted fallback should be wrapped in a transform root so it remains placeable and queryable as normal topology.

Conceptually:

```text
T { name = "editor_runtime_ui_root" ...default placement... }
  EditorUI { ...default args... }
```

If an authored `EditorUI` already exists, runtime must not spawn a duplicate fallback shared editor UI.

## Placement model

`EditorUI` should be placeable by authored transform ancestry rather than by Rust-only mount position.

That means the effective shape should become conceptually:

```text
T { ... parent-authored placement ... }
  EditorUI { ... }
    Overlay
      LayoutRoot { ... panels ... }
```

instead of:

```text
Rust spawn:
  T.position(...)
    Overlay
      LayoutRoot { ... panels ... }
```

Rust may still pass sizing/configuration arguments into `EditorUI`, but the mount itself should be authored.

For the fallback bootstrap path, runtime may still create the outer transform automatically, but the inserted subtree should still use the same `EditorUI` contract as the authored path.

## Runtime contract

After `EditorUI` is spawned, Rust should:

1. resolve the shared layout root and panel roots by selector
2. cache the resolved ids in workspace/panel runtime state
3. install selection/handler wiring against authored control nodes
4. render dynamic panel content into authored slots

Rust should no longer manually construct the editor UI mount CE for the shared layout root.

For bootstrap, the runtime sequence should be:

1. discover whether any `Editor {}` scope exists
2. discover whether any `EditorUI` already exists
3. if not, insert one fallback transform-wrapped `EditorUI`
4. resolve the resulting shared layout root and panel nodes

## Relationship to a later v2

V2 may choose to move more ownership into MMS, including:

- default panel sets
- named panel subsets
- optional panel inclusion
- authored workspace presets

That should be treated as a separate state/configuration problem.

The v1 `EditorUI` contract should be designed so those later changes can happen without moving the root ownership back into Rust.

## Implementation direction

The likely implementation path is:

1. add an MMS export such as `editor_ui(...)`
2. move the current Rust-authored mount topology into that export
3. keep Rust-side panel materialization and runtime resolution behavior intact
4. replace direct `build_panel_layout_mount_ce(...)` usage for the shared editor UI path with `editor_ui(...)` materialization

## Non-goals

- redesigning panel workspace state
- adding panel presets
- adding user-configurable panel subsets
- solving multi-workspace editor ownership
- changing panel-local authored shell structure beyond what is needed to preserve selectors
