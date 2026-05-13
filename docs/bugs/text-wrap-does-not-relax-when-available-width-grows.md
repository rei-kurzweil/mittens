# Text wrap does not relax when LayoutRoot `available_width` grows

## Status

Open.

## Symptom

Once a Text in an auto-width inline-block has wrapped at one
`LayoutRoot.available_width`, it stays wrapped at that column count
forever:

- Shrinking `available_width` further does **not** re-wrap to a tighter
  column count.
- Growing `available_width` back up does **not** un-wrap the text.

Reproduce in `examples/percentage-layout` (or `examples/padding-demo`)
with the `-` / `+` buttons: click `-` until lines wrap, then `+` back to
the starting width. The wrapped lines stay wrapped.

## Expected behavior

Text wrap is a function of the current available content width. When
`LayoutRoot.available_width` changes, the next layout pass should
re-derive `wrap_at` from the new available width and rebuild line breaks
accordingly. Wrap should both tighten (more breaks at smaller widths)
and relax (fewer/zero breaks at larger widths).

## Actual behavior

After the first wrap, the wrap column appears to be cached / latched.
Subsequent measure passes do not re-wrap to a different column even
though `intrinsic_block_width` / `text_intrinsic_width` are now being
called with a different `avail_content_w_gu`.

## Likely cause (to investigate)

- `TextComponent` may carry persistent `wrap_at` / glyph-layout state
  that is set once on first measurement and never invalidated when the
  enclosing item's measured width changes.
- Or the inline cursor's `is_auto_width` remeasure path is computing the
  new available width but the text glyph-layout produced by
  `TextSystem::register_text` is using stale wrap state from a prior
  tick.

Either way, the fix is to make text glyph layout (line breaks) a pure
function of `(content, wrap_at_for_this_pass)` with no carry-over.

## Affected examples

- `examples/percentage-layout.mms`
- `examples/padding-demo.mms`

## Related

- `src/engine/ecs/system/layout/measure.rs` —
  `text_intrinsic_width`, `intrinsic_block_width`
- `src/engine/ecs/system/layout/inline.rs` — `is_auto_width` remeasure
  path
- `TextSystem::register_text` — where the on-screen glyph layout /
  line breaks are produced from `TextComponent`
