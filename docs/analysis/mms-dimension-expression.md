# MMS DimensionExpression — Analysis

Date: 2026-05-30

## Problem

We want MMS to support expressions like:

```mms
let meme_dimensions = [2.85188wu, 4wu]

let bg_quad = T.scale(meme_dimensions[0], meme_dimensions[1], 1.0) { ... }
let panel = T.position(-meme_dimensions[0] / 2.0, -meme_dimensions[1] / 2.0, 0.0) {
    LayoutRoot {
        available_width(meme_dimensions[0])
        available_height(meme_dimensions[1])
    }
}
```

Today this fails for two separate reasons:

1. The surface AST only has `Expression::Dimension(f64, Unit)` for **dimension literals**, not a distinct expression family for **unit-aware math**.
2. Arithmetic in the evaluator only supports `Number op Number`; a variable or array element that carries a unit does not stay on a special numeric path.

The user goal is broader than just fixing `Transform.scale(...)`:

- support unit values and unit math everywhere it makes semantic sense,
- keep ordinary number-only expressions fast,
- preserve enough typed structure that a future VM or transpiler can distinguish scalar math from dimension math.

## Current state

The relevant layers today are:

- Parse AST: `Expression::Number`, `Expression::Dimension`, generic `BinaryOp`, generic `UnaryOp`
- Runtime value: `Value::Number`, `Value::Dimension`
- Boundary coercion: some APIs are unit-aware (`Style`, `LayoutRoot`), many are plain-number-only (`Transform.position`, `Transform.scale`)

Important observation:

`meme_dimensions[0]` is not syntactically a dimension literal. It is an identifier + array index whose value is only known after evaluation / binding resolution. That means a parser-only distinction is not sufficient. A dedicated unit-aware AST type is still useful, but it should exist in a **typed lowered form**, not only in the raw source AST.

## Recommendation

Introduce a **typed numeric lowered AST** with two disjoint paths:

- `ScalarExpression` for ordinary number-only math
- `DimensionExpression` for unit-aware math

Keep the existing source AST mostly intact. Add a lowering / analysis pass that inspects expressions after name resolution and chooses the appropriate numeric form.

This gives us:

- a fast path for ordinary math with no unit checks
- a typed path for expressions containing units directly or indirectly
- explicit semantics for call boundaries
- cleaner future VM / transpiler behavior

## Why not only change the parser AST?

If we only add a new parse node for literal syntax like `4wu`, we still do not solve:

- identifiers bound to dimensions,
- array elements containing dimensions,
- function returns that carry dimensions,
- mixed expressions where unit information appears through the environment rather than directly in source syntax.

Example:

```mms
let h = 4wu
let dims = [2wu, h]
T.scale(dims[0], dims[1], 1.0)
```

The parser only sees identifiers and index expressions in the call. The unit-typed nature of those values is a property of the **resolved expression graph**, not the token stream.

So the right split is:

- raw AST remains syntax-oriented,
- lowered AST becomes semantics-oriented.

## Proposed shape

### Surface AST

We can keep the source AST close to what it is now:

```rust
pub enum Expression {
    String(String),
    Number(f64),
    Dimension(f64, Unit),
    Bool(bool),
    Null,
    Identifier(Ident),
    Array(Vec<Expression>),
    Call(CallExpression),
    Component(ComponentExpression),
    BinaryOp { op: BinOpKind, lhs: Box<Expression>, rhs: Box<Expression> },
    UnaryOp { op: UnaryOpKind, operand: Box<Expression> },
    Function { params: Vec<Ident>, body: BlockStatement },
}
```

This is still useful for parsing, printing, and syntax tooling.

### Lowered numeric AST

Add a typed lowered representation used by evaluation / compilation:

```rust
pub enum NumericExpression {
    Scalar(ScalarExpression),
    Dimension(DimensionExpression),
}

pub enum ScalarExpression {
    Number(f64),
    Identifier(SymbolId),
    ArrayIndex { base: Box<ScalarExpression>, index: Box<ScalarExpression> },
    Unary { op: ScalarUnaryOp, operand: Box<ScalarExpression> },
    Binary { op: ScalarBinaryOp, lhs: Box<ScalarExpression>, rhs: Box<ScalarExpression> },
    Call { callee: CallTargetId, args: Vec<ScalarExpression> },
}

pub enum DimensionExpression {
    Literal { value: f64, unit: Unit },
    Identifier(SymbolId),
    ArrayIndex { base: Box<DimensionExpression>, index: Box<ScalarExpression> },
    Neg(Box<DimensionExpression>),
    Add { lhs: Box<DimensionExpression>, rhs: Box<DimensionExpression> },
    Sub { lhs: Box<DimensionExpression>, rhs: Box<DimensionExpression> },
    MulScalar { lhs: Box<DimensionExpression>, rhs: Box<ScalarExpression> },
    DivScalar { lhs: Box<DimensionExpression>, rhs: Box<ScalarExpression> },
    Convert { expr: Box<DimensionExpression>, to: UnitFamilyTarget },
    Call { callee: CallTargetId, args: Vec<NumericExpression> },
}
```

This is only a sketch. The key property is that dimension math is explicit and separate from scalar math.

## Lowering rules

The lowering pass classifies expressions according to what they produce.

### Scalar stays scalar

These remain on the fast path:

```mms
1 + 2 * 3
speed * dt
i + 1
```

No unit handling, no dynamic unit checks, no conversion tables.

### Dimension expressions are lifted

These lower to `DimensionExpression`:

```mms
4wu
-4wu
width / 2.0
origin + offset
dims[0]
```

