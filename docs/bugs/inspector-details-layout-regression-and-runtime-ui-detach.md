# Bug: Inspector detail split view renders oversized/overlapping content and escapes `editor_runtime_ui_root`

## Status

Open bug / investigation note.

## Summary

After adding the inspector detail split view, the inspector panel can render in a visibly broken
state:

- the component list is not readable
- the detail view is not readable
- a large green button is prominent in the title/content area
- oversized text overlaps the panel content

The same repro also shows the panel trees materialized as top-level roots instead of ending up
cleanly under `editor_runtime_ui_root`.

## Repro

Observed from a live topology dump after the inspector detail work landed.

Top-level roots included both the shared runtime UI root and loose panel roots:

```text
16: editor_runtime_ui_root  type=transform
17: paint_panel_root        type=transform
18: world_panel_root        type=transform
20: inspector_panel_root    type=transform
25: inspector_panel_content_root type=transform
```

And the inspector shell itself still has the new split slots:

```text
inspector_panel_root
  title_bar
  content_slot
    content_area
      sidebar_slot
      detail_slot
```

## Expected

- The inspector sidebar should show the component list at normal panel text size.
- The detail slot should show the selected component fields at normal panel text size.
- The split view should fit inside the inspector panel without overlapping giant text.
- Runtime editor panels should be attached beneath `editor_runtime_ui_root`, not left as
  independent top-level roots.

## Actual

- The split view is visually broken: the list and detail content are effectively unusable.
- A large green affordance dominates the panel, consistent with the inspector pin/title area being
  much more visible than the actual content.
- Panel-related roots appear at top level alongside `editor_runtime_ui_root` instead of only under
  it.

## Likely causes

### 1. New inspector detail subtree never sets panel-sized text

The older sidebar rows explicitly set panel text size:

- [assets/components/panel_items.mms](/home/rei/_/cat-engine/assets/components/panel_items.mms:151)
  sets `font_size(1)` on `inspector_panel_row`.

The new split-view/detail layout does not do the same:

- [assets/components/panels.mms](/home/rei/_/cat-engine/assets/components/panels.mms:321)
  `content_area` / `sidebar_slot` / `detail_slot` do not set `font_size(...)`
- [assets/components/inspector_details.mms](/home/rei/_/cat-engine/assets/components/inspector_details.mms:12)
  detail rows and labels/values also do not set `font_size(...)`

This is a strong match for the observed "overlapping giant text" regression. The split-view
subtrees are likely inheriting an unintended text scale instead of the normal panel row size.

### 2. Inspector width constants are out of sync between MMS and Rust

The MMS panel width was doubled for the split view:

- [assets/components/panels.mms](/home/rei/_/cat-engine/assets/components/panels.mms:265)
  `INSPECTOR_PANEL_WIDTH_GU = 44.0`

But the stopgap adapter still uses the old width:

- [src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs](/home/rei/_/cat-engine/src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs:61)
  `const INSPECTOR_PANEL_WIDTH_GU: f64 = 22.0;`

Even if text sizing were correct, this stale Rust-side width can leave layout calculations out of
date for the new split inspector.

### 3. Panel trees are spawned root-first and attached later

The stopgap adapter still spawns the inspector instance and detail/sidebar subtrees with
`spawn_tree(..., None, ...)`, then queues later attach intents:

- [src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs](/home/rei/_/cat-engine/src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs:3228)
  `spawn_tree(&panel_ce, None, world, emit)`
- [src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs](/home/rei/_/cat-engine/src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs:2559)
  later attaches inspector instances to the layout root
- [src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs](/home/rei/_/cat-engine/src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs:1498)
  later attaches the panel mount to `panel_query_root`

This matches the topology symptom where panel roots are still visible as independent roots.

There is also a suspicious duplicate attach of the panel mount:

- [src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs](/home/rei/_/cat-engine/src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs:1660)
- [src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs](/home/rei/_/cat-engine/src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs:1700)

The mount is queued for attach to `panel_query_root` twice in the same spawn path.

## Concrete suspects

- [assets/components/panels.mms](/home/rei/_/cat-engine/assets/components/panels.mms)
  - `inspector_panel`
  - `content_slot`
  - `content_area`
  - `sidebar_slot`
  - `detail_slot`
- [assets/components/inspector_details.mms](/home/rei/_/cat-engine/assets/components/inspector_details.mms)
  - `detail_row`
  - `inspector_details`
- [src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs](/home/rei/_/cat-engine/src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs)
  - `spawn_panel_layout`
  - `rerender_inspector_panels`
  - `spawn_inspector_panel_instance_tree`
  - `rerender_single_inspector_panel_sidebar`
  - `rerender_single_inspector_panel_detail`

## Notes

- The bright green button is probably not the primary bug by itself; it is likely just the pin/title
  affordance remaining visible while the actual split content is laid out or scaled incorrectly.
- The hard-coded yellow `content_slot` background in
  [assets/components/panels.mms](/home/rei/_/cat-engine/assets/components/panels.mms:312)
  also looks like debug/test styling that now makes the regression more obvious.
- The runtime topology issue may be older than the detail-view styling regression, but it is now
  easier to observe because the new panel structure is more complex and is being re-rendered more
  often.
