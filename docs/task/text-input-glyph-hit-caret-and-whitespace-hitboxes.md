# TextInput caret placement from glyph hits, with later whitespace hitboxes

## Why

The current `TextInput` click path focuses the input but does not move the
caret from the clicked location.

For the immediate fix, we do not need full "nearest caret slot" behavior.
The simpler and more direct behavior is:

- clicking a visible glyph places the caret on that glyph
- no attempt is made yet to target whitespace, trailing line remainder, or
  empty wrapped regions

That keeps v1 aligned with the actual hit target the engine already produces:
the clicked glyph renderable.

Whitespace targeting remains useful, but it is a separate problem with a
different topology need. It should be added later as explicit hit-test-only
surfaces for `TextInput`, not folded into the first glyph-hit implementation.

## 1. Product decision

### V1 behavior

When a `TextInput` glyph is clicked:

- focus the `TextInput`
- resolve which spawned glyph renderable was clicked
- resolve that glyph's source text index
- move the caret to that glyph's index

For this first pass, there is no "before vs after glyph" split.

The intended interaction is not "choose the left or right edge of a cell".
It is simply "put the cursor on the glyph I clicked".

That means v1 can treat a clicked glyph as a single caret destination.

### V2 behavior

Later, allow caret placement in whitespace and empty inline regions by adding
transparent hit-test quads for `TextInput` text layout.

Examples:

- spaces between visible glyphs
- tabs
- line remainder to the right of wrapped text
- empty visual rows produced by explicit newlines

Those helper quads should participate in raycast / BVH only when the text is
owned by `TextInput`.

## 2. Why glyph-hit first is the right cut

### A. It matches the current signal path

Today the click event already identifies the clicked renderable. For text,
that renderable is usually the glyph quad itself.

So the missing piece is not hit testing in general. The missing piece is the
mapping from a clicked glyph renderable back to the source text index.

### B. It avoids premature whitespace topology

Whitespace is not rendered today, so supporting it requires extra helper
geometry. That is real additional topology, BVH, and lifecycle work.

It should not block the first useful behavior improvement.

### C. It keeps `TextComponent` lean

Plain `TextComponent` should stay focused on visible text rendering.

`TextInput` needs extra interaction affordances that normal display text does
not:

- caret ownership
- glyph hit metadata
- later whitespace hitboxes

Those should be added only for the `TextInput` path rather than becoming the
default cost of every `TextComponent` in the engine.

## 3. Proposed architecture

### 3.1 Text rendering stays shared

`TextSystem` should continue to own text layout and glyph spawning.

We do not want a second text layout implementation inside `TextInputSystem`.

### 3.2 TextInput adds opt-in interaction metadata

When a `TextComponent` is owned by a `TextInput`, glyphs spawned for that text
should additionally receive `TextInput`-specific metadata.

Proposed shape:

- add a small metadata component on each spawned glyph renderable, only when
  building text under `TextInput`
- store at least:
  - owning `TextInput` root id
  - owning text target id
  - source char index for that glyph

Possible component name:

- `TextInputGlyphHitComponent`

This component is intentionally not a general `TextComponent` feature.

It exists so `TextInputSystem` can answer:

- "was this clicked renderable one of my glyphs?"
- "if so, which text index does it represent?"

### 3.3 Caret semantics for v1

For glyph hit v1, a glyph maps to exactly one caret destination.

Recommended rule:

- clicking glyph at source index `i` sets `caret = i`

This matches the request to move the cursor onto the glyph, not to choose a
side of it.

If later editing behavior suggests `i + 1` would be more natural, that can be
changed separately. The important point for this task is that v1 chooses one
stable index per glyph and does not require sub-glyph hit splitting.

## 4. Topology boundaries

### 4.1 Plain TextComponent

Plain display text should keep today's behavior:

- visible glyph quads only
- no glyph-hit metadata
- no invisible whitespace helpers

### 4.2 Text owned by TextInput

Text generated for `TextInput` should gain opt-in interaction helpers:

- glyph renderables tagged with text-input-specific hit metadata
- later, optional transparent whitespace quads for hit testing

This keeps the heavier interaction topology scoped to editable text.

## 5. V2 whitespace plan

Whitespace hit support should be added as explicit helper quads, not inferred
from visible glyph neighbors.

### 5.1 Helper surface shape

For `TextInput` only, spawn additional transparent quads for hit testing over
regions that currently have no visible renderable.

These may be:

- one cell wide for ordinary spaces
- tab-width wide for tabs
- wider-than-tall rectangles for trailing row remainder on wrapped lines
- zero-visual-opacity but raycast-enabled

These helpers exist for interaction, not rendering.

They should be excluded from serialization the same way other runtime helper
topology is excluded.

### 5.2 Metadata for whitespace helpers

Whitespace helper quads should carry their own text-input-specific metadata,
for example:

- owning `TextInput`
- caret destination index
- helper kind: space / tab / line remainder / empty line

Possible component name:

- `TextInputCaretHitComponent`

At that point the click path can become uniform:

- any hit-test helper under `TextInput` resolves directly to a caret index

Visible glyph quads and invisible whitespace quads can then share the same
caret-placement pipeline if desired.

## 6. Implementation outline

### Phase 1: glyph hit caret placement

1. Detect when a `TextComponent` is being built as the owned text target of a
   `TextInput`.
2. During glyph spawn, attach `TextInput`-specific glyph-hit metadata to each
   spawned visible glyph renderable.
3. In `TextInputSystem` click handling, check whether the clicked renderable
   carries that metadata.
4. If yes, set focus and move the caret to the metadata's source index.
5. Keep current fallback behavior for non-glyph clicks: focus without changing
   caret, or whatever the current focus policy is at that point.

### Phase 2: whitespace hit helpers

1. Extend the text build path for `TextInput` to emit transparent helper quads
   for whitespace and line remainder regions.
2. Attach caret-destination metadata to those helpers.
3. Route clicks on those helpers through the same caret-placement path.
4. Verify that helper quads participate in BVH / raycasting but remain
   visually invisible.

## 7. Acceptance criteria

### Phase 1

1. Clicking a visible glyph inside a `TextInput` moves the caret to that
   glyph's mapped text index.
2. This behavior is implemented without introducing a second text layout
   algorithm in `TextInputSystem`.
3. Plain `TextComponent` outside `TextInput` does not pay for the metadata.
4. No whitespace hit support is required for this phase.

### Phase 2

1. Clicking whitespace inside a `TextInput` can place the caret.
2. Trailing wrapped-line remainder can be clicked when helper quads are
   present.
3. Those helper quads are `TextInput`-only and remain visually transparent.

## 8. Verification

### Phase 1

Add focused tests covering:

- glyph metadata is attached only for text owned by `TextInput`
- clicking a glyph renderable updates the `TextInput` caret to the stored
  source index
- plain `TextComponent` glyphs remain untagged

Run:

- `cargo test focused_text_input_mutates_backing_text --lib`
- targeted new text-input click-placement tests

### Phase 2

Add focused tests covering:

- whitespace helper quads are emitted only for `TextInput`
- whitespace helper clicks move the caret to the encoded destination index
- helper quads are invisible but raycastable

## 9. Non-goals for this task

- full nearest-caret selection from arbitrary local `(x, y)`
- choosing left vs right side within a glyph cell
- adding whitespace hitboxes to all `TextComponent`s
- proportional-font caret metrics

Those can be revisited later if the editing model expands beyond the current
cell-based monospace text path.