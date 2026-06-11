# Breaking up the editor stopgap adapter

Date: 2026-06-11

Status: planning / refactor inventory

## Goal

Replace the current monolithic editor stopgap adapter with smaller generic panel infrastructure plus panel-specific modules.

Target file being decomposed:
- [`src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs`](/home/rei/_/cat-engine/src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs:1)

That file currently mixes:
- runtime editor UI root creation
- MMS shell materialization
- dynamic row/detail reconcile
- panel click routing
- world panel save/load logic
- inspector workspace state orchestration
- selection synchronization

The result is not one “bad abstraction”, it is several abstractions collapsed into one file.

## What the file is doing today

High-level responsibilities visible in the file:

1. Editor runtime UI bootstrap
   - create/find `editor_runtime_ui_root`
   - create shared panel layout root/mount
   - place world/paint/assets/inspector panels

2. MMS shell materialization
   - resolve asset paths
   - build panel component expressions
   - decorate panel roots
   - spawn static panel shells

3. Dynamic content projection
   - rerender world panel content
   - rerender inspector sidebar
   - rerender inspector detail
   - track rendered panel models
   - coordinate `DataRendererSystem`

4. Click/focus routing
   - focus panel on click
   - detect which panel region was clicked
   - dispatch row/button/pin actions
   - resolve clicked row ancestry / payloads

5. Panel-specific domain behavior
   - world panel scene model rebuild and save/load
   - inspector workspace reducer sync
   - selection-to-inspector retargeting

6. Local generic utilities
   - subtree ancestry helpers
   - path helpers
   - slot finding
   - instance id wiring

These need to be separated by ownership, not just moved into smaller files arbitrarily.

## What should become generic

To kill the stopgap adapter, the generic pieces need first-class representations.

## 1. Panel shell spec

We need a generic description of a panel shell that says:
- which MMS component to materialize
- what static args it takes
- which named slots inside it are used for dynamic content
- which named descendants act as action targets

Example shape:

```text
PanelShellSpec
  asset_path
  export_name
  args
  selectors:
    root
    content_slot
    status_slot?
    detail_slot?
    selection_root?
```

Why:
- right now selector knowledge is scattered across constants in the stopgap adapter
- every panel-specific reconcile path re-finds descendants ad hoc

## 2. Panel instance registry

We need one generic runtime registry for mounted editor panels:
- panel kind
- root component id
- slot component ids
- owning editor root
- optional instance id

Example shape:

```text
PanelInstance
  panel_kind
  editor_root
  root
  slots: HashMap<PanelSlotKind, ComponentId>
  actions: HashMap<PanelActionTarget, ComponentId>
  instance_id: Option<u64>
```

Why:
- the current file repeatedly finds panel nodes by selector
- mounted panel discovery should happen once, near spawn/reconcile time

## 3. Slot-driven render contract

`DataRendererSystem` already owns remove/spawn/attach for slots.

What still needs to be represented generically is the panel-facing contract:
- which slot receives list content
- which slot receives detail content
- which `RendererSpec` is used for each slot
- which payload model feeds it

Example:

```text
PanelProjectionSpec<ListModel, DetailModel>
  list_slot
  list_renderer: RendererSpec<ListItem>
  detail_slot?
  detail_renderer?
```

Why:
- today the adapter still orchestrates too much slot-level work manually

## 4. Panel action payload contract

Repeated panel content needs a generic way to describe semantic actions.

Current rows mostly encode ad hoc `DataComponent` fields like:
- `row_name`
- `row_kind`
- `target_component`

That is not enough for richer rows like grid rows with multiple action targets.

We need a shared action payload shape:
- action kind
- logical item key
- target component reference
- optional panel instance id
- optional field key

Example:

```text
PanelActionPayload
  panel_kind
  action_kind
  item_key
  target_component?
  instance_id?
  field_key?
```

Why:
- click routing should decode one generic payload format, then delegate semantically
- this is especially needed for `grid_panel` row body / eye / X / add button

## 5. Panel controller trait or equivalent

Each panel needs panel-specific logic, but not panel-specific shell plumbing.

We need a generic controller seam that owns:
- state/model building
- action reduction
- optional detail projection

Example responsibilities:
- `build_shell_args(...)`
- `build_list_items(...)`
- `build_detail_item(...)`
- `handle_action(...)`
- `sync_from_editor_context(...)`

This does not need to be a Rust trait if closures or plain modules work better, but the responsibilities need to be explicit.

