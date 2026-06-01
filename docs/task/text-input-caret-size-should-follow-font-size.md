# TextInput caret size should follow text font size

## Why

`TextInput` now has a visible caret helper, but its geometry is effectively
hard-coded to a 1x1 cell scale.

That means caret placement respects text layout, but caret size does not yet
track the owned text's `font_size`.

Today this creates a mismatch:

- glyph positions scale with `TextComponent.font_size`
- caret position scales with `TextSystem::caret_local_position(...)`
- caret helper size does not appear to scale from the same source of truth

So a text input with larger or smaller text can end up with a caret that lands
in the right place but has the wrong visual size.

## 1. Problem statement

The current caret helper topology in
[src/engine/ecs/system/text_input_system.rs](../../src/engine/ecs/system/text_input_system.rs)
creates the caret background transform with a unit scale.

The relevant seam is the caret helper creation/sync path:

- `ensure_caret_bg(...)`
- `sync_caret_bg(...)`
- `caret_bg_sync_state(...)`

The same file already reads the owned text's `font_size` when computing caret
position through `TextSystem::caret_local_position(...)`, so the missing piece
is not data availability. It is simply that caret geometry is not yet derived
from that same text state.

## 2. Desired behavior

The caret helper should size itself from the effective `TextComponent.font_size`
of the `TextInput`'s owned text target.

That means:

- larger font size -> larger caret helper
- smaller font size -> smaller caret helper
- the same text state should drive both caret position and caret size

The important contract is visual consistency, not a specific authored unit.

## 3. Proposed implementation direction

### 3.1 Reuse the existing text target lookup

`TextInputSystem` already resolves the owned text target and already reads the
`TextComponent` from it during caret sync.

The most direct implementation is:

1. resolve the text target
2. read `text.font_size`
3. apply that scale to the caret helper transform

This should happen in the same place that already syncs caret position, so the
caret's transform remains internally consistent.

### 3.2 Preferred seam

The most likely seam is to extend `caret_bg_sync_state(...)` so it returns the
size information needed to fully sync the helper, not just `(x, y)` and
visibility.

For example, it could later return something conceptually like:

- local x
- local y
- font-scaled caret width/height or transform scale
- visible / focused state

That keeps all text-derived caret facts in one place.

### 3.3 Avoid introducing a separate caret sizing policy if possible

The first implementation should probably use `font_size` directly rather than
introducing another authored knob.

If a later product decision wants a thinner insertion bar or a different aspect
ratio, that can still be expressed as a simple multiplier on top of
`font_size`.

Example later policy:

- width = `font_size * kx`
- height = `font_size * ky`

But the base unit should still come from the text's font size.

## 4. Likely files involved

Primary file:

- [src/engine/ecs/system/text_input_system.rs](../../src/engine/ecs/system/text_input_system.rs)

Reference / shared text state:

- [src/engine/ecs/system/text_system.rs](../../src/engine/ecs/system/text_system.rs)
- [src/engine/ecs/component/text.rs](../../src/engine/ecs/component/text.rs)

Tests will most likely live alongside existing text input tests in:

- [src/engine/ecs/system/text_input_system.rs](../../src/engine/ecs/system/text_input_system.rs)

## 5. Acceptance criteria

1. The `TextInput` caret helper size changes when the owned text's `font_size`
   changes.
2. Caret position and caret size are derived from the same owned text state.
3. Plain `TextComponent` outside `TextInput` is unaffected.
4. No separate text layout algorithm is introduced just for caret sizing.

## 6. Verification

Add focused tests covering:

- default-font text input caret size
- non-default `font_size` text input caret size
- caret continues to land at the correct local position after the size change

Run:

- targeted `TextInputSystem` tests
- `cargo check --lib`

## 7. Non-goals

- whitespace hitboxes
- arbitrary nearest-caret placement
- author-configurable caret thickness in this task

This task is only about making the existing caret helper match the scale of the
text it belongs to.