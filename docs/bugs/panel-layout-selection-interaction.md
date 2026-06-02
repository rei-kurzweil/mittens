# Panel layout / selection interaction checklist

This issue tracks the remaining editor panel problems discovered during runtime testing.

- [ ] Reduce world/inspector panel row depth so row items do not protrude too far along Z.
  - Verify whether `Style::background_z` can override the automatic stacking distance.
  - Confirm row `__bg` quads are placed at a shallower local Z while preserving proper layer order.

- [ ] Fix `world_panel` row click side effects leaking into the `assets` panel.
  - Clicking items in the world panel should only affect the inspector panel and world panel state.
  - Determine why the assets panel title style/size is changing on world panel clicks.

- [ ] Resolve `assets` + `paint` panel horizontal overlap.
  - Confirm the shared `LayoutRoot` is being marked dirty and recomputed after panel creation.
  - Check whether panel shell sizing, available width, or layout children are misconfigured.

- [ ] Add regression coverage for editor panel layout and interaction boundaries.
  - Include tests for world panel click only updating inspector content.
  - Add a layout-root validation test for no horizontal overlap between sibling panels.
