# TextInput editing can become very slow on long buffers

Date: 2026-06-02

## Symptom

Editing a `TextInput` with many characters can feel very slow.

The new [examples/text-editor.mms](../../examples/text-editor.mms) is a good repro surface because it gives the input a long, scrollable body of text. Once the buffer gets large enough, insertions and deletions appear to do too much work for a single keystroke.

## What seems expensive

The likely hot path is not the text mutation itself. It is the amount of layout and tree work that happens after the edit.

The current suspicion is:

- text after the insertion or deletion point is being rebuilt or re-laid out from scratch
- layout only really needs to be recomputed for glyphs after the edit point
- we may not need to recreate every glyph component tree on each edit

That means the current edit path may be doing a full text-tree rebuild when a smaller incremental update would be enough.

## Why this is a bug

For short strings this is fine. For long editable text, it makes editing scale badly with buffer size.

The expected cost model should be closer to:

- mutate the backing string
- insert or delete the affected glyph subtree
- re-run layout from the edit point forward

not:

- rebuild the whole glyph tree for the entire string
- recompute layout for glyphs before the edit point that did not change

## Current code to inspect

Relevant paths:

- [src/engine/ecs/system/text_input_system.rs](../../src/engine/ecs/system/text_input_system.rs)
- [src/engine/ecs/system/text_system.rs](../../src/engine/ecs/system/text_system.rs)
- [examples/text-editor.mms](../../examples/text-editor.mms)

The text system already has the layout logic for glyph placement, including `layout_position_for_index(...)` and the underlying text walk used by `register_text(...)`.

## Likely fix direction

The edit path should probably become incremental.

Possible direction:

1. confirm whether edits currently rebuild the entire text subtree
2. if so, change the text input update path to insert or delete only the affected glyph component tree
3. re-run layout only for the affected suffix, or let `TextSystem` / layout system recompute positions from the edit point forward

The key point is that layout after an edit only needs to affect glyphs that come after the insertion or deletion point.

## Open question

We should check whether edits are actually reconstructing all glyph components today, or whether the slowness is mostly from layout recomputation.

If the tree is being rebuilt, that is the first thing to stop.
If not, the next target is incremental layout after the edit point.

## Acceptance criteria

- typing into a long `TextInput` no longer scales like full-buffer rebuild on every keystroke
- insertion and deletion only touch the necessary glyph subtree
- layout work is limited to the changed suffix, not the entire buffer
- the text-editor repro remains responsive with long input

