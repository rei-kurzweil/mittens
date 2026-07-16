# Pose panel captured pose text overlap and slot routing

## Status

Investigation note.

No implementation yet.

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

The pose panel currently manually clears and adds dynamic children under:

```text
#content_area
```

That differs from the newer grid and world panel direction, where panel content
is rendered through `DataRendererSystem` into a stable slot and layout/routing is
allowed to place children through the authored panel structure.

This mismatch is suspicious because scroll content measurement, style
inheritance, and layout-owned child routing may not be following the same path as
the grid/world panel rows.

## Proposed investigation

Compare the pose panel row and header path against the grid/world panel row
path.

Specific checks:

- verify font size inherited by `pose_section_header` and `pose_row`
- verify text wrapping behavior for captured pose labels and values
- verify row height and padding after dynamic insertion
- verify scroll content measurement for `content_area`
- compare row/header style inheritance with grid/world rows
- inspect whether direct mutation under `content_area` bypasses a slot or
  routing rule that the other panels rely on

## Possible fix direction

Decide whether pose capture results should render into `#content_slot` through
`DataRendererSystem` instead of directly mutating `content_area`.

If the issue is only row styling, a smaller local style fix may be enough. If
the issue is caused by bypassing the stable panel slot/routing model, the pose
panel should be migrated to the same renderer-owned projection path used by the
grid/world panel content.

## Assumptions

- pose row content itself is being created successfully
- the active bug is the rendered layout of captured pose content
- the likely failure area is style inheritance, text measurement, row sizing, or
  slot/routing mismatch
