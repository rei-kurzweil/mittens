# Bug: Inspector sidebar content depends on sidebar background, and pin/active-panel state regresses

## Status

Open bug / investigation note. Updated after the sidebar became partially visible again.

## Summary

After adding the inspector detail split view work and later disabling the detail pane again, the
inspector panel regressed in multiple ways:

- the sidebar slot can now be made visible by giving `#sidebar_slot` its own background, but the
  actual row/text subtree does not reliably render unless that background exists
- pinning still has UI/state issues:
  - the pin button does not fit the title bar and is pushed down onto a second line
  - the active inspector panel does not always stay on the most recently created unpinned panel
  - selection changes can eagerly create a new inspector panel when reusing the current unpinned
    one would be expected

The current issue is now split between:

- a sidebar render/layout bug
- a separate pinning / active-panel workspace-state bug

## Repro

Observed with the detail view temporarily disabled and only the sidebar path active.

Additional repro note:

- if `#sidebar_slot` has no explicit background, the inspector title updates on selection but the
  sidebar appears empty
- if `#sidebar_slot` is given a light grey background, the sidebar region itself becomes visible
  and the row path starts behaving differently, suggesting a layout / paint / clipping dependency
  on the sidebar container rather than pure data-flow failure

## Expected

- The inspector sidebar should render its component list without requiring a debug background on
  `#sidebar_slot`.
- The inspector sidebar should show the component list at normal panel text size.
- The pin button should stay within the title bar and not wrap to a second line.
- The most recently created unpinned inspector panel should remain the active reuse target until
  the user focuses a different panel or pins it.
- Selection changes should not eagerly spawn a new inspector panel when the active unpinned panel
  should simply retarget.
- The sidebar should fit inside the inspector panel without overlapping giant text.

## Actual

- The inspector title updates correctly when selecting a component in `world_panel`.
- The sidebar slot itself only becomes reliably visible after adding a background to
  `#sidebar_slot`.
- The sidebar row/content subtree still behaves inconsistently relative to that background change,
  which points away from selection/data-flow and toward layout, clipping, or paint ordering.
- Pinning works inconsistently:
  - the pin button overflows the title bar and drops to a second line
  - the active unpinned inspector panel is not always reused
  - a new inspector panel is sometimes spawned too eagerly

## Likely causes

### 1. Sidebar container may need its own painted box to make descendants appear

Recent observation:

- adding an explicit light grey `background_color(...)` to `#sidebar_slot` makes the sidebar
  region become visible and changes the behavior of the missing rows/text

That is suspicious because the world panel content does not require the same workaround to show its
children. This suggests a problem in one of:

- sidebar container sizing
- background / clip / stencil interaction
- paint ordering / z behavior for sidebar descendants
- layout not treating the sidebar as a visible content box until it has painted background bounds

### 2. Sidebar and detail split-view containers rely on inherited text sizing

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

### 3. Inspector width constants are out of sync between MMS and Rust

The MMS panel width was doubled for the split view:

- [assets/components/panels.mms](/home/rei/_/cat-engine/assets/components/panels.mms:265)
  `INSPECTOR_PANEL_WIDTH_GU = 44.0`

But the stopgap adapter still uses the old width:

- [src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs](/home/rei/_/cat-engine/src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs:61)
  `const INSPECTOR_PANEL_WIDTH_GU: f64 = 22.0;`

Even if text sizing were correct, this stale Rust-side width can leave layout calculations out of
date for the new inspector shell.

### 4. Panel trees are spawned root-first and attached later

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

### 5. Inspector workspace state still has active-panel / pinning logic bugs

The current reducer/workspace flow appears to allow cases where:

- the wrong unpinned panel remains active
- a selection change spawns a fresh panel even though the user expectation is to reuse the current
  active unpinned panel

This is likely separate from the sidebar render bug and should be treated independently in the
workspace-state logic.

## Concrete suspects

- [assets/components/panels.mms](/home/rei/_/cat-engine/assets/components/panels.mms)
  - `inspector_panel`
  - `content_slot`
  - `content_area`
  - `sidebar_slot`
  - `detail_slot`
- [src/engine/ecs/system/editor_inspector_system.rs](/home/rei/_/cat-engine/src/engine/ecs/system/editor_inspector_system.rs)
  - `reduce_inspector_workspace_state`
  - `InspectorWorkspaceEvent::SelectionChanged`
- [assets/components/inspector_details.mms](/home/rei/_/cat-engine/assets/components/inspector_details.mms)
  - `detail_row`
  - `inspector_details`
- [src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs](/home/rei/_/cat-engine/src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs)
  - `spawn_panel_layout`
  - `rerender_inspector_panels`
  - `spawn_inspector_panel_instance_tree`
  - `rerender_single_inspector_panel_sidebar`
  - `rerender_single_inspector_panel_detail`
  - shared row subtree generation for world/inspector panel items

## Notes

- The green pin/title affordance likely needs explicit sizing or wrapping control.
- The new shared Rust-side row builder now attaches a `DataComponent` to both world-panel and
  inspector-sidebar rows instead of relying on more bespoke row-specific metadata structures.
- The hard-coded yellow `content_slot` background in
  [assets/components/panels.mms](/home/rei/_/cat-engine/assets/components/panels.mms:312)
  still looks like debug/test styling that makes the regression easier to spot.
- The light grey background on `#sidebar_slot` is currently diagnostic; it should not be assumed to
  be the real fix until the underlying render/layout dependency is understood.
- The detail view can stay disabled until the sidebar layout is stable again.