Why:
- today world-panel save/load, inspector workspace, and future grid-panel behavior all live in one place because there is no dedicated panel controller seam

## 6. Shared panel layout manager

Runtime UI root and panel placement should be one generic subsystem.

It should own:
- create/find runtime UI root
- create/find panel layout root
- attach panel shells into layout
- panel ordering / row placement
- width-based placement logic

Why:
- panel layout spawning is not inspector-specific
- it is editor workspace infrastructure

## 7. Generic editor workspace state

The stopgap file currently mixes:
- active editor context
- world panel scene model
- inspector workspace state
- rendered inspector models

At minimum we should separate:
- shared editor workspace state
- per-panel controller state

Likely generic workspace concepts:
- active editor root
- focused panel
- mounted panels
- panel ordering/layout metadata

Panel-specific state stays outside:
- inspector pin/subtree state
- world panel scene rows
- grid panel entry cache if needed

## What should stay panel-specific

Not everything should become generic.

These should remain in dedicated panel modules:

- world scene traversal and labels
  - [`src/engine/ecs/system/editor/world_panel.rs`](/home/rei/_/cat-engine/src/engine/ecs/system/editor/world_panel.rs:1)
- inspector workspace reducer and detail modeling
  - [`src/engine/ecs/system/editor/inspector_panel.rs`](/home/rei/_/cat-engine/src/engine/ecs/system/editor/inspector_panel.rs:1)
- future grid enumeration and actions
  - `src/engine/ecs/system/editor/grid_panel.rs`

The rule should be:
- generic infra knows how to mount, render, and route
- panel modules know what their data means

## Suggested decomposition

## Layer 1: editor workspace shell

Possible file:
- `src/engine/ecs/system/editor/workspace.rs`

Owns:
- runtime UI root
- panel layout root/mount
- mounted panel registry
- panel shell spawn/find helpers

## Layer 2: generic panel runtime

Possible files:
- `src/engine/ecs/system/editor/panel_runtime.rs`
- `src/engine/ecs/system/editor/panel_actions.rs`

Owns:
- `PanelShellSpec`
- `PanelInstance`
- slot lookup
- generic action payload decode
- `DataRendererSystem` integration helpers

## Layer 3: panel controllers

Possible files:
- existing `world_panel.rs`
- existing `inspector_panel.rs`
- new `grid_panel.rs`
- later `assets_panel.rs`, `paint_panel.rs`

Owns:
- panel model/state
- `RendererSpec` choices
- action semantics
- sync with editor selection/focus

## Layer 4: editor orchestration

Possible file:
- `src/engine/ecs/system/editor/system.rs` or extend existing `editor/mod.rs`

Owns:
- install shared handlers once
- fan out decoded actions to the right panel controller
- trigger controller refreshes when editor state changes

## Smallest useful first refactor

The adapter is too large to replace in one move. The first extraction should target the least controversial generic piece.

Recommended order:

1. Extract runtime UI root + panel layout spawning into a workspace/layout module.
2. Extract panel shell spawn/find and selector bookkeeping into a generic panel runtime module.
3. Move world panel save/load click handling back behind a world-panel controller seam.
4. Move inspector workspace click/detail sync behind an inspector-panel controller seam.
5. Add `grid_panel` on top of that split rather than inside the old adapter.

This matters because if `grid_panel` is added directly into the stopgap adapter first, it will make the later extraction harder.

## Concrete generic representations to add

If we want a checklist of the things that need names/types:

- `PanelKind`
- `PanelShellSpec`
- `PanelSlotKind`
- `PanelInstance`
- `PanelActionKind`
- `PanelActionPayload`
- `PanelProjectionSpec`
- `EditorWorkspaceRuntime`
- panel controller interface or module contract

Those are the representations missing today.

## Relationship to RendererSpec

`RendererSpec` already solved one important split:
- MMS for static/low-count UI
- Rust factories for repeated/dynamic content

The stopgap adapter persists because the repo still lacks the generic representations around:
- shell mounting
- slot ownership
- action routing
- mounted panel identity
- controller delegation

So the path forward is not replacing `RendererSpec`.
It is building the generic panel runtime around it.

## Summary

To break up the huge stopgap file, the editor needs a real panel runtime model. The key generic concepts are:
- shell spec
- mounted panel instance registry
- slot projection contract
- action payload contract
- workspace layout/runtime
- panel controller seam

Once those exist, the remaining files become much smaller and much more obviously panel-specific.