where `width`, `origin`, `offset`, or `dims[0]` have been resolved to dimension-typed values.

### Allowed operators

Recommended v1 rules:

- `Dimension + Dimension` when units are compatible
- `Dimension - Dimension` when units are compatible
- `Dimension * Number`
- `Dimension / Number`
- unary `-Dimension`

Not recommended for v1:

- `Dimension * Dimension`
- `Dimension / Dimension`
- `Number + Dimension`
- implicit mixed-family conversion

This keeps the semantics narrow and avoids inventing generalized unit algebra.

## Unit families

Not all unit suffixes belong on the same arithmetic path.

At minimum we already have:

- length-like: `wu`, `gu`, `%`
- angle-like: `deg`, `rad`

These should be tracked as unit families during lowering. A `DimensionExpression` should carry enough information that lowering can reject nonsense like:

- `4wu + 10deg`
- `50% + 2rad`

Percent needs special handling:

- it is not an absolute unit by itself,
- it only becomes meaningful relative to a consuming API or contextual container.

So `%` should likely remain invalid in generic dimension arithmetic until lowered inside a boundary that knows the reference quantity.

## Boundary behavior

This is the part that matters most for engine APIs.

Different call sites do not want the same thing.

### Category A: Preserve unit information

Examples:

- `Style.width(...)`
- `Style.height(...)`
- `Style.padding(...)`
- `LayoutRoot.available_width(...)`
- `LayoutRoot.available_height(...)`

These APIs want a dimension-like result, not a bare float. Their boundary should consume `DimensionExpression` and lower to a typed runtime form such as `Value::Dimension` or `SizeDimension`.

### Category B: Consume world/local scalar lengths

Examples:

- `Transform.position(...)`
- `Transform.scale(...)`
- likely many render / light / physics setters

These APIs do not want `Value::Dimension` at runtime. They want a plain `f32`, but they should be able to accept a `DimensionExpression` at the MMS surface and coerce it to a number at the boundary.

That means the call boundary needs an argument contract such as:

```rust
enum NumericArgKind {
    ScalarNumber,
    WorldLength,
    SizeDimension,
    AngleRadians,
}
```

Then the transform builders can say:

- `position`: `WorldLength, WorldLength, WorldLength`
- `scale`: `WorldLength, WorldLength, WorldLength` or plain scalar if we decide scale is unitless
- `rotation`: `AngleRadians, AngleRadians, AngleRadians`

This is better than a blanket “dimensions become f32 everywhere”, because it keeps each API explicit and future-proof.

## Performance story

The point of splitting the numeric paths is to avoid slowing down ordinary MMS arithmetic.

With the proposed lowered form:

- plain number math remains a compact scalar-only tree,
- scalar evaluation stays branch-light,
- unit conversion logic only runs when lowering produced `DimensionExpression` nodes,
- a future VM can emit specialized bytecode ops for scalar and dimension math separately.

This is much better than making every arithmetic op dynamically inspect `Value::Number` vs `Value::Dimension` at runtime.

## VM / transpiler implications

A typed lowered numeric IR helps both future execution backends.

### VM

The VM can compile:

- scalar ops: `add_f64`, `mul_f64`, `div_f64`
- dimension ops: `add_dim`, `sub_dim`, `mul_dim_scalar`, `div_dim_scalar`

It can also validate unit-family compatibility once during lowering / compilation instead of every frame.

### Transpiler

When targeting another language, the transpiler can choose:

- emit plain numeric expressions for `ScalarExpression`,
- emit helper-wrapped expressions for `DimensionExpression`,
- or fold dimension expressions earlier into literal numeric outputs when the target API consumes a scalar world length.

This separation makes the generated code more predictable.

## Example lowering

Source:

```mms
let meme_dimensions = [2.85188wu, 4wu]
let px = -meme_dimensions[0] / 2.0
```

Conceptually lowered:

```text
meme_dimensions : Array<DimensionExpression>
px : DimensionExpression =
    DivScalar(
        Neg(ArrayIndex(Identifier(meme_dimensions), Number(0))),
        Number(2.0)
    )
```

Then, at call boundaries:

- `LayoutRoot.available_width(px)` preserves dimension semantics
- `Transform.position(px, ...)` converts the dimension expression into a scalar world-space float before executing the builder

## Suggested rollout

### Phase 1

- Keep parse AST unchanged
- Add lowered `ScalarExpression` / `DimensionExpression`
- Lower only the subset needed for length and angle literals + identifier/array propagation
- Add boundary contracts for `Style`, `LayoutRoot`, and `Transform`

### Phase 2

- Add dimension-aware arithmetic for `+`, `-`, `* scalar`, `/ scalar`
- Add typed evaluator / VM hooks

### Phase 3

- Extend other APIs with explicit numeric arg kinds
- Consider whether `%` should gain contextual lowering beyond style/layout consumers

## Non-goals

This proposal does **not** imply:

- full symbolic unit algebra,
- implicit conversion between unrelated unit families,
- slowing all arithmetic down with generic tagged-value checks,
- changing all source-level AST arithmetic nodes immediately.

## Recommendation summary

The right design is not “replace `Expression::Dimension` with a parser-level `DimensionExpression` everywhere”.

The right design is:

1. keep the surface AST syntax-oriented,
2. add a typed lowered numeric IR with a dedicated `DimensionExpression` path,
3. classify API boundaries by what numeric form they consume,
4. preserve a fast scalar-only path for normal arithmetic,
5. only run unit logic for expressions that actually carry units.

That gives MMS the ergonomic behavior users want while keeping evaluation and future backends disciplined.