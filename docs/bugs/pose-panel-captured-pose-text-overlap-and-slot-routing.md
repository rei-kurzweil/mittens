# Pose panel captured pose text overlap and slot routing

## Status

Implemented; awaiting visual verification.

The pose panel now uses the same authored viewport, stable content slot, and
`DataRendererSystem` projection pattern as the grid and world panels.

## Observed topology

After capturing a pose, the pose panel content exists in the live component tree.

The observed path is:

```text
pose_capture_panel_root/content_slot/__scroll/__scroll_track/content_area
```

Dynamic children are present under `content_area`, including:

- `pose_section_header`
- `pose_row`

That means the captured pose data and row/header components are being created.
The remaining bug appears to be rendering and layout behavior, not missing data.

## Visible symptom

The captured pose UI shows large or overlapping text even though the content is
present and inspectable in the component tree.

The panel therefore fails visually after capture while still having the expected
runtime topology below the scroll content area.

## Current implementation mismatch

Before the fix, the pose panel manually cleared and added dynamic children
under:

```text
#content_area
```

That differed from the newer grid and world panel direction, where panel
content is rendered through `DataRendererSystem` into a stable slot and
layout/routing is allowed to place children through the authored panel
structure.

This mismatch is suspicious because scroll content measurement, style
inheritance, and layout-owned child routing may not be following the same path as
the grid/world panel rows.

## Implemented change

The authored panel topology is now:

```text
pose_capture_panel_root
`-- pose_panel_content_area
    `-- content_slot
        `-- data_renderer_list_*
            |-- pose_section_header
            `-- pose_row
```

`pose_panel_content_area` owns the fixed-height scroll viewport.
`#content_slot` is the stable renderer mount beneath it. Pose headers and rows
are flattened into `UiItem` values and rendered through a Rust
`ItemRendererSpec`.

The pose row renderer preserves the existing click payload:

- `target_component` identifies the captured pose
- `pose_target` identifies the owning `PoseCaptureComponent`

This keeps pose application behavior unchanged while moving subtree lifecycle,
attachment, initialization, and layout invalidation under `DataRendererSystem`.

## Verification

- `cargo check` passes
- focused Rust formatting and diff whitespace checks pass
- visual verification after capturing one or more poses is still required

## Assumptions

- pose row content itself is being created successfully
- the active bug is the rendered layout of captured pose content
- the likely failure area is style inheritance, text measurement, row sizing, or
  slot/routing mismatch
