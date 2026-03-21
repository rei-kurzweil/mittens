# Expression evaluation in MMS

Design decisions for Phase 2 of the MMS roadmap: arithmetic, boolean, and comparison
expressions.

---

## Number types

Currently `Expression::Number(f64)` — all numeric literals are 64-bit floats.

**The question:** should MMS distinguish integers from floats?

Most component fields are `f32`. Some take `usize` (counts, indices). `f64` in the script
layer is wide enough to represent all of them without precision loss for practical values.

**Options:**

| Option | Tokens | AST | Notes |
|--------|--------|-----|-------|
| A: everything `f64` | `Number(f64)` as-is | no change | Cast to `f32`/`usize` at component boundary. Simple. May surprise users who write `range(10)` and get a float. |
| B: `Int(i64)` + `Float(f64)` | two numeric literal kinds | `Expression::Int` / `Expression::Float` | More precise. Slightly more parser complexity. Allows `range(n)` to require an int. |
| C: infer from context | `Number(f64)` as-is | no change | The registry/component boundary handles the cast silently. Same as A but framed as "always right". |

**Recommendation:** start with Option A (single `Number(f64)`), cast at the component
boundary. Add `Int` if script-level integer semantics are needed (e.g. array indexing,
`range` argument). The cast boundary already exists in `component_registry.rs`.

---

## Operator precedence

Standard precedence table (highest to lowest):

| Level | Operators | Associativity |
|-------|-----------|---------------|
| 7 | unary `-`, `!` | right |
| 6 | `*`, `/`, `%` | left |
| 5 | `+`, `-` | left |
| 4 | `<`, `>`, `<=`, `>=` | left, non-assoc |
| 3 | `==`, `!=` | left |
| 2 | `&&` | left |
| 1 | `\|\|` | left |

This matches Rust, C, and most scripting languages. No surprises expected.

Parentheses `( expr )` have highest precedence (already parsed via `LParen`/`RParen`).

---

## Mixing arithmetic and logical without parens

Should `1 + 2 == 3 && true` parse as `((1 + 2) == 3) && true` (standard) or require
explicit parens? Standard precedence handles this correctly without requiring parens —
no special rule needed.

---

## New tokens needed

```
Plus        +
Minus       -
Star        *
Slash       /
Percent     %
EqEq        ==
BangEq      !=
Lt          <
Gt          >
LtEq        <=
GtEq        >=
AmpAmp      &&
PipePipe    ||
Bang        !
```

`-` is ambiguous: subtraction (`a - b`) vs unary negation (`-x`). Resolved in the parser
by context (unary applies when `-` appears at the start of an expression or after an
operator).

---

## New AST nodes needed

```rust
Expression::BinaryOp {
    op: BinaryOpKind,
    lhs: Box<Expression>,
    rhs: Box<Expression>,
}

Expression::UnaryOp {
    op: UnaryOpKind,
    operand: Box<Expression>,
}

pub enum BinaryOpKind {
    Add, Sub, Mul, Div, Mod,
    Eq, NotEq, Lt, Gt, LtEq, GtEq,
    And, Or,
}

pub enum UnaryOpKind { Neg, Not }
```

---

## Runtime value changes

`StoredValue::Primitive(String)` is the current placeholder. Phase 2 replaces this with
a proper `Value` enum:

```rust
pub enum Value {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<Value>),
    ComponentExpr(Box<ComponentExpression>),  // v1: unresolved CE
    // Phase 6: ComponentObject(ComponentId), // live handle
    // Phase 4: Function { params, body, env },
}
```

Arithmetic: `Number + Number → Number`, type error otherwise.
Comparison: `Number cmp Number → Bool`, `String == String → Bool`.
Logical: `Bool && Bool → Bool`; short-circuit evaluation (left side evaluated first).

---

## Coercion policy

MMS v1: **no implicit coercion**. `1 + true` is a runtime type error, not `2`. Type errors
produce `EvalResponse::Error` and skip the statement; they do not halt the script.

String concatenation via `+`: `"hello" + " world" → "hello world"` — this one implicit
coercion is probably worth supporting since string building is common in text components.
