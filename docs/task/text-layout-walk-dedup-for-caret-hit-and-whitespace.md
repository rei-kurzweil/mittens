# Text layout walk de-dup for caret hit testing and whitespace helpers

## Why

The upcoming `TextInput` caret-hit work can be implemented without refactoring
the existing text layout code first.

That is the right order for delivery, but it also highlights an obvious follow-
up: the engine already performs the same text walk in multiple places, and the
next `TextInput` features will increase that duplication further.

Today the codebase already has at least these overlapping layout consumers:

- glyph spawning for visible text
- text measurement
- caret index -> local position

Planned `TextInput` work adds more likely consumers:

- clicked glyph -> source text index metadata during spawn
- whitespace helper quad generation
- possibly caret-slot hit helpers later

If each of those keeps its own copy of wrap/newline/tab/word-wrap traversal,
behavior drift becomes likely.

This task is a note for the likely refactor seam, not a prerequisite for the
first glyph-hit implementation.

## 1. Current duplication seam

The strongest seam is in [src/engine/ecs/system/text_system.rs](../../src/engine/ecs/system/text_system.rs).

Most of the shared logic lives around:

- `WordWrapState`
- `compute_wrap_allowed_after(...)`
- `compute_word_run_len(...)`
- `TextSystem::register_text(...)`
- `TextSystem::measure(...)`
- `TextSystem::caret_local_position(...)`

These paths all replay the same conceptual algorithm:

1. turn text into `chars`
2. compute wrap opportunities
3. compute word-run lookahead
4. iterate characters in source order
5. handle newline / wrap / space / tab / visible glyph advance
6. produce some output for each visited position

The differences are mostly in the output side effects:

- `register_text(...)` spawns topology for visible glyphs
- `measure(...)` accumulates bounds only
- `caret_local_position(...)` stops at a target caret index and returns one
  local point

That is the likely extraction boundary.

## 2. Most likely refactor shape

The probable cleanup is to extract a shared text-layout walk helper that owns
the traversal and lets callers plug in behavior.

This does not need to be generic-framework-heavy. A plain internal helper is
enough.

Two realistic shapes:

### A. Callback walker

Add an internal function in [src/engine/ecs/system/text_system.rs](../../src/engine/ecs/system/text_system.rs) that:

- initializes `chars`, wrap opportunities, and `WordWrapState`
- walks the text once in source order
- emits layout events to a caller-provided closure

Possible seam names:

- `walk_text_layout(...)`
- `for_each_text_slot(...)`
- `walk_text_cells(...)`

Likely event kinds:

- newline
- wrapped line break
- space advance
- tab advance
- visible glyph with source index and local position
- maybe caret slot before/after advance if later needed

This is the most flexible seam if `TextInput` later adds whitespace helper
quads.

### B. Precomputed layout entries

Add a small internal data model that records layout entries, then have callers
consume that.

Possible shapes:

- `Vec<TextLayoutEntry>`
- `TextLayoutRun`
- `TextLayoutSnapshot`

Possible entry fields:

- source char index
- char kind: glyph / space / tab / newline
- row / col
- local x / y
- wrap-caused line transition

This is easier to inspect in tests, but may be more allocation-heavy than the
walker approach if built every time.

For this engine, the callback-walker shape is the more likely first refactor.

## 3. Methods most likely to move behind that seam

### In [src/engine/ecs/system/text_system.rs](../../src/engine/ecs/system/text_system.rs)

Most likely direct callers or wrappers:

- `TextSystem::register_text(...)`
- `TextSystem::measure(...)`
- `TextSystem::caret_local_position(...)`

Most likely shared helpers that stay near the seam:

- `WordWrapState::newline(...)`
- `WordWrapState::apply_wrap_if_needed(...)`
- `WordWrapState::advance_space(...)`
- `WordWrapState::advance_tab(...)`
- `WordWrapState::advance_glyph(...)`
- `WordWrapState::apply_word_wrap_lookahead(...)`
- `WordWrapState::cursor_pos(...)`
- `compute_wrap_allowed_after(...)`
- `compute_word_run_len(...)`
- `TextSystem::handle_word_wrap_for(...)`

Potential cleanup within that file:

- `handle_word_wrap_for(...)` is already part of the seam, but today it still
  leaves some caller-specific control flow around it
- `caret_local_position(...)` and `measure(...)` duplicate the same per-char
  cases explicitly rather than sharing a single traversal helper

## 4. Files most likely involved later

### Primary file

- [src/engine/ecs/system/text_system.rs](../../src/engine/ecs/system/text_system.rs)

This is where the shared traversal should probably live, because the layout
rules belong to text rendering, not to `TextInputSystem`.

### First downstream consumer

- [src/engine/ecs/system/text_input_system.rs](../../src/engine/ecs/system/text_input_system.rs)

This file should consume the seam, not define it.

Likely future uses here:

- glyph-hit metadata hookup during owned text build
- whitespace helper generation for `TextInput`
- maybe caret helper sync if later features need richer caret slot data

### Possible component files

- [src/engine/ecs/component/text.rs](../../src/engine/ecs/component/text.rs)
- a future `TextInput`-only metadata component file under
  [src/engine/ecs/component](../../src/engine/ecs/component)

Those are not where the traversal logic should live, but they are likely to be
adjacent to the feature work that depends on it.

### Possible tests

- tests already colocated in [src/engine/ecs/system/text_system.rs](../../src/engine/ecs/system/text_system.rs)
- tests in [src/engine/ecs/system/text_input_system.rs](../../src/engine/ecs/system/text_input_system.rs)

If a walker/snapshot seam is added, it is worth adding narrow tests for the
shared layout traversal directly so feature tests do not become the only guard
against wrap drift.

## 5. What should stay out of the seam

The seam should own traversal and local text-layout facts.

It should not directly own:

- ECS component creation
- `World` mutation
- `RenderableComponent` setup
- `TextInput` focus policy
- click routing

Those belong in the systems that consume the layout walk.

That boundary matters because otherwise the extracted helper turns into a
second, harder-to-test mini text system.

## 6. Suggested incremental order

1. Implement glyph-hit caret placement first with minimal changes.
2. Once that works, extract the shared traversal helper inside
   [src/engine/ecs/system/text_system.rs](../../src/engine/ecs/system/text_system.rs).
3. Migrate `measure(...)` and `caret_local_position(...)` onto the new seam.
4. If whitespace helper quads are added, build them on that seam rather than
   copying traversal logic again.

This keeps the first feature grounded and prevents the refactor from getting
speculative.

## 7. Acceptance criteria for the later refactor

1. `measure(...)` and `caret_local_position(...)` no longer duplicate the full
   per-character traversal.
2. Visible glyph spawning continues to produce the same layout as before.
3. Word wrap, tabs, spaces, and explicit newlines stay behaviorally identical.
4. `TextInput`-specific whitespace helpers, if added later, reuse the shared
   traversal rather than introducing a third copy.

## 8. Non-goals

- changing text layout behavior as part of the refactor
- moving text layout ownership out of `TextSystem`
- introducing a public API unless a real caller outside `TextSystem` needs it

This is a maintenance seam, not a behavior redesign.