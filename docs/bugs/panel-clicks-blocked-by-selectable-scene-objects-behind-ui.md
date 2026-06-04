# Panel clicks are blocked by selectable scene objects behind the UI

## Status

Open bug / investigation.

## Symptom

When a panel is in front of a selectable editor object, clicking the panel does not
consistently resolve as a panel-shell click.

Inner UI controls can still be interactive even when scene geometry overlaps the panel,
but panel-focus behavior differs by panel and control type:

- `world_panel`: clicking list items / options still focuses the panel, and clicking the
  `TextInput` still focuses the panel and places/selects the clicked glyph, even with a
  ground plane / prism / wall behind the panel
- `inspector`: clicking a component row such as `renderable` only focuses/selects the
  panel if there is no object behind the clicked part of the panel
- `paint_panel`: clicking an inner `Select` / option still selects that option, but does
  not also focus/select the outer panel when scene geometry is behind it
- `assets`: this currently has no clickable inner UI elements, so it can only be selected
  when the clicked part of the panel does not have scene geometry behind it

## Repro

- Use an editor scene with one or more editor panels visible.
- Place a selectable scene object behind a panel so its projected screen area overlaps a
  clickable part of the panel. A ground plane, prism, or wall behind the panel is enough.

Steps to reproduce:
1. Run an editor scene with the world / assets / paint panels visible.
2. Move the camera or panel placement so a selectable editor object is directly behind a
   panel click target.
3. Click a `world_panel` list item / option where scene geometry overlaps the panel.
4. Observe that the world panel still becomes focused/selected.
5. Click the `world_panel` `TextInput` where scene geometry overlaps the panel.
6. Observe that the text input still takes focus on that panel and selects the clicked
   glyph.
7. Click an `inspector` component row such as `renderable` where scene geometry overlaps
   the panel.
8. Observe that the Inspector panel only focuses/selects correctly when there is no object
   behind the clicked part of the panel.
9. Click a `paint_panel` option where scene geometry overlaps the panel.
10. Observe that the option is selected, but the Paint panel itself does not become the
    selected/focused panel.
11. Click the overlapping shell area of the `assets` panel.
12. Observe that it only selects correctly when there is no object behind the clicked part
    of the panel.

## Expected behavior

The foreground panel should win hit resolution and receive the click.

- Panel focus should shift to the containing panel when appropriate.
- Nested panel controls or selection items should still respond normally.
- If an inner interactive control is clicked, that interaction should not depend on
  whether selectable scene geometry is behind the panel.
- If panel focus is supposed to accompany an inner interaction, that should also work
  consistently regardless of overlapped scene geometry.
- Selectable scene objects behind the panel should not interfere with the UI click.

## Actual behavior

The behavior is not a pure dead click. Inner UI interaction can still succeed, but outer
panel selection/focus becomes inconsistent when overlapping scene geometry exists behind
the panel.

- `world_panel` `TextInput` works through the overlap: the panel becomes focused and the
- `world_panel` inner list items / options also work through the overlap and still focus
  the panel
- `world_panel` `TextInput` works through the overlap: the panel becomes focused and the
  clicked glyph is selected
- `inspector` component rows do not work the same way: clicking rows such as `renderable`
  only focuses/selects the panel when there is no object behind that part of the panel
- `paint_panel` inner option selection works through the overlap, but the outer panel does
  not also become selected/focused
- `assets` currently exposes only shell-level click behavior, so in overlapping regions it
  effectively cannot be selected via the panel surface

Additional observation:

- this does not look limited to a bad ground-box BVH shape or oversized bounds
- moving the background wall farther away still lets it absorb the ray hit and/or click
  that the panel should have received, as long as the wall remains behind that panel in
  screen space
- the interference stops when the object is no longer behind the panel's clicked region

## Likely investigation targets

- `src/engine/ecs/system/raycast_system.rs`
  Check front-to-back hit ordering for overlay/layout-owned UI renderables versus scene
  objects behind them.
- `src/engine/ecs/system/bvh_system.rs`
  Verify scene-object BVH bounds are sane, but treat this as secondary unless inspection
  shows obviously incorrect extents; current repro suggests the larger issue is not just a
  single mis-sized ground-box volume.
- `src/engine/ecs/system/gesture_system.rs`
  Check which hit becomes the emitted `Click` target and whether editor-selectable scene
  objects are stealing or invalidating the click path.
- `src/engine/ecs/system/editor_system.rs`
  Check whether editor-object selection handling is competing with panel focus / panel item
  routing for the same click.
- `src/engine/ecs/system/selection_system.rs`
  Confirm that once a UI renderable descendant is clicked, nested `Selection` / `Option`
  resolution still reaches the panel or panel item option root, and confirm why
  `world_panel` inner items and `TextInput` can focus the panel while `inspector` rows and
  `paint_panel` option clicks do not.
- `src/engine/ecs/system/layout/block.rs`
  Confirm layout-owned `__bg` hit surfaces are present and raycastable for the relevant
  panel shells / items.
- `src/engine/ecs/system/text_input_system.rs`
  Check whether text-input focus is using a different routing or focus path than panel
  shell / option selection, since `world_panel` text input remains interactive through the
  overlap.

## Questions to answer

- Is the raycast result choosing the topmost visual hit correctly when UI overlaps scene
  geometry?
- Is the wrong target being chosen because of hit ordering / routing, even when the object
  behind the panel is moved farther back in world space?
- Does the click event reach the panel renderable at all, or is it being replaced by a
  selectable scene object before `SelectionSystem` sees it?
- Why do `world_panel` inner list items and text-input focus survive the overlap while
  `inspector` rows do not and `paint_panel` option clicks do not also promote/focus the
  panel?
- Are there cases where the UI renderable is hit first, the inner control handles the
  click, and later editor-selection logic suppresses or skips panel focus changes?
- Should foreground UI clicks explicitly short-circuit editor scene selection when the hit
  is inside a panel subtree?

## Related

- `docs/bugs/panel-layout-selection-interaction.md`
- `docs/bugs/vtuber-desktop-scrolling-interference.md`
