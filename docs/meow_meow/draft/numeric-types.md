# ᓚᘏᗢ MMS Numeric Types — Design Draft

> **Status: draft.** Not yet implemented.
> Covers `Int`, `Float`, and `Double`.
> Coercion rules and operator result tables are in [coercion.md](coercion.md).

---

## The three numeric types

| Type | Runtime | Width | Notes |
|------|---------|-------|-------|
| `Int` | `i64` | 64-bit signed integer | Loop counters, array indices, discrete values |
| `Float` | `f32` | 32-bit IEEE 754 | Component boundary type — most engine APIs use f32 |
| `Double` | `f64` | 64-bit IEEE 754 | Default floating-point for authored values |

There is no `Num` supertype. Functions or struct fields that accept any numeric type are
either left unannotated (gradual — no type checking) or overloaded per type. In the
gradual evaluator, an unannotated binding holds whatever value it receives; the type
checker does not enforce anything on it.

---

## Literal syntax

| Literal form | Type | Examples |
|--------------|------|---------|
| No decimal point | `Int` | `0`, `3`, `-7`, `1_000_000` |
| Decimal point or exponent | `Double` | `3.0`, `0.5`, `1e6`, `-2.718` |

There is no `Float` literal. `Float` (f32) appears in type annotations and is produced
by explicit conversion — not authored as a literal. The distinction between `Float` and
`Double` is mainly relevant at component boundaries, not in script arithmetic.

```mms
let i = 3        // Int
let d = 3.0      // Double
let f: Float = 3.0   // Double literal coerced to Float at the annotation boundary
```

Underscore separators in integer literals are allowed (`1_000_000`). Stripped by the
tokenizer, no semantic meaning.

---

## Widening order

Implicit coercion only flows toward wider types:

```
Int  →  Float  →  Double
```

Narrowing (Double → Float, Double → Int, Float → Int) is always **explicit** and
requires a conversion function call. Implicit narrowing is not allowed.

Precision notes:
- `Int → Float`: integers > 2²⁴ cannot be represented exactly in f32 — precision loss,
  not overflow
- `Int → Double`: exact for all integers up to 2⁵³
- `Float → Double`: always exact (f32 is a strict subset of f64)

---

## Explicit conversion functions

| Function | Input | Output | Behaviour |
|----------|-------|--------|-----------|
| `int(x)` | `Float \| Double` | `Int` | Truncate toward zero |
| `floor(x)` | `Float \| Double` | `Int` | Round toward −∞ |
| `ceil(x)` | `Float \| Double` | `Int` | Round toward +∞ |
| `round(x)` | `Float \| Double` | `Int` | Round to nearest, ties to even |
| `float(x)` | `Int \| Double` | `Float` | Widen or narrow to f32 |
| `double(x)` | `Int \| Float` | `Double` | Widen to f64 |

```mms
let n = 7.9
int(n)     // 7  (truncate toward zero)
floor(n)   // 7
ceil(n)    // 8
round(n)   // 8

int(-2.3)  // -2  (truncate toward zero, not floor)
floor(-2.3) // -3
```

---

## Component boundary coercion

Engine APIs use `f32` throughout. When MMS passes a numeric value to a component
constructor or mutation method, automatic boundary coercion applies:

| MMS type | Engine type | Notes |
|----------|-------------|-------|
| `Double` | `f32` | Precision loss accepted — same as current behaviour |
| `Float` | `f32` | Exact |
| `Int` | `f32` | Widens through the chain |

This coercion happens only at component-call boundaries, not in general arithmetic.
Script authors don't need to think about it.

---

## Open questions

1. **Integer overflow** — `Int` arithmetic overflows i64: runtime error, wrapping, or
   saturating? Recommendation: runtime error in the evaluator; transpiled Rust uses
   Rust's debug-mode panic / release-mode wrapping (document the difference).

2. **`Int` as array index** — `arr[i]` where `i: Int` is the natural form. If `i` is
   `Double`, implicit `int(i)` at the index boundary, with a runtime error if the value
   is not a whole number.

3. **`Float` literals** — should `3.0f` be a Float literal? Adds tokenizer complexity
   for a rarely-needed feature. Lean toward no suffix — annotation-boundary coercion
   (`let x: Float = 3.0`) is sufficient.

4. **`NaN` / `Inf` coerced to `Int`** — `int(1.0 / 0.0)` — runtime error.
