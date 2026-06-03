# Panel clicks are blocked by selectable scene objects behind the UI

## Status

Open bug / investigation.

## Symptom

When a panel is in front of a selectable editor object, clicking the panel does not
shift panel focus and the clicked panel element does not respond either.

This shows up as a "dead click" on the panel: neither the foreground UI nor the
background editor object produces the expected panel-focus or panel-item-selection result.

## Repro

- Use an editor scene with one or more editor panels visible.
- Place a selectable scene object behind a panel so its projected screen area overlaps a
  clickable part of the panel.

Steps to reproduce:
1. Run an editor scene with the world / assets / paint panels visible.
2. Move the camera or panel placement so a selectable editor object is directly behind a
   panel click target.
3. Click the overlapping part of the panel.
4. Observe that panel focus does not change and the panel control/item does not respond.

## Expected behavior

The foreground panel should win hit resolution and receive the click.

- Panel focus should shift to the containing panel when appropriate.
- Nested panel controls or selection items should still respond normally.
- Selectable scene objects behind the panel should not interfere with the UI click.

## Actual behavior

If a selectable editor object is behind the panel, the click path appears to fail before
the panel selection / panel control handler can claim it. The panel does not focus, and
the clicked UI element does not react.

## Likely investigation targets

- `src/engine/ecs/system/raycast_system.rs`
  Check front-to-back hit ordering for overlay/layout-owned UI renderables versus scene
  objects behind them.
- `src/engine/ecs/system/gesture_system.rs`
  Check which hit becomes the emitted `Click` target and whether editor-selectable scene
  objects are stealing or invalidating the click path.
- `src/engine/ecs/system/editor_system.rs`
  Check whether editor-object selection handling is competing with panel focus / panel item
  routing for the same click.
- `src/engine/ecs/system/selection_system.rs`
  Confirm that once a UI renderable descendant is clicked, nested `Selection` / `Option`
  resolution still reaches the panel or panel item option root.
- `src/engine/ecs/system/layout/block.rs`
  Confirm layout-owned `__bg` hit surfaces are present and raycastable for the relevant
  panel shells / items.

## Questions to answer

- Is the raycast result choosing the topmost visual hit correctly when UI overlaps scene
  geometry?
- Does the click event reach the panel renderable at all, or is it being replaced by a
  selectable scene object before `SelectionSystem` sees it?
- Are there cases where the UI renderable is hit first, but later editor-selection logic
  suppresses panel focus changes?
- Should foreground UI clicks explicitly short-circuit editor scene selection when the hit
  is inside a panel subtree?

## Related

- `docs/bugs/panel-layout-selection-interaction.md`
- `docs/bugs/vtuber-desktop-scrolling-interference.md`

