# Editor slot projection and mount points

Date: 2026-06-12

## Goal

Clarify how editor UI slot projection works today, and propose a cleaner model for modular authored trees that expose named mount points.

This note is specifically about the path around:
- [`src/engine/ecs/system/panel_system.rs`](/home/rei/_/cat-engine/src/engine/ecs/system/panel_system.rs:1)
- [`src/engine/ecs/system/data_renderer_system.rs`](/home/rei/_/cat-engine/src/engine/ecs/system/data_renderer_system.rs:1)
- [`src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs`](/home/rei/_/cat-engine/src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs:1)

It is not trying to redesign panel reducers or panel domain logic.

## Short version

The current system already has most of the right ingredients:

1. authored MMS shell trees
2. named descendants inside those trees
3. runtime discovery of those descendants
4. a `DataRendererSystem` that can clear and re-attach content under a chosen slot

What is still missing is a cleaner generic contract for:

- which authored component tree exposes which mount points
- how those mount points are discovered and cached
- how a caller projects one or more live subtrees into those mount points
- which post-attach panel-local steps still happen after projection

The important conceptual shift is:

- `DataRendererSystem` should not need to know panel selectors
- panel/domain code should not keep repeating selector walks
- the runtime layer should resolve a modular authored tree into a small registry of mount points once

That registry will differ by authored modular unit:
- panel shell
- list view shell
- detail view shell
- toolbar shell
- other small authored subtrees later

So yes: the main generic need is to query a materialized component tree and find the specific transforms that future content should attach under.

## Current behavior

## 1. Authored panel shell materialization

Today a panel shell is usually authored in MMS, materialized, spawned, then queried for named descendants.

The generic pieces already exist in:
- [`PanelShellSpec` in `panel_system.rs`](/home/rei/_/cat-engine/src/engine/ecs/system/panel_system.rs:37)
- [`resolve_panel_instance(...)` in `panel_system.rs`](/home/rei/_/cat-engine/src/engine/ecs/system/panel_system.rs:289)
- [`spawn_panel_instance(...)` in `panel_system.rs`](/home/rei/_/cat-engine/src/engine/ecs/system/panel_system.rs:312)

That gives us:
- one authored shell tree
- one root selector
- a map from logical slot kinds to descendant selectors
- one resolved `PanelInstance` containing those slot `ComponentId`s

For inspector panel instances, this is already used to resolve:
- `Toolbar`
- `Sidebar`
- `Detail`

See:
- [`spawn_inspector_panel_instance_tree(...)`](/home/rei/_/cat-engine/src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs:2867)

## 2. Slot projection today

Once the code has a slot `ComponentId`, it calls `DataRendererSystem` directly.

Current renderer boundary:
- `render_list(...)`
- `render_detail(...)`
- `clear_slot(...)`

See:
- [`DataRendererSystem`](/home/rei/_/cat-engine/src/engine/ecs/system/data_renderer_system.rs:70)

That system currently owns:
- previous rendered subtree tracking per slot
- full-rerender semantics for a slot
- attaching a new root/container under that slot
- clearing previous slot content

This is already generic and useful.

## 3. Where the current path is still too open-coded

The current repo still mixes three separate concerns in panel-specific code:

### A. mount-point discovery

Examples:
- world panel path still does ad hoc `find_component(..., "#content_slot")`
- grid panel path still does ad hoc `find_component(..., "#content_slot")`
- inspector update path still does ad hoc `find_component(..., "#sidebar_slot")` and `"#detail_slot"`

Examples:
- [`refresh_world_panels(...)`](/home/rei/_/cat-engine/src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs:854)
- [`update_inspector_panel_instance_tree(...)`](/home/rei/_/cat-engine/src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs:2051)
- [`rerender_grid_panel_from_context(...)`](/home/rei/_/cat-engine/src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs:2438)

### B. projection into slots

Examples:
- world panel builds `UiItem`s then calls `render_list(...)`
- inspector sidebar builds `UiItem`s then calls `render_list(...)`
- inspector detail builds `UiDetailItem` then calls `render_detail(...)`
- grid panel builds `UiItem`s then calls `render_list(...)`

Examples:
- [`rerender_world_panel_content(...)`](/home/rei/_/cat-engine/src/engine/ecs/system/editor/world_panel.rs:527)
- [`rerender_single_inspector_panel_sidebar(...)`](/home/rei/_/cat-engine/src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs:1935)
- [`rerender_single_inspector_panel_detail(...)`](/home/rei/_/cat-engine/src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs:2016)

### C. post-attach local augmentation

This is the piece that matters when deciding how generic the next seam should be.

After projection, some callers still do local panel-specific work:

- world panel adds a `SelectionComponent` under the rendered list container
- inspector sidebar adds a `SelectionComponent` under the rendered list container
- inspector shell adds a pin button under the toolbar slot
- title labels may update independently from slot content

Examples:
- [`rerender_world_panel_content(...)`](/home/rei/_/cat-engine/src/engine/ecs/system/editor/world_panel.rs:527)
- [`rerender_single_inspector_panel_sidebar(...)`](/home/rei/_/cat-engine/src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs:1935)
- [`update_inspector_panel_instance_tree(...)`](/home/rei/_/cat-engine/src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs:2051)

This means a projection helper should probably return the rendered subtree root/container so panel-local augmentation can continue to happen above it.

## What the current architecture is implicitly assuming

The current code already assumes a modular pattern like this:

