# Editor UI

Date: 2026-06-27

Status: accepted v1 contract.

## Goal

Define the first explicit `EditorUI` authored/runtime boundary.

The first version defines authored placement and a configurable panel subset:

- `EditorUI` should expose the shared editor panel `LayoutRoot` as authored topology
- that topology should be placeable from MMS / scene graph code like any other subtree
- editor initialization should auto-provide a default `EditorUI` when editor scopes exist but no authored `EditorUI` is present
- shared editor workspace state and panel refresh logic remain in Rust
- `panels([...])` chooses which panel shells are materialized

Supported names are `settings`, `paint`, `color`, `grid`, `pose`, `assets`,
`world`, and `inspector`. Unknown names are evaluation errors, duplicate names
are normalized, and shells always use that canonical order. `EditorUI {}` means
all panels.

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

- canonical panel ordering and panel shell materialization
- panel shell materialization inputs
- default `EditorUI` bootstrap when none is authored
- runtime discovery of panel roots, slots, and controls
- projection/rerender of dynamic content
- shared editor workspace state and reducers

## V1 scope

`EditorUI` v1 covers authored placement, topology exposure, and panel selection.

It should enable:

- wrapping editor UI in `T {}` or other authored scene graph structure
- spawning editor UI from MMS as a regular component subtree
- automatic fallback insertion of a default transform-wrapped `EditorUI` when needed
- querying the shared panel layout root and panel shells through stable selectors

It should not yet decide:

- workspace presets
- user-defined panel ordering
- panel-state persistence or per-instance panel configuration

## Required topology contract

Every materialized `EditorUI` subtree provides these stable shared nodes:

Minimum required nodes:

- `#editor_panel_layout_mount`
- `#editor_panel_layout_root`
- `#editor_panel_layout_selection`

It contains only the selected panel shell roots and their existing panel-local selectors, such as:

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

If an authored `EditorUI` already exists, runtime must not spawn a duplicate fallback shared editor UI. One shared workspace is supported; the first explicit instance wins and additional instances are ignored with a warning. `Editor.panels(false)` remains the master opt-out.

## Placement model

`EditorUI` should be placeable by authored transform ancestry rather than by Rust-only mount position.

That means the effective shape should become conceptually:

```text
T { ... parent-authored placement ... }
  EditorUI { ... }
    T { name = "editor_panel_layout_mount" }
      LayoutRoot { ... selected panels ... }
```

instead of:

```text
Rust spawn:
  T.position(...)
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

## Later work

Named workspace presets, user-defined ordering, state persistence, and multiple
independent workspaces remain later work.

## Non-goals

- redesigning panel workspace state
- adding panel presets
- solving multi-workspace editor ownership
- changing panel-local authored shell structure beyond what is needed to preserve selectors
