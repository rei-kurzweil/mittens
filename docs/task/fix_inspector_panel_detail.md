# Fix Inspector Panel Detail

## Status

Current analysis only. No source changes made yet.

## Symptom

The inspector detail view appears to take the full panel width and visually overlaps the sidebar instead of sitting beside it.

At first glance the authored MMS layout looks correct:

- `sidebar_slot` is `display("inline-block")`
- `detail_slot` is `display("inline-block")`
- their authored widths add up to the panel width once the split gap is included

Relevant authored values in [assets/components/panels.mms](/home/rei/_/cat-engine/assets/components/panels.mms:265):

- `INSPECTOR_PANEL_WIDTH_GU = 44.0`
- `INSPECTOR_PANEL_SIDEBAR_WIDTH_GU = 15.0`
- `INSPECTOR_PANEL_SPLIT_GAP_GU = 0.5`
- `INSPECTOR_PANEL_DETAIL_WIDTH_GU = 28.5`

So the authored arithmetic is not the problem:

- `15.0 + 0.5 + 28.5 = 44.0`

## What is actually going wrong

### 1. The MMS layout is internally consistent

The split-view structure in [assets/components/panels.mms](/home/rei/_/cat-engine/assets/components/panels.mms:338) is authored as:

- `content_area`
- `sidebar_slot`
- `detail_slot`

Both slots are inline-block siblings under the same container, and the widths are derived correctly. The overlap is therefore not explained by a simple authored-width or margin math mistake.

### 2. The layout engine does not treat panel width as a hard visual clamp

The runtime layout engine chooses inline layout only when every immediate child is inline-block, which is true for the `sidebar_slot` / `detail_slot` pair. See [src/engine/ecs/system/layout/mod.rs](/home/rei/_/cat-engine/src/engine/ecs/system/layout/mod.rs:103).

However, the engine's width model is not CSS-like in the way the panel authoring assumes. Explicit child widths are preserved rather than clamped to the containing panel width. The existing investigation note in [docs/bugs/layoutroot-available-width-does-not-constrain-explicit-panel-widths.md](/home/rei/_/cat-engine/docs/bugs/layoutroot-available-width-does-not-constrain-explicit-panel-widths.md:1) already documents this behavior.

The relevant consequence is:

- `available_width` behaves more like a measurement and wrapping budget
- it does not reliably act as a hard constraint on visible descendant width
- explicit-width inline-block children can therefore still visually exceed or escape the budget the author expects the panel to enforce

This is the main reason the authored split math can still produce visible overlap.

### 3. The inspector width constants are out of sync between MMS and Rust

The authored panel shell was widened for the split view:

- [assets/components/panels.mms](/home/rei/_/cat-engine/assets/components/panels.mms:265)
  `INSPECTOR_PANEL_WIDTH_GU = 44.0`

But the stopgap Rust adapter still carries the old inspector width:

- [src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs](/home/rei/_/cat-engine/src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs:25)
  `const INSPECTOR_PANEL_WIDTH_GU: f64 = 22.0;`

This means the authored MMS shell and the Rust-side panel layout logic are no longer using the same width model. Even if the inline-block layout were otherwise sound, the runtime can still be operating with stale geometry assumptions.

### 4. The detail subtree is attached at runtime, not authored directly in the MMS body

The runtime adapter replaces the detail slot contents after the panel shell is spawned:

- [src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs](/home/rei/_/cat-engine/src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs:2630)
  `rerender_single_inspector_panel_detail(...)`
- [src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs](/home/rei/_/cat-engine/src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs:2650)
  `spawn_debug_inspector_detail_tree(...)`

That debug detail tree is currently a Rust-built block subtree with:

- `width = 100%`
- `height = 100%`
- `overflow = visible`

See [src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs](/home/rei/_/cat-engine/src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs:2668).

So the visible detail content is not coming from [assets/components/inspector_details.mms](/home/rei/_/cat-engine/assets/components/inspector_details.mms:1) right now; it is coming from a stopgap debug tree built in Rust. That matters because the bug is not purely an MMS authoring issue.

## Current conclusion

The overlap is not caused by `sidebar_slot` and `detail_slot` accidentally behaving like block items.

The more accurate explanation is:

- the authored inline-block widths are correct
- the runtime layout system does not hard-constrain explicit descendant widths the way the panel author expects
- the Rust stopgap adapter still uses the old `22.0` inspector width while the MMS shell uses `44.0`
- the actual detail content is injected by a Rust-built debug subtree, not by the authored MMS detail component

## Likely fix order

1. Sync the Rust-side inspector width constant with the MMS shell width.
2. Re-verify whether the overlap persists once both sides agree on the panel width.
3. If it still persists, fix the layout model or overflow behavior so explicit-width descendants cannot visually escape the intended panel width.
4. Replace the Rust debug detail subtree with the authored MMS detail component once the shell layout is stable.

## Relevant files

- [assets/components/panels.mms](/home/rei/_/cat-engine/assets/components/panels.mms:265)
- [assets/components/inspector_details.mms](/home/rei/_/cat-engine/assets/components/inspector_details.mms:1)
- [src/engine/ecs/system/layout/mod.rs](/home/rei/_/cat-engine/src/engine/ecs/system/layout/mod.rs:103)
- [src/engine/ecs/system/layout/inline.rs](/home/rei/_/cat-engine/src/engine/ecs/system/layout/inline.rs:25)
- [src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs](/home/rei/_/cat-engine/src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs:25)
- [docs/bugs/layoutroot-available-width-does-not-constrain-explicit-panel-widths.md](/home/rei/_/cat-engine/docs/bugs/layoutroot-available-width-does-not-constrain-explicit-panel-widths.md:1)
