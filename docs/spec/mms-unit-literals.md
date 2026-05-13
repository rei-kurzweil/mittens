# MMS Unit Literals (≧◡≦)

MMS supports unit-suffixed number literals so authors can write the same
property in any unit the engine understands without separate `_pct` setters.

## Syntax

A unit suffix attaches directly to a numeric literal — no whitespace allowed
between the number and the unit. Recognized units:

| Suffix | Meaning                              | Internal type                          |
|--------|--------------------------------------|----------------------------------------|
| `%`    | Percentage (0–100)                   | `SizeDimension::Percent`               |
| `gu`   | Glyph units (1.0 = one cell)         | `SizeDimension::GlyphUnits`            |
| `deg`  | Degrees (forward-compatible)         | `Value::Dimension { Unit::Degrees }`   |
| `rad`  | Radians (forward-compatible)         | `Value::Dimension { Unit::Radians }`   |

Bare numbers (no suffix) used in a length-typed setter default to glyph units,
which matches existing behavior.

### Disambiguating `%` from modulo

The `%` suffix only attaches when it directly follows a number with no
whitespace. `5%` is a percentage literal; `5 % 2` is modulo.

## Supported setters

The Style setters that accept a `SizeDimension`:

```mms
Style {
    width(50%)         // 50% of the container's content width
    height(20gu)       // 20 glyph units (same as bare `20`)
    padding(10%)       // 10% of inline-axis width on all four sides
    margin_xy(5%, 2gu) // axes form
    top(10%)           // positioned-layout offsets (consumer pending)
}
```

### CSS-aligned semantics

- **Box sizing**: cat-engine defaults to **`border-box`** — `width(...)`
  describes the **outer (padding+content) box**, and padding eats into
  the content area. This differs from CSS's default `content-box` but
  matches the modern best-practice / Bootstrap default and makes percent
  math compose cleanly: two siblings with `width(25%) + width(75%)` fit
  a parent's content width exactly even when each has its own padding.
  Both modes are supported — set per-element with
  `box_sizing("content_box")` or `box_sizing("border_box")` in a `Style`
  block.
- **Width**: percent resolves against the *containing block's* content
  width (the parent's content area, after the parent's own padding is
  subtracted).
- **Height**: percent only resolves when the container's height is
  determined; with an auto-height parent, percent height falls back to 0
  (matches CSS's conservative rule).
- **Padding/margin**: percent always resolves against the **inline-axis**
  (container width), even for the top/bottom sides. This matches W3C CSS.

## AST

Unit literals lex to `TokenKind::Dimension(f64, Unit)` and produce
`Expression::Dimension(f64, Unit)`, evaluating to
`Value::Dimension { value, unit }`. The bare-number path stays on the
existing `Number` token / `Number(f64)` expression / `Value::Number` value.

## Out of scope

- `em`, `vh`, `vw`, `calc()` — not in v1.
- `deg` / `rad` consumers — the lexer/parser/AST land here; rotation
  setters still take bare floats. Switching them is a follow-up.

rawr ✨
