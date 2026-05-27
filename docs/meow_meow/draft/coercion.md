# ᓚᘏᗢ MMS Type Coercion — Design Draft

> **Status: draft.** Not yet implemented.
> One table per operator showing result types for all LHS × RHS combinations.
> Numeric types only in this version. String coercion covered in a later section.
> See [numeric-types.md](numeric-types.md) for type definitions and widening rules.

---

## Reading the tables

Each cell shows the **result type** of `lhs op rhs`. The rule is: widen both operands
to the wider of the two types, then apply the operation in that type.

Widening order: `Int` < `Float` < `Double`

`Err` means a type error — caught at compile time by the type checker, or at runtime
in the gradual evaluator if an unexpected type is encountered.

---

## `+` — Addition

| LHS \ RHS | `Int` | `Float` | `Double` |
|-----------|-------|---------|----------|
| `Int` | `Int` | `Float` | `Double` |
| `Float` | `Float` | `Float` | `Double` |
| `Double` | `Double` | `Double` | `Double` |

---

## `-` — Subtraction

| LHS \ RHS | `Int` | `Float` | `Double` |
|-----------|-------|---------|----------|
| `Int` | `Int` | `Float` | `Double` |
| `Float` | `Float` | `Float` | `Double` |
| `Double` | `Double` | `Double` | `Double` |

---

## `*` — Multiplication

| LHS \ RHS | `Int` | `Float` | `Double` |
|-----------|-------|---------|----------|
| `Int` | `Int` | `Float` | `Double` |
| `Float` | `Float` | `Float` | `Double` |
| `Double` | `Double` | `Double` | `Double` |

---

## `/` — Division

Same widening rule as `+`, `*`. `Int / Int` produces `Int` with truncation toward zero.
Mixed types widen to the wider type.

| LHS \ RHS | `Int` | `Float` | `Double` |
|-----------|-------|---------|----------|
| `Int` | `Int` ⚠️ | `Float` | `Double` |
| `Float` | `Float` | `Float` | `Double` |
| `Double` | `Double` | `Double` | `Double` |

⚠️ `Int / Int` truncates toward zero — `7 / 2 = 3`, `-7 / 2 = -3`.
Division by zero: `Int / 0` is a runtime error. `Float / 0.0` and `Double / 0.0`
produce `Inf` or `-Inf` per IEEE 754.

```mms
7 / 2          // Int / Int   → Int    → 3
7 / 2.0        // Int / Double → Double → 3.5
7.0 / 2        // Double / Int → Double → 3.5
float(7) / 2   // Float / Int → Float  → 3.5
```

---

## `%` — Remainder

| LHS \ RHS | `Int` | `Float` | `Double` |
|-----------|-------|---------|----------|
| `Int` | `Int` | `Float` | `Double` |
| `Float` | `Float` | `Float` | `Double` |
| `Double` | `Double` | `Double` | `Double` |

For float types, `%` uses `fmod` semantics — result has the sign of the dividend.
`Int % 0` is a runtime error. `Float/Double % 0.0` → `NaN` (IEEE 754).

---

## `==` / `!=` — Equality

All combinations produce `Bool`. Both operands are widened to a common type before
comparison.

| LHS \ RHS | `Int` | `Float` | `Double` |
|-----------|-------|---------|----------|
| `Int` | `Bool` | `Bool` | `Bool` |
| `Float` | `Bool` | `Bool` | `Bool` |
| `Double` | `Bool` | `Bool` | `Bool` |

`1 == 1.0` widens the Int to Double first, then compares `1.0 == 1.0` → `true`.

Floating-point equality has the usual caveats (`0.1 + 0.2 != 0.3`). No epsilon
comparison is built into `==`. For approximate equality use `abs(a - b) < epsilon`.

`NaN != NaN` — IEEE 754 behaviour, applies to `Float` and `Double`.

---

## `<` / `>` / `<=` / `>=` — Ordering

All combinations produce `Bool`. Operands are widened before comparison.

| LHS \ RHS | `Int` | `Float` | `Double` |
|-----------|-------|---------|----------|
| `Int` | `Bool` | `Bool` | `Bool` |
| `Float` | `Bool` | `Bool` | `Bool` |
| `Double` | `Bool` | `Bool` | `Bool` |

`NaN` comparisons always return `false` (IEEE 754).

---

## Unary `-` — Negation

| Operand | Result |
|---------|--------|
| `Int` | `Int` |
| `Float` | `Float` |
| `Double` | `Double` |

Type is preserved. No coercion.

---

## Implicit widening summary

When operands have different types, the narrower widens to match the wider before the
operation:

```
Int   op  Float   →  Float   op  Float
Int   op  Double  →  Double  op  Double
Float op  Double  →  Double  op  Double
```

Widening is always implicit for arithmetic and comparison. Narrowing is never implicit —
requires an explicit call: `int()`, `float()`, `double()`.

---

## `+` with strings — placeholder

> Full string coercion tables to be added in a follow-up.

`String + any` and `any + String` produce `String` via automatic stringification of the
non-string operand. The table will cover all combinations of `String` with `Int`,
`Float`, `Double`, `Bool`, and `Null`.

---

## Open questions

1. **`Int / Int` floor vs truncate** — decided: truncate toward zero (C/Rust semantics).
   Truncate and floor differ only for negative dividends: `-7 / 2 = -3` (truncate) vs
   `-4` (floor). If floor division is needed, use `floor(double(a) / double(b))`.

2. **`NaN` / `Inf` coerced to `Int`** — `int(inf)` or `int(nan)`: runtime error.

3. **`Int` overflow** — see numeric-types.md open question 1.
