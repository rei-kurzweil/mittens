# Flex item padding does not expand background quads

Date: 2026-07-03

Status: observed / needs reduction

## Summary

On flex items, adding padding can improve the apparent text position, but the background quad does
not expand to match the padded content box.

The visible result is:

- text appears better aligned once padding is added
- but the background fill remains too small or sized to the unpadded bounds
- so the text looks visually offset relative to the colored row background

This appears to be specific to the flex layout path, or at least much easier to trigger there than
with non-flex layout.

This does **not** currently appear to be a broad regression across the existing editor panels.

## Observed in

- [`examples/table-field-reassign.mms`](/home/rei/_/cat-engine/examples/table-field-reassign.mms:1)

The clearest repro shape was a flex-column table made of flex-row items with alternating row
background colors, where each cell/row needed padding to make the text sit correctly.

This example is currently the best visible repro for the issue.

In particular, the `table field reassignment` text near the top of the demo is a good marker:

- its top and left placement appear to respect padding
- but the text is pushed down toward the bottom edge of the colored background
- some of the glyphs appear to extend past the bottom of the background quad

That makes the padding effect easy to see: the text position changes, but the visible background
box does not fully track the padded bounds.

## Scope notes

Most existing editor panels do not visibly exhibit this exact behavior.

Current observations:

- world/inspector/assets/paint panels do not appear to show the same obvious padded-flex-item
  background mismatch
- the editor settings panel may have a related but narrower issue
- in editor settings, the `Select + Cursor` label, which wraps to two lines, looks a bit off in
  terms of visual centering within its background quad
- the editor settings panel also appears slightly too short to comfortably fit all of its items

So this should be treated as a somewhat isolated flex/layout/background interaction rather than a
repo-wide panel rendering failure.

## Symptoms

- row text sits a bit low without extra padding
- adding padding on flex rows/cells improves text placement
- the row/cell background quad does not grow to the padded size
- this makes the padded text look like it is escaping or hanging below the intended colored block

## Expected

For a flex item with background color and padding:

- layout size should include the padding
- the generated background quad should match the padded box
- text should render inside that padded box

## Actual

The layout/text relationship and the background-quad sizing appear to disagree for padded flex
items.

## Initial suspicion

Likely one of:

- flex measurement is computing padded size differently from background quad generation
- background quad generation is using content bounds instead of padded layout bounds
- text baseline/offset and background box sizing are being derived from different box models on the
  flex path

## Why this matters

This shows up immediately in table/list UIs and could affect future editor-style lists even if the
current editor panels are mostly okay:

- table-like data views
- future flex-based editor rows with explicit padding and alternating backgrounds
- narrow cases like wrapped multi-line labels in settings-style panels

It makes it harder to visually center text in rows without breaking the visible block sizing.

## Suggested next step

Make a smaller dedicated repro with:

1. one `LayoutRoot`
2. one flex column
3. two flex rows with background colors
4. text children
5. toggle padding on the row and/or cell wrappers

Then compare:

- computed layout bounds
- background quad bounds
- text transform / baseline placement

## Related

- [world-panel-row-text-appears-low-inside-padding.md](/home/rei/_/cat-engine/docs/bugs/world-panel-row-text-appears-low-inside-padding.md:1)
