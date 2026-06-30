# Task: Implement `display("flex")` And `flex_direction(...)` MVP

Date: 2026-06-29

Status: active task

## Why this task exists

MMS authoring already exposes flex-style layout controls:

- `Style.display("flex")`
- `Style.flex_direction("row" | "column" | ...)`
- `Style.justify_content(...)`
- `Style.align_items(...)`

But the layout runtime does not actually execute a flex formatting context yet.

That mismatch is now surfacing in real authored UI work like
`examples/mms-tables.mms`, where a scene that reads like a horizontal flex row
still lays out like a block column.

## Current evidence

### 1. Parser / style patching already accept flex properties

In `src/meow_meow/component_registry.rs`:

- `display("flex")` maps to `Display::Flex`
- `flex_direction(...)` maps to `FlexDirection`
- `justify_content(...)` maps to `JustifyContent`
- `align_items(...)` maps to `AlignItems`

So the authoring surface already claims this feature exists.

### 2. The flex layout module is explicitly stubbed

In `src/engine/ecs/system/layout/flex.rs`:

- the module header says flex handles row/column containers
- the body says:
  - "Not yet implemented"
  - it is currently a `TODO`

### 3. Layout dispatch never routes to flex

In `src/engine/ecs/system/layout/mod.rs`:

- the runtime only distinguishes:
  - all-inline/inline-block children => `inline::layout(...)`
  - everything else => `block::layout(...)`
- `Display::Flex` is not dispatched into `flex::layout(...)`

So today:

- authored `display("flex")` is parsed
- style state stores flex metadata
- layout ignores that metadata
- authored flex scenes silently fall back to block behavior

## Immediate goal

Make `display("flex")` actually mean "use flex layout" in the layout system.

Minimum first win:

- `display("flex")`
- `flex_direction("row")`
- `flex_direction("column")`

That is the smallest honest implementation that makes authored MMS predictable.

## MVP scope

### Required for MVP

- dispatch `Display::Flex` into a real flex layout path
- implement container axis switching:
  - row => horizontal main axis
  - column => vertical main axis
- respect authored child order
- position children using measured sizes instead of block fallback

### Strongly consider including in the same MVP

These may be needed immediately for authored scenes to feel usable:

- `gap(...)`
- `justify_content(...)`
  - at least `flex_start`
  - likely `center`
  - likely `space_between`
- `align_items(...)`
  - at least `stretch`
  - likely `center`
- `flex_grow(...)`

Reason:

If MVP ships with only axis switching and no spacing/alignment controls, many
real scenes will still need layout hacks or manual transforms, which undercuts
the value of flex as an authored surface.

### Safe to defer if needed

- reverse directions:
  - `row_reverse`
  - `column_reverse`
- `flex_shrink(...)`
- `flex_basis(...)`
- wrapping:
  - `flex_wrap(...)`
- per-item alignment overrides
- full CSS parity

## Suggested implementation order

### Phase 1: Honest dispatch

- in `src/engine/ecs/system/layout/mod.rs`, detect `Display::Flex`
- route those containers to `flex::layout(...)`
- stop silently treating flex containers as block layout

### Phase 2: Minimal row/column layout

In `src/engine/ecs/system/layout/flex.rs`:

- row:
  - horizontal main-axis cursor
  - vertical cross-axis placement
- column:
  - vertical main-axis cursor
  - horizontal cross-axis placement

Start with:

- fixed-size items
- measured intrinsic item sizes
- no wrapping

### Phase 3: MVP polish properties

Add only the properties that are actually needed to make authored scenes viable:

- `gap`
- `justify_content`
- `align_items`
- `flex_grow`

This phase should be guided by real examples, especially:

- `examples/mms-tables.mms`
- `examples/html-layout.mms`

## Acceptance criteria

1. A flex row container with three children lays them out horizontally, not as a block column.
2. A flex column container lays children out vertically through the flex path, not through block fallback.
3. `examples/mms-tables.mms` can place left button / table / right button in one authored row without transform hacks.
4. At least one example proves `justify_content` and/or `align_items` if those are included in MVP.
5. The code no longer presents flex authoring as available while silently ignoring it at layout time.

## Open design note

We should explicitly decide which flex-related properties are part of the first
usable MVP, instead of landing only `display("flex")` and `flex_direction(...)`
and then immediately discovering that every real authored scene still needs:

- spacing hacks
- manual transforms
- fake inline-block fallbacks

That decision should happen before implementation starts in earnest.
