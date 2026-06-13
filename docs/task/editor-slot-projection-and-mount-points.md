# Editor slot projection and mount points

Date: 2026-06-12

Status: proposed task / runtime-boundary clarification

## Goal

Define the next refactor step around slot projection for editor UI:

- how authored MMS component trees expose mount points
- how runtime code resolves and caches those mount points
- how dynamic content is projected under them
- which pieces should stay authored in MMS shells instead of being re-added from Rust

This task is specifically about the path around:
- [`src/engine/ecs/system/panel_system.rs`](/home/rei/_/cat-engine/src/engine/ecs/system/panel_system.rs:1)
- [`src/engine/ecs/system/data_renderer_system.rs`](/home/rei/_/cat-engine/src/engine/ecs/system/data_renderer_system.rs:1)
- [`src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs`](/home/rei/_/cat-engine/src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs:1)

It is not a reducer redesign task.

## Core conclusion

The generic need is:

1. materialize an authored modular tree
2. query that live component tree for named mount points and control nodes
3. cache those resolved ids in a runtime instance object
4. project dynamic content under the resolved mount points
5. attach runtime handlers or update local state against already-authored shell nodes

The important correction is:

- `SelectionComponent` roots and pin-button structure should already be authored in the MMS shells where they belong
- Rust should not keep re-creating those shell-level structures after every projection
- Rust should resolve/query those nodes after spawn, then:
  - attach handlers
  - update their state
  - project dynamic content into the mount points they own

That means the world panel item slot should sit inside a `SelectionComponent` subtree authored by the world-panel shell, not outside it and not added later by Rust.

## Current behavior

## 1. Shell materialization and mount-point discovery

Today the repo already has the first half of the right model.

Generic pieces already present:
- [`PanelShellSpec`](/home/rei/_/cat-engine/src/engine/ecs/system/panel_system.rs:37)
- [`PanelInstance`](/home/rei/_/cat-engine/src/engine/ecs/system/panel_system.rs:47)
- [`resolve_panel_instance(...)`](/home/rei/_/cat-engine/src/engine/ecs/system/panel_system.rs:289)
- [`spawn_panel_instance(...)`](/home/rei/_/cat-engine/src/engine/ecs/system/panel_system.rs:312)

That gives us:
- one authored shell tree
- one logical root selector
- a map from logical slot kinds to descendant selectors
- one resolved runtime object containing `ComponentId`s for those slots

For inspector panel instances, this is already used to resolve:
- `Toolbar`
- `Sidebar`
- `Detail`

See:
- [`spawn_inspector_panel_instance_tree(...)`](/home/rei/_/cat-engine/src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs:2867)

## 2. Projection into slots today

Once code has a slot `ComponentId`, it usually calls `DataRendererSystem` directly:
- `render_list(...)`
- `render_detail(...)`
- `clear_slot(...)`

See:
- [`DataRendererSystem`](/home/rei/_/cat-engine/src/engine/ecs/system/data_renderer_system.rs:70)

That system already correctly owns:
- previous rendered subtree tracking per slot
- clear/rebuild semantics per slot
- attach of the new subtree under a chosen slot

So the renderer itself is not the main architectural problem.

## 3. What is still wrong today

The remaining problem is that panel code still mixes three concerns.

### A. repeated mount-point discovery

Examples:
- world panel path re-finds `#content_slot`
- grid panel path re-finds `#content_slot`
- inspector update path re-finds `#sidebar_slot`, `#detail_slot`, and pin-button nodes

Examples:
- [`refresh_world_panels(...)`](/home/rei/_/cat-engine/src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs:854)
- [`update_inspector_panel_instance_tree(...)`](/home/rei/_/cat-engine/src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs:2051)
- [`rerender_grid_panel_from_context(...)`](/home/rei/_/cat-engine/src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs:2438)

### B. projection logic coupled directly to panel roots

Examples:
- build `UiItem`s or `UiDetailItem`
- call `render_list(...)` or `render_detail(...)`
- then keep walking the same shell again for follow-up work

