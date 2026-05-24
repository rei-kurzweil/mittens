# Text measurement vs. glyph render — unit-system mismatch ( ˘•灬•˘ )

## TL;DR

Layout treats `font_size` as **glyph units** (GU).
The renderer treats `font_size` as **local units of the inner text-bearing TC**
— which, given the way `LayoutSystem` positions styled boxes, is actually
**world units**.

When `LayoutRoot.unit_scale != 1.0` (e.g. `world_panel` uses `unit_scale = 0.08`),
the two interpretations diverge by exactly `unit_scale`. Math accidentally works
for **single-line text with `vertical_align: middle`** because the off-by-`unit_scale`
term collapses out. It breaks everywhere else:

- list rows with auto-height (`world_panel_content_root` items) — content box is
  ~`0.0064 wu` tall but the glyph block is `font_size = 0.08 wu` tall, so text
  overflows the content area and pokes into padding / margin viz.
- multi-line `vertical_align: middle` — the text block off-centers by roughly
  `(rows - 1) * font_size / 2` world units.
- any `vertical_align: bottom` / `vertical_align: top` on multi-line text —
  same off-by-`unit_scale` story.

The user-visible symptom that prompted this writeup: text in
`world_panel_content` rows sits about a half-glyph below where the row's
content/padding/margin viz says it should, intruding into the purple/red
margin band.

## Where the conflict lives

### Measurement claims GU

`src/engine/ecs/system/text_system.rs:415` — `TextSystem::measure(...)`:

```rust
(state.max_col as f32 * font_size, (state.row + 1) as f32 * font_size)
```

Caller comment:
```rust
/// Returns `(width_gu, height_gu)` in glyph units after applying font size.
```

`src/engine/ecs/system/layout/measure.rs:355-383` — `text_intrinsic_height`:
the return value is plugged straight into `MeasuredItem.content_height_gu`,
and is also what `intrinsic_block_height` returns when the auto-height TC
contains text. So *measurement-side* code believes `font_size` is in GU.

### Render places glyphs in styled-TC-local units

`src/engine/ecs/system/text_system.rs:347`:

```rust
let t = TransformComponent::new()
    .with_position(x, y, 0.0)
    .with_scale(font_size, font_size, 1.0);
```

The glyph quad (1×1) becomes `font_size × font_size` in the **parent
transform's local frame**. Walking up the chain:

- `TextComponent` parent = inner `T.position(0,0,Z)` wrapper (scale 1).
- That wrapper's parent = the styled `T` (scale 1 — `layout_items` in
  `block.rs:88-100` preserves whatever scale the TC had, which is 1 unless
  the author overrode it).
- The styled `T`'s parent = the `LayoutRoot` TC (scale 1).
- The `LayoutRoot`'s parent = whatever positioning `T` the author placed
  it under — usually scale 1 (`world-panel.mms:37` is just `T.position`).

So a glyph rendered with `font_size = 0.08` is **0.08 world units tall**
in the panel example.

But layout said the row's content box was `1 * font_size = 0.08` **GU** tall,
which `block::layout_items` positions as
`content_height_gu * unit_scale = 0.08 * 0.08 = 0.0064 wu`.

Glyph ≈ 12.5× taller than the content box layout reserved for it.

### `LayoutComponent.unit_scale` docs assume the styled-TC frame *is* GU

`src/engine/ecs/component/layout.rs:28-37`:
```
Scale factor to convert glyph units → local coordinates of the nearest
ancestor `TransformComponent`.
```

`LayoutSystem` honors that for translations and for `__bg` scale
(`block.rs:362`, `block.rs:97`), but the glyph spawn path bypasses it
entirely — glyphs end up at `font_size` of the styled-TC's frame, not
`font_size * unit_scale`.

## Why `vertical_align: middle` "works" for single-line text

`block::apply_text_align` (`block.rs:441-505`):

```rust
let (text_w, text_h) = TextSystem::measure(&text, 0, word_wrap, &tokens, font_size);
let half_glyph = font_size * 0.5;
...
VerticalAlign::Middle =>
    -((((content_h_gu - text_h) * 0.5) + half_glyph) * unit_scale),
```

For **one row**, `text_h = font_size = 2 * half_glyph`, so

```
(content_h_gu - text_h)/2 + half_glyph
  = content_h_gu/2 - half_glyph + half_glyph
  = content_h_gu/2
```

→ `y_translation = -content_h_gu/2 * unit_scale` — the middle of the content
box in world units. Pure luck: `text_h` (which is in the wrong unit) cancels
the `half_glyph` term (also in the wrong unit), and the formula coincides
with "center the inner T on the content midline." Since the inner T has scale 1
and the glyph is centered at its origin, the rendered glyph also ends up
centered. ✓ for one row.

For **N rows** the block height is `N * font_size` in world units (rendered),
but the formula plugs `N * font_size` as if it were GU. After multiplying by
`unit_scale`, the formula treats the text block as `unit_scale × smaller`
than it really is, so the resulting y leaves the text block off-center by
roughly `(N-1) * font_size / 2 * (1 - unit_scale)` ≈ `(N-1) * font_size / 2`
world units when `unit_scale << 1`. For `unit_scale = 0.08`, 2-row text
shifts down by ~`0.5 * font_size` wu — half a glyph below center.

The test `block::tests::vertical_align_middle_respects_text_font_size`
(`block.rs:920`) only asserts on the single-row case and so passes the
accidental-cancellation path. There is no multi-row coverage of vertical
alignment with `unit_scale != 1.0`.

## Why list rows look low

`world_panel_content.mms:21-41` — `world_panel_row` sets:

```
Style {
    display("block")
    width(100%)
    margin_xy(0.25, 0.20)
    padding_xy(0.55, 0.45)
    font_size(TEXT_SCALE)   // = 0.08
    background_color = bg
}
T.position(0, 0, 0.015) { Text { label, C.rgba(...) } }
```

No explicit height → auto height → `text_intrinsic_height` →
`1 row * font_size = 0.08 GU` → `content_height_gu = 0.08`.

In `block::layout_items` the styled `T` is placed at
`content_origin = (margin_left + padding_left, margin_top + padding_top)` GU,
times `unit_scale`. The row's content box, in the styled-T's local (world-unit)
frame, spans:

- top:    `y = 0`
- bottom: `y = -content_height_gu * unit_scale = -0.0064 wu`

`apply_text_align` runs with `vertical_align = Auto, text_align = Auto`,
so it falls through to:

```rust
VerticalAlign::Auto => -(half_glyph * unit_scale),
```

i.e. `y_translation = -font_size/2 * unit_scale = -0.04 * 0.08 = -0.0032 wu`.

The renderer then spawns row-0 glyphs at row-local `y = 0` with
`scale = font_size = 0.08`. In the styled-T's frame:

- glyph row-0 center: `y = -0.0032`
- glyph top edge:    `y = -0.0032 + 0.04 = +0.0368`
- glyph bottom edge: `y = -0.0032 - 0.04 = -0.0432`

The layout believed the row's reserved area extended only to `-0.0064 wu`
(content bottom) or `-(0.08 + 0.45) * 0.08 = -0.0424 wu` (padding bottom),
with the **margin** band starting beyond that. The glyph's actual bottom edge
`-0.0432 wu` is past padding-bottom, so it lands inside the margin viz.

Visually: the text sits half-a-glyph below where the row's padding box ends,
slipping into the pink/red margin viz of `__box_model_viz`. Exactly the
"half its gu height too far down" the user described.

(Same applies to the next row down — its margin-top sits where the previous
row's glyph bottom already is, so glyph rendering overlaps row gaps too.)

## Knock-on effects

1. **`__bg` size vs glyph size**. `sync_bg_quad` (`block.rs:354-366`) uses
   `box_width_gu * unit_scale` for scale — correct as "GU box → world quad."
   Buttons in `world_panel` get a 0.55 wu × 0.192 wu bg with a 0.08 wu glyph
   inside — proportionally sensible. Rows get a `width_GU * 0.08` wu bg
   (a horizontal sliver) with a 0.08 wu × `N * 0.08` wu glyph block on top.
   Background looks fine; text doesn't fit it.

2. **Scrolling content height**. `sync_scrolling_metrics` (`block.rs:268-285`)
   sums `margin_box_height_gu` over nested items. Because each row's
   `content_height_gu = font_size = 0.08`, the sum vastly under-reports the
   true rendered height of the row list, so scroll thumb size / max scroll
   end up wrong (rows visually clip beyond what the scroll machinery
   believes).

3. **`apply_text_wrap_for_item`** (`measure.rs:391-455`) uses
   `container_cols_for_width_and_font_size(content_width_gu, font_size)`:

   ```rust
   let glyph_advance_gu = font_size.max(EPS) * CHAR_WIDTH_GU;
   (content_width_gu / glyph_advance_gu).floor()
   ```

   This is consistent with the "font_size is GU" interpretation, so the
   horizontal column count is computed against the GU-claimed width. Same
   mismatch story on the inline axis — `container_cols` over-counts when
   the rendered glyph is actually `font_size` wu wide and the container box
   is `content_width_gu * unit_scale` wu wide. For the rows
   (`width(100%) * unit_scale = 29.5 * 0.08 = 2.36 wu`), the wrap_at it
   produces is ~30 (29.5 / 1.0 with font_size 0.08 GU = floor(29.5/0.08) = 368
   cols!). The wrap_at silently inflates and rows never wrap — they overrun
   instead. (You can confirm by setting a longer row string.)

4. **Inline cursor wrap** (`inline::layout_items`, `inline.rs:36-136`) uses
   `item.margin_box_width_gu` against `avail_w_gu`. Both are GU, so wrap
   timing is internally consistent. The render mismatch only manifests on
   the rendered glyph size, not on layout flow.

## What "correct" looks like

Two coherent choices:

### Option A — `font_size` is GU end-to-end (preferred)

Renderer scales glyphs by `font_size * cumulative_unit_scale_from_nearest_LayoutRoot`,
or — simpler — `LayoutSystem` inserts a scale step on the styled-TC so its
children operate in GU and the renderer's existing `font_size` scaling lands
in GU. That would also remove the `* unit_scale` sprinkled through `__bg`
and `apply_text_align`, since children of the styled TC would already speak
GU.

This is invasive — every layout-positioned local transform changes — but it
makes `font_size = 1.0` mean "one row per GU" and matches the
`LayoutComponent` doc comment's stated invariant ("StyleComponent heights
are authored in glyph units").

### Option B — `font_size` is world-units and measure converts back

Easier patch. `TextSystem::measure` keeps returning `rows * font_size` but
the layout side divides by the current `LayoutRoot.unit_scale` when
consuming it as a GU height:

```rust
let text_h_gu = TextSystem::measure(...).1 / unit_scale;
```

…in `text_intrinsic_height`, `apply_text_align`, and the wrap-column
math. The `half_glyph` term becomes `font_size * 0.5 / unit_scale` in GU,
or just stays in WU and is added to the y_translation **after** the
`unit_scale` multiplication (so the formula stops mixing units inside the
multiply).

Author code stays the same — `font_size(TEXT_SCALE)` keeps meaning
"glyph quads are 0.08 wu tall" — but layout finally agrees with what
the renderer will draw.

The world_panel author behavior that exposed the bug
(`font_size(TEXT_SCALE)` + `unit_scale(TEXT_SCALE)`) implies they expect
**Option B**: `font_size` is the world-unit glyph size. The simplest
follow-up is to harmonize on that.

## Suggested test gaps to close before fixing

- `vertical_align: middle` with **multi-line** text under
  `unit_scale != 1.0` — should center the rendered block, not the
  GU-claimed block.
- `vertical_align: bottom` / `top` with multi-line under `unit_scale != 1.0`.
- Auto-height row that contains one row of text under `unit_scale = 0.08` —
  assert `content_height_gu` matches **rendered** glyph height in GU
  (i.e. `1.0`, not `0.08`).
- `apply_text_wrap_for_item` with `font_size != unit_scale` to pin the wrap
  column count to what the renderer's wrap state actually produces.

These three plus a render-side smoke (glyph-block bounds vs padding-box
bounds in world space) would lock the contract.

## Files to touch when fixing

- `src/engine/ecs/system/text_system.rs` — `measure`, `register_text` glyph
  scale, `WordWrapState::cursor_pos` (all share the `font_size` semantics
  decision).
- `src/engine/ecs/system/layout/measure.rs` — `text_intrinsic_height`,
  `text_intrinsic_width`, `container_cols_for_width_and_font_size`,
  `apply_text_wrap_for_item`.
- `src/engine/ecs/system/layout/block.rs` — `apply_text_align`
  (vertical/horizontal alignment math, `half_glyph` term).
- `src/engine/ecs/component/layout.rs` — doc comment on `unit_scale` so the
  invariant matches whatever direction we go.

Don't touch `box_model_viz.rs` — it's the messenger, not the bug. The viz
itself is correctly drawing the GU-claimed boxes; the glyphs just don't
agree with those boxes today.

rawr 🧻🧻🧻
