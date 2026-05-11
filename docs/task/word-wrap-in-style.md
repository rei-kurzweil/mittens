# Expose `word_wrap` (+ tokens) through `Style{}` (=^･ω･^=)

## Why

`TextComponent` already supports two wrap modes:

- `word_wrap: false` — hard wrap at `wrap_at` (cuts mid-token)
- `word_wrap: true` + `word_wrap_tokens: Vec<String>` — prefer wrapping at
  whitespace/tokens (CSS `overflow-wrap: break-word` shape)

But the only way to switch modes is via `TextComponent::with_word_wrap` /
direct `Text { ... word_wrap(true) }` on the text node. In a layout-driven
document, wrap mode logically belongs to the *box* the text sits in (same
place as `padding`, `width`, `background_color`), not the text component
itself. Padding-demo's text hard-wraps mid-token today because nothing in
`Style{}` can flip the mode.

Goal: let MMS authors write `Style { word_wrap("break-word") }` and have
layout propagate that onto the descendant `TextComponent` the same way it
already narrows `wrap_at` to fit the container width.

## What changes

### 1. `StyleComponent` + `StylePatch` — new fields

`src/engine/ecs/component/style.rs`:

```rust
pub enum WordWrapMode {
    Normal,     // hard wrap at wrap_at
    BreakWord,  // prefer whitespace/token boundaries
}

pub struct StyleComponent {
    // ...
    pub word_wrap: Option<WordWrapMode>,
    pub word_wrap_tokens: Option<Vec<String>>,
}
```

`Option<...>` follows the existing inheritance pattern (e.g. `display:
Option<Display>`): `None` = don't touch the `TextComponent`, fall back to
whatever it was authored with (or its `Default`).

Mirror the same `Option<...>` / `Option<Option<...>>` fields on
`StylePatch` so `IntentValue::UpdateStyle` can patch them.

`encode`/`decode` updated to round-trip both fields through the JSON
component-tree dump.

### 2. MMS method registration

`src/meow_meow/component_registry.rs`, in the `StyleComponent` `match
method` block (around line 785):

```rust
"word_wrap" => st.word_wrap = match arg_str(args, 0)? {
    "normal"                    => Some(WordWrapMode::Normal),
    "break_word" | "break-word" => Some(WordWrapMode::BreakWord),
    _                           => None,
},
"word_wrap_tokens" => {
    st.word_wrap_tokens = Some(/* parse list of strings from args */);
}
```

Token-list parsing should match whatever idiom the file uses for
multi-string args (check `arg_str_arr` or comma-split).

### 3. Propagation in layout

`src/engine/ecs/system/layout/measure.rs::apply_text_wrap_for_item` is the
existing propagation point. Today it:

1. Finds descendant `TextComponent` in local-content subtree.
2. Computes `new_wrap_at` from `content_width_gu`.
3. If `new_wrap_at != current_wrap_at`, mutates `TextComponent.wrap_at`
   and emits `SetText` → glyph rebuild.

Extend it to also accept `Option<WordWrapMode>` + `Option<Vec<String>>`
from the parent `StyleComponent`, threaded through from `measure_item`
(which already collects style fields for box-model use). Inside:

1. Compute desired `(wrap_at, word_wrap, tokens)` triple, where
   `word_wrap` / `tokens` come from Style if `Some(_)` else fall back to
   the current TC value.
2. Broaden the early-return gate to compare all three fields.
3. If any differ, mutate `TextComponent` and emit the one `SetText`.

The call sites in `block.rs` and `inline.rs` need their signatures
updated to pass the new overrides — cheapest interface is probably a
small `TextStyleOverride { wrap_mode: Option<WordWrapMode>, tokens:
Option<Vec<String>> }` struct so we don't churn the signature again next
time a text style prop gets added.

### 4. Resolution order

Same as the rest of `StyleComponent`:

1. `StyleComponent.word_wrap = Some(_)` → writes through to
   `TextComponent` on the next layout pass.
2. Otherwise leaves whatever was authored on the `TextComponent`.
3. Default falls through to `TextComponent::DEFAULT` (`word_wrap: false`).

Result: `Text { "..." word_wrap(true) }` directly on a text node still
works, and `Style { word_wrap("break-word") }` on the parent overrides it.

## Performance impact

The honest answer: **no additional glyph rebuilds in the steady state.**

- `apply_text_wrap_for_item` already triggers a glyph rebuild via
  `SetText` whenever `wrap_at` narrows for the first time. Folding the
  word-wrap toggle into the same `SetText` means we never emit a second
  one — at most one rebuild per text per change frame, exactly as today.
- If only `word_wrap` toggles (column count stable), one rebuild happens.
  That's unavoidable — the glyph layout depends on which algorithm
  produced the line breaks.
- Steady-state cost: a couple extra `==` comparisons in the early-return
  gate (mode + token-vec). Negligible.
- First-frame cost: identical to today. One `SetText` per text item to
  apply container-derived wrap settings; subsequent ticks short-circuit
  via the equality gate.
- Footgun: anyone toggling `word_wrap` every tick (none do) eats one
  glyph rebuild per tick. Same shape as toggling `text` every tick —
  not a regression, just a property of the rebuild model.

Why the gate is safe: the existing `wrap_at == current_wrap_at` short-
circuit already prevents per-tick rebuilds for unchanging input. Extending
it to `(wrap_at, word_wrap, tokens)` preserves that behavior; we just
treat any of the three changing as a reason to rebuild.

## Verification

1. `cargo build --release` clean.
2. `cargo test`:
   - Existing layout tests pass unchanged (no Style sets `word_wrap`, so
     propagation no-ops).
   - New unit test in `measure.rs`: construct a TC with
     `StyleComponent { word_wrap: Some(BreakWord), .. }` and a child
     `TextComponent { word_wrap: false }`; run one measure pass; assert
     the `TextComponent.word_wrap == true` afterwards.
3. `cargo run --release --example padding-demo` after adding
   `word_wrap("break-word")` on the text cells in
   `examples/padding-demo.mms`. Expected: "it's a piece of cake" wraps at
   the space rather than mid-word.
4. Inspect: any example that doesn't author `word_wrap` should render
   identically to the previous build.

## Out of scope

- CSS-spec `overflow-wrap` / `word-break` granularity beyond the simple
  two-mode toggle.
- Exposing `wrap_at` itself via `Style{}` — still derived from container
  width; authors override with `Text { wrap_at(...) }` when they want a
  fixed cap.
- A separate `UpdateWordWrap` intent. Piggybacking on the existing
  `SetText` rebuild keeps the intent surface small.

🧃 rawr
