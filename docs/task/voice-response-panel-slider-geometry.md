# Voice-response panel slider geometry and text sizing

Status: code correction applied; XR visual verification pending.

## Observation in Rei(mu)

The two high-pass rows render with oversized, overlapping text. Across all
five controls, the visible tracks appear wider than the panel, and the thumbs
can travel past the track ends. Row alignment differs between the high-pass
controls and the three mouth-response controls.

## Cause and correction to verify

`info_panel` lays out rows in glyph units and uses `unit_scale = 0.08` world
units per glyph unit. `Slider.width` is a world-space thumb-centre travel
distance. The voice panel previously passed the 13 glyph unit slot width
directly as `Slider.width(13.0)` and drew a 13 world-unit track. The filter
rows were also placed under a transform with no flex column style.

The shared panel now converts its 13 glyph unit slot to world width, uses that
width for the visible track, and subtracts thumb diameter plus an inset for
thumb travel. The filter rows have an explicit flex column container. All row
labels and readouts use the same smaller font size. These are panel-local
changes; the generic `Slider` component retains its world-unit contract.

## Acceptance on headset

1. In `examples/rei(mu).mms`, all five tracks fit their row slots and align
   consistently within the panel. Neither filter row overlaps its neighbors.
2. Every thumb remains fully over its visible track at minimum and maximum.
   Clicking or dragging to either end reaches the corresponding value.
3. Cutoff and resonance labels and readouts fit their assigned cells without
   overlap, including the largest displayed value and an accordion restore.
4. The same shared panel remains usable in the XR linear velocity example and
   both AGC examples. If the headset still shows a mismatch, measure the
   rendered world bounds of the track, thumb, slot, and panel before changing
   the generic slider implementation.
