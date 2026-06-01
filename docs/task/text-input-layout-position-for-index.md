# Shared layout position helper for Text and TextInput

## Motivation

`TextInput` currently derives caret placement by replaying text layout logic in a way that is separate from the core `Text` layout path.
That duplication is already fragile and the wrapped-line bug shows it clearly: the caret index-to-local-position calculation is a shared concern, but it lives in a consumer-specific path today.

A dedicated helper like `layout_position_for_index(...)` would:

- centralize the text traversal rules for wrap/newline/tab/word-wrap
- keep `Text` glyph placement and `TextInput` caret placement consistent
- make edge cases like left-edge wrapped-line cursor placement easier to reason about
- reduce duplicated algorithm drift as TextInput adds more layout-dependent features

## Proposed change

Add a shared helper in `src/engine/ecs/system/text_system.rs` that computes the layout position for a source text index.

### Candidate API

- `TextSystem::layout_position_for_index(
    text: &str,
    index: usize,
    wrap_at: usize,
    word_wrap: bool,
    word_wrap_tokens: &[String],
    font_size: f32,
) -> (f32, f32)`

The helper should:

- walk the text in source order
- apply newline, wrap, space, tab, and glyph advance correctly
- use the same wrap opportunity logic as `TextSystem::register_text(...)`
- return the local caret position for the requested index

## How this would be used

### `TextSystem`

- `TextSystem::caret_local_position(...)` should become a thin wrapper around the new helper.
- `TextSystem::measure(...)` should be able to reuse the same traversal data if desired.

### `TextInputSystem`

- `TextInputSystem::sync_caret_bg(...)` should call the shared helper rather than independently walking text.
- That ensures caret placement for wrapped lines is consistent with glyph layout and avoids the current left-edge bug.

## Why `layout_position_for_index(...)` is a better abstraction

The helper encapsulates the notion of: “given the text and layout rules, where should the cursor sit for this source index?”

That is the exact shared problem for both:

- visible `Text` glyph layout
- `TextInput` caret placement

By contrast, `caret_local_position(...)` is too narrowly named and leaves room for duplicate traversal logic.

## Implementation notes

The shared helper should likely reuse or be built on top of the existing `WordWrapState` / `compute_wrap_allowed_after(...)` / `compute_word_run_len(...)` machinery.

The key bug to close is that a caret positioned before a wrapped glyph must still evaluate the wrap decision for that glyph, even though the glyph itself is not advanced yet.

A practical helper shape is:

- compute `chars`, `wrap_allowed_after`, and `word_run_len`
- walk the text prefix up to `index`
- if `index` is before a visible glyph, apply the same pre-glyph wrap logic used by `register_text(...)` for that character
- then return the current cursor position

That ensures the caret slot state is driven by the same wrap/newline logic as glyph layout.

Potential outputs:

- text-local `(x, y)` coordinates
- or a richer row/col position if future caret slot helpers need it

For now, `(x, y)` is sufficient, but the function should be designed so it can evolve without duplicating the core traversal.

## Acceptance criteria

1. `TextInputSystem` and `TextSystem` both rely on the same text-index-to-position helper.
2. Wrapped-line caret placement behaves consistently for all glyph indices.
3. The helper preserves existing wrap/newline/tab semantics.
4. The bug where a leftmost wrapped glyph selects the previous-line whitespace is fixed.

## Notes

This is a good follow-up refactor once the current caret position bug is understood and reproducible.
The actual first change can remain small: introduce `layout_position_for_index(...)` and migrate `caret_local_position(...)` before touching any higher-level `TextInput` logic.
