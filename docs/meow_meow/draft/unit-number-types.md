# ₊˚ʚ Unit Number Types in MMS ＼(＾▽＾)／

Design for unit-aware numeric types in Meow Meow Script (MMS). This allows users to write natural units like `180deg` or `1.5rad` and have the evaluator handle the conversion to the engine's internal representation (always radians for angles).

---

## 1 — Syntax & Parser Representation

The parser identifies unit suffixes immediately following a numeric literal and wraps them in a `UnitFloat` node.

### Tokens
- `deg` (degrees)
- `rad` (radians - identity unit)

### Meow Meow AST Representation (Conceptual)
A numeric literal with a unit becomes a `UnitFloat`:

```rust
pub enum Expression {
    // ...
    Number(f64),
    UnitFloat {
        value: f64,
        coefficient: f64, // Factor to convert to absolute internal unit
    },
}
```

### Examples
| MMS Source | Internal `UnitFloat` | Absolute Value (Internal) |
|---|---|---|
| `180deg` | `{ value: 180.0, coefficient: PI / 180.0 }` | `3.14159...` |
| `1rad` | `{ value: 1.0, coefficient: 1.0 }` | `1.0` |
| `0.5` | `Number(0.5)` | `0.5` |

---

## 2 — Evaluator Behavior

The `eval_expr` logic for `UnitFloat` is straightforward:

```rust
fn eval_expr(expr: &Expression, ...) -> Result<Value, String> {
    match expr {
        Expression::Number(n) => Ok(Value::Number(*n)),
        Expression::UnitFloat { value, coefficient } => {
            // Units are collapsed into absolute numbers at evaluation time
            Ok(Value::Number(value * coefficient))
        }
        // ...
    }
}
```

By collapsing units at evaluation time, the rest of the engine (Intents, Components, Systems) never needs to know about units; they always receive absolute numbers in the canonical engine units (Radians, Meters, Seconds).

---

## 3 — Roadmap

1.  **Lexer Update:** Recognize `deg` and `rad` as valid suffixes for numbers.
2.  **Parser Update:** Create `UnitFloat` nodes when suffixes are encountered.
3.  **Evaluator Update:** Implement the `value * coefficient` collapse logic.
4.  **Refactor:** Update existing MMS code to use `180deg` instead of manual `3.1415...` constants.

---

## 4 — Future Extensions

- **Time:** `1s`, `500ms`, `2beats`.
- **Distance:** `1m`, `100cm`.
- **Screen:** `10px`, `50%`.

These would follow the same `UnitFloat` pattern, collapsing into the engine's base unit (Seconds, Meters, Pixels/Relative) during evaluation.
