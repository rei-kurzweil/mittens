# inline layout — true inline formatting context (draft) ʕ•ᴥ•ʔ

Status: **draft / not implemented**. Today the engine handles `display: inline-block` (atomic flow boxes with horizontal cursor + wrap). `display: inline` currently falls through to inline-block treatment as a stop-gap so existing scenes don't break.

## What "true inline" needs (and inline-block doesn't do)

Inline-block treats every TC as an atomic, indivisible box. That's enough for icons-beside-text rows, tag chips, toolbars. It is **not** enough for prose, where:

1. **Line boxes span sibling TCs.** `<span>hello </span><span>world</span>` should share one line box and wrap mid-stream, not stack two boxes.
2. **Mid-text wrap.** A long text run inside one inline span breaks on word boundaries into multiple line boxes; today a `TextComponent` is measured as one rectangle.
3. **Baseline alignment.** Mixed font sizes / inline images on the same line align on the typographic baseline, not the top edge.
4. **Whitespace collapsing.** Consecutive spaces collapse to one; trailing whitespace at line break is consumed.
5. **Vertical-align modes.** `top`, `middle`, `bottom`, `baseline`, numeric offsets — needed once we mix small icons with text glyphs in the same line.

## Why inline-block fallback is OK for now

The visible regressions are small and well-defined:

- `Display::Inline` items get one box per TC instead of glyph-level flow → readable but ugly for long prose.
- No baseline alignment → icons sit at item top-left, not on text baseline.
- Wrapping happens between TCs only, not mid-text.

For the panels / inspector / icon-row cases on `mittens`, this is fine. The fallback also keeps `Display::Inline` from dropping authored items into block stacking, which would be a worse failure mode (icons on their own line beneath text).

Dispatch sites doing the fallback today (search: `Display::Inline`):

- `src/engine/ecs/system/layout/mod.rs::run_layout`
- `src/engine/ecs/system/layout/block.rs::layout_items`
- `src/engine/ecs/system/layout/inline.rs::layout_items`

All three coalesce `Display::Inline | Display::InlineBlock` into the inline cursor path.

## Sketch of true inline support

### Pass 0 — content collection

Walk the inline subtree and produce a flat list of **inline runs**:

```rust
enum InlineRun {
    Text { tc_id: ComponentId, text: String, font_metrics: FontMetrics, style: InlineStyle },
    AtomicBox { tc_id: ComponentId, measured: MeasuredItem, vertical_align: VerticalAlign },
    Break,                 // explicit <br>
    Whitespace { collapsed: bool },
}
```

Boundaries: stop descending when a TC has `display: block` (it terminates the inline context).

### Pass 1 — line breaking

Run a Knuth-Plass-lite (or just greedy) breaker over the run list, producing a sequence of **line boxes**:

```rust
struct LineBox {
    runs:    Vec<RunFragment>,    // runs may be split across line boxes
    height:  f32,                 // tallest box on the line, after baseline math
    baseline_y: f32,              // ascent above this y; descent below
}
struct RunFragment {
    run_idx:  usize,              // index into the InlineRun list
    char_lo:  usize,              // for Text runs: glyph slice
    char_hi:  usize,
    advance:  f32,
}
```

Greedy breaker is fine until we care about justification.

### Pass 2 — emit transforms

For each line box, walk its fragments left-to-right. For text fragments, hand the slice to `TextSystem` so it can render only that span at the resolved position. For atomic boxes, emit the existing `UpdateTransform` intent at `(cursor_x, line_baseline_y - box.baseline_offset)`.

The text rendering path is the hard part — `TextComponent` today owns its full string and wrap behavior. To support mid-text wrap from layout, we either:

- **(A)** Split `TextComponent` content into per-line child `TextComponent`s authored by the layout pass (adds churn each layout tick, but reuses the existing renderer).
- **(B)** Add a `TextRangeComponent` sibling that tells `TextSystem` to render only `[char_lo, char_hi)` of the parent text — keeps one source of truth, requires a renderer-side change.

(B) is cleaner and probably the right target.

### Baselines / vertical-align

Each inline run carries a `VerticalAlign`. Line height = `max(ascent_i) + max(descent_i)` over runs; baseline_y = `max(ascent_i)`. Per-run y offset = `baseline_y - ascent_i + vertical_align_offset`. Atomic boxes default to `vertical_align: baseline` with `descent = 0` (their bottom edge lands on the baseline) — matches CSS UA default.

## What this draft does NOT cover

- **Bidi / RTL text.** Punted; we have no bidi pass anywhere yet.
- **Justified text.** Greedy breaker is enough to start; justification needs glyph-level x-advance adjustment in `TextSystem`.
- **Floats.** Out of scope — we don't have `float` either; would need its own design.
- **Hanging punctuation, hyphenation, soft-hyphens.** Future polish.

## Acceptance criteria for "real inline lands"

1. A paragraph with mixed `<span>` styles shares a single line box and wraps mid-stream on word boundaries.
2. An inline atomic box (icon) inside the paragraph aligns on the text baseline by default; `vertical-align: middle` shifts it visibly.
3. `examples/inline-block-layout.mms` keeps working unchanged (inline-block fallback no longer needed but must stay valid).
4. A new `examples/inline-text-wrap.mms` demonstrates a multi-line wrapped paragraph with a mid-line icon.
5. The fallback coalescing of `Display::Inline | Display::InlineBlock` is removed from the three dispatch sites; `Display::Inline` is handled by the new path; `Display::InlineBlock` stays on the existing atomic path.

## Open questions

- Do we keep `inline.rs` as the inline-block path and add `inline_text.rs` for true inline, or fold both into one module that branches per run kind?
- How do we represent a font metric snapshot (`ascent`, `descent`, `x_height`) for measure-time math when text is technically GPU-rendered?
- Should `TextComponent.wrap_at` survive once the layout pass owns wrapping, or become a hint only?

rawr