```text
materialize authored shell
  -> spawn component tree
  -> query tree for named mount points
  -> cache or retain those mount point ids
  -> project dynamic content under one or more of them
  -> optionally attach panel-local helpers under the projected subtree
```

That pattern is not specific to one panel shell.

It should apply equally to:
- a whole panel shell
- a sub-view shell with its own list mount
- a detail view shell with field-group mounts
- a toolbar shell with icon/button mounts

So the next generic step is not “make `DataRendererSystem` smarter about panels.”
It is:
- define a general mount-point discovery contract for modular authored trees
- then put thin slot-projection helpers on top of that

## Proposed model

## 1. Treat authored modular units as mount-point providers

A materialized authored tree should be able to declare:
- one logical root
- zero or more named mount points

That means a panel shell is one instance of a broader concept:

```text
MountedView<UnitKind>
  root: ComponentId
  mounts: HashMap<MountPointKind, ComponentId>
```

For panels, `MountPointKind` might remain `PanelSlotKind`.

For other authored units, it may be better to use a unit-specific enum or a local string key.

The important point is not the exact type.
The important point is that discovery happens once, near spawn time, and projection code receives resolved ids instead of selectors.

## 2. Separate shell spec from projection spec

Today `PanelShellSpec` already covers the authored shell side reasonably well:
- module path
- export name
- args
- selectors for root and slots

What is still missing is a projection-side contract such as:

```text
SlotProjectionSpec<Model>
  mount_point
  renderer_spec
  build_payload(model) -> list/detail payload
```

That contract would say:
- which resolved mount point receives content
- which renderer renders it
- what model shape is projected into it

This keeps authored-tree discovery separate from data projection.

## 3. Keep mount-point sets local to each modular unit

Your intuition here is correct.

The set of mount points should be different per modular unit.

Examples:

### Panel shell

Possible mount points:
- `Content`
- `Sidebar`
- `Detail`
- `Toolbar`
- `Status`

### List view shell

Possible mount points:
- `Rows`
- `SelectionRoot`
- `Footer`

### Detail view shell

Possible mount points:
- `Fields`
- `Actions`
- `Status`

The runtime should not assume every authored unit has the same mount-point vocabulary.
It should only assume:
- the unit declares a map from logical mount keys to selectors
- the runtime resolves them after spawn

## 4. The next generic helper should be small

The next helper should probably not combine:
- mount discovery
- model building
- renderer choice
- post-attach selection setup
- panel-specific event semantics

That would recreate the same giant-file problem at a different level.

A better first helper is:

```text
project_list_into_mount(world, emit, mounted_view, mount_key, renderer, items)
project_detail_into_mount(world, emit, mounted_view, mount_key, renderer, detail)
clear_mount(world, emit, mounted_view, mount_key)
```

Where `mounted_view` already contains resolved `ComponentId`s.

Then panel code can still do:
- attach a `SelectionComponent`
- restore selected row payload
- attach a pin button
- update title text

That keeps generic projection generic, and keeps panel-local augmentation panel-local.

## Suggested type direction

This is one plausible direction, not a final API:

```text
MountPointKey
  panel-specific enum today
  maybe generic string-like key later

MountedView<K>
  root: ComponentId
  mounts: HashMap<K, ComponentId>

MountableViewSpec<K>
  asset_path
  export_name
  args
  root_selector
  mount_selectors: HashMap<K, String>

ListProjectionSpec<K, Item>
  mount: K
  renderer: RendererSpec<Item>

DetailProjectionSpec<K, Detail>
  mount: K
  renderer: RendererSpec<Detail>
```

The current `PanelShellSpec` and `PanelInstance` are already close to the panel-specific version of this idea.

## Proposed immediate change

Before changing `DataRendererSystem`, introduce one analysis-backed seam:

1. use resolved mount-point maps consistently for panel shells
2. stop doing ad hoc selector walks in update paths when the mounted instance could carry those ids
3. add thin projection helpers that accept resolved mount ids
4. keep post-attach selection/pin/title logic outside those helpers

That means the first implementation step after this note should probably be:

- make world/grid/inspector update paths operate on a resolved panel-instance or mount-point map rather than re-querying selectors from the shell root

Only after that should we decide whether `DataRendererSystem` grows a higher-level helper API.

## Why this is better

This split makes ownership clearer:

- authored unit spec owns selector knowledge
- runtime discovery owns mount-point resolution
- projection helper owns clear/spawn/attach under a resolved mount
- panel module owns model building and post-attach local behavior

That is the smallest step that reduces repeated work without hiding too much.

## Open questions

1. Should mount-point keys stay as enums, or become more generic string-like ids?
   For panels, enums are probably still better for now.

2. Should `DataRendererSystem` learn about resolved mount maps directly?
   Probably not at first. It can stay slot-id based while a thin runtime helper bridges from logical mount key to `ComponentId`.

3. Should selection-root setup become part of generic list projection?
   Probably not initially. World and inspector both add selection, but they still differ in payload selector and selected entry restoration.

4. Do we want modular units smaller than panel shells right away?
   Not required for the first extraction, but the design should allow it.

## Recommended next implementation move

The first code move after this note should be:

1. make mounted shell instances the normal carrier of resolved mount-point ids
2. thread those resolved ids into world/grid/inspector refresh paths
3. only then add a very small projection helper layer above `DataRendererSystem`

That gives a clean stepping stone into the slot-projection refactor without prematurely over-abstracting the renderer.
