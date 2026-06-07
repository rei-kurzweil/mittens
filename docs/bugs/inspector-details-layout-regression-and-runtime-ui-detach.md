# Bug: Inspector sidebar text scale and pin layout regress after inspector split-view work

## Status

Open bug / investigation note.

## Summary

After adding the inspector detail split view, the inspector panel regressed in two ways:

- pinning still works as described, but the pin button does not fit the title bar and is pushed
  down onto a second line
- even with the detail view disabled, the sidebar text is still rendered at a huge scale and the
  rows overlap vertically on a single line

The current issue is now focused on sidebar layout/text sizing rather than the detail view itself.

## Repro

Observed with the detail view temporarily disabled and only the sidebar path active.

## Expected

- The inspector sidebar should show the component list at normal panel text size.
- The pin button should stay within the title bar and not wrap to a second line.
- The sidebar should fit inside the inspector panel without overlapping giant text.

## Actual

- Pinning works, but the pin button overflows the title bar and drops to a second line.
- The sidebar rows are still rendered at an oversized text scale, so the component list overlaps
  vertically and becomes unreadable.

## Likely causes

### 1. Sidebar and detail split-view containers rely on inherited text sizing

The older sidebar rows explicitly set panel text size:

- [assets/components/panel_items.mms](/home/rei/_/cat-engine/assets/components/panel_items.mms:151)
  sets `font_size(1)` on `inspector_panel_row`.

The split-view container and the detail subtree were the first places to audit:

- [assets/components/panels.mms](/home/rei/_/cat-engine/assets/components/panels.mms:321)
  `content_area` / `sidebar_slot` / `detail_slot` do not set `font_size(...)`
- [assets/components/inspector_details.mms](/home/rei/_/cat-engine/assets/components/inspector_details.mms:12)
  detail rows and labels/values also do not set `font_size(...)`

Even with the detail subtree disabled, the sidebar is still affected, which suggests the inherited
text scale is being introduced higher in the inspector shell or during panel materialization.

### 2. Inspector width constants are out of sync between MMS and Rust

The MMS panel width was doubled for the split view:

- [assets/components/panels.mms](/home/rei/_/cat-engine/assets/components/panels.mms:265)
  `INSPECTOR_PANEL_WIDTH_GU = 44.0`

But the stopgap adapter still uses the old width:

- [src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs](/home/rei/_/cat-engine/src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs:61)
  `const INSPECTOR_PANEL_WIDTH_GU: f64 = 22.0;`

Even if text sizing were correct, this stale Rust-side width can leave layout calculations out of
date for the new inspector shell.

### 3. Panel trees are spawned root-first and attached later

The stopgap adapter still spawns the inspector instance and detail/sidebar subtrees with
`spawn_tree(..., None, ...)`, then queues later attach intents:

- [src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs](/home/rei/_/cat-engine/src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs:3228)
  `spawn_tree(&panel_ce, None, world, emit)`
- [src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs](/home/rei/_/cat-engine/src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs:2559)
  later attaches inspector instances to the layout root
- [src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs](/home/rei/_/cat-engine/src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs:1498)
  later attaches the panel mount to `panel_query_root`

This remains a possible contributor to layout timing, but it is not the primary repro now that the
detail view is disabled.

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

- The green pin/title affordance likely needs explicit sizing or wrapping control.
- The hard-coded yellow `content_slot` background in
  [assets/components/panels.mms](/home/rei/_/cat-engine/assets/components/panels.mms:312)
  still looks like debug/test styling that makes the regression easier to spot.
- The detail view can stay disabled until the sidebar layout is stable again.