Examples:
- [`rerender_world_panel_content(...)`](/home/rei/_/cat-engine/src/engine/ecs/system/editor/world_panel.rs:527)
- [`rerender_single_inspector_panel_sidebar(...)`](/home/rei/_/cat-engine/src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs:1935)
- [`rerender_single_inspector_panel_detail(...)`](/home/rei/_/cat-engine/src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs:2016)

### C. Rust re-creating shell-level structure that should be authored

Today Rust still does shell-local structure work that should move into MMS-owned shells:

- world panel adds a `SelectionComponent` under the rendered list container
- inspector sidebar adds a `SelectionComponent` under the rendered list container
- inspector shell spawns a pin button into the toolbar slot

Examples:
- [`rerender_world_panel_content(...)`](/home/rei/_/cat-engine/src/engine/ecs/system/editor/world_panel.rs:527)
- [`rerender_single_inspector_panel_sidebar(...)`](/home/rei/_/cat-engine/src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs:1935)
- [`update_inspector_panel_instance_tree(...)`](/home/rei/_/cat-engine/src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs:2051)

This is the specific part that should change.

## Intended authored/runtime split

## 1. MMS shell owns structural control nodes

Shell-authored MMS components should own the stable structure for things like:
- panel root
- title label node
- toolbar mount
- pin-button node or pin-button mount
- selection root
- list/detail/status/content mounts

That means the shell can define:

```text
world_panel_root
  title_label
  world_panel_selection
    content_slot
```

instead of:

```text
world_panel_root
  content_slot
Rust later attaches SelectionComponent under rendered list container
```

For inspector-like shells, similarly:

```text
inspector_panel_root
  title_label
  pin_button
  inspector_sidebar_selection
    sidebar_slot
  detail_slot
```

or, if the button remains a reusable authored subtree:

```text
inspector_panel_root
  toolbar_slot
    pin_button_mount
```

The exact shell shape is flexible.
The important part is that stable shell structure should not be re-authored in Rust on every refresh.

## 2. Runtime resolves authored control nodes after spawn

After materializing and spawning the shell, runtime code should resolve:
- dynamic-content mount points
- authored `SelectionComponent` roots
- authored button nodes or button mounts
- title/status label nodes if needed

Those resolved ids belong in the mounted runtime instance object.

For panels, that likely means `PanelInstance` needs to grow beyond only content slots.

Today it stores:
- panel kind
- root
- slot ids
- optional instance id

The next version likely also needs resolved control-node ids such as:
- selection roots
- button nodes
- title/status labels

Whether those live in:
- a second map like `controls`
- a wider `PanelSlotKind`
- a separate panel-local resolved-node struct

is secondary to the ownership split.

## 3. Runtime attaches handlers and state, not shell structure

Once those nodes are resolved, Rust should:
- configure `SelectionComponent` payload selectors and selected entries
- install or route button handlers
- update title/status text
- project dynamic content under the resolved content mount

Rust should not keep rebuilding:
- the `SelectionComponent` subtree itself
- the existence of a pin button shell node

## Why the world-panel selection slot should be inside the shell selection root

This is the clearest example of the intended direction.

Today world-panel list projection is:

1. resolve `content_slot`
2. render list container into it
3. add a `SelectionComponent` under that rendered list container
4. restore selected entry on that newly added selection root

That makes the selection root an artifact of projection rather than of shell structure.

The better model is:

1. shell authors a stable `SelectionComponent`
2. shell defines the list/content mount beneath that selection root
3. runtime resolves both the selection root and the content mount
4. projection renders rows into that content mount
5. runtime updates the already-existing `SelectionComponent`

Benefits:
- shell ownership is stable across rerenders
- selection topology is not recreated every time the list rerenders
- slot projection stays focused on list/detail subtree replacement
- click/focus routing can target shell-authored selection roots consistently

## Proposed generic model

## 1. Treat authored modular units as mount-point providers

A materialized authored tree should declare:
- one logical root
- zero or more dynamic-content mount points
- zero or more stable control nodes

Conceptually:

```text
MountedView<UnitKey, ControlKey>
  root: ComponentId
  mounts: HashMap<UnitKey, ComponentId>
  controls: HashMap<ControlKey, ComponentId>
```

For panel shells, `UnitKey` can stay panel-specific.

## 2. Distinguish content mounts from control nodes

This distinction matters now.

Not every resolved node is a projection target.

Examples:

### content mounts
- `Content`
- `Sidebar`
- `Detail`
- `Status`

### control nodes
- `WorldSelection`
- `InspectorSidebarSelection`
- `PinButton`
- `TitleLabel`

Projection helpers should work with content mounts.
Handler/state setup should work with control nodes.

## 3. Keep mount-point vocabularies local to each modular unit

This should vary by unit.

Examples:

### world panel shell
- content mount
- selection root
- title label
- save/load button nodes

### inspector panel shell
- sidebar mount
- detail mount
- sidebar selection root
- pin button or pin-button mount
- title label

### detail view shell
- fields mount
- actions mount
- status mount

The runtime should not assume every modular unit has the same vocabulary.

## Proposed first helper layer

The next helper layer should still be small.

It should not absorb:
- panel model building
- reducer semantics
- selection semantics
- button semantics

It should probably provide:

```text
project_list_into_mount(world, emit, mounted_view, mount_key, renderer, items)
project_detail_into_mount(world, emit, mounted_view, mount_key, renderer, detail)
clear_mount(world, emit, mounted_view, mount_key)
```

And separately:

```text
resolve control node by key
configure existing SelectionComponent
attach handlers to existing shell button/control nodes
```

## Immediate task list

### 1. Stop treating shell-local selection roots as renderer output

Migrate world panel and inspector sidebar so:
- `SelectionComponent` roots are authored by MMS shells
- projection targets live under those authored selection roots
- Rust only updates the resolved `SelectionComponent`

### 2. Stop treating the pin button as refresh-spawned shell content

Migrate inspector shell so:
- pin-button structure is authored by the shell
- runtime resolves the pin-button node or mount once
- runtime updates state/handlers against that authored structure

### 3. Extend mounted runtime instances to carry control-node ids

Add a generic way for runtime instances to remember:
- mount points for projected content
- control nodes for selection/buttons/labels

### 4. Add thin projection helpers above `DataRendererSystem`

Only after resolved ids are threaded through consistently:
- keep `DataRendererSystem` slot-id based
- add thin helpers that bridge from logical mount key to resolved mount id

### 5. Rewire world panel first

World panel is the best first target because the selection-root issue is most obvious there.

Acceptance for that step:
- world-panel shell authors the selection root
- world-panel content mount is inside that selection subtree
- Rust no longer creates the selection root after each list render

## Non-goals

This task does not yet require:
- keyed incremental diffing in `DataRendererSystem`
- generic reducer extraction
- complete controller-trait design
- all panel shells to be redesigned at once

## Recommended next implementation move

The next code change after this task note should be:

1. change the world-panel shell so its content slot lives under a shell-authored `SelectionComponent`
2. extend runtime resolution so the world-panel mounted instance resolves both:
   - the content mount
   - the selection root
3. update world-panel rerender code so it:
   - renders rows into the content mount
   - configures the already-existing selection root
   - does not spawn a new `SelectionComponent`

That should be the first concrete proof that shell structure and slot projection are now separated correctly.

## Related

- [panel-system.md](/home/rei/_/cat-engine/docs/task/panel-system.md:1)
- [data-renderer-system-for-editor-ui.md](/home/rei/_/cat-engine/docs/task/data-renderer-system-for-editor-ui.md:1)
- [editor-stopgap-adapter-next-steps.md](/home/rei/_/cat-engine/docs/task/editor-stopgap-adapter-next-steps.md:1)
