# MMS expression spec

Authoritative reference for all expression AST nodes, operator tokens, and runtime
`Value` types in Meow Meow Script.

Status markers: ✅ implemented · 🔧 planned (phase noted) · ❓ open question

---

## 1. Tokens

### 1.1 Literal tokens

| Token | Lexeme | Notes |
|-------|--------|-------|
| `Ident(String)` ✅ | any identifier | component names, variable names, keywords before keyword match |
| `String(String)` ✅ | `"..."` | UTF-8; no escape sequences yet |
| `Number(f64)` ✅ | `0`, `3.14`, `1e6` | all numerics parse as `f64` |
| `True` ✅ | `true` | |
| `False` ✅ | `false` | |
| `Null` ✅ | `null` | |

### 1.2 Keyword tokens

| Token | Lexeme | Notes |
|-------|--------|-------|
| `Let` ✅ | `let` | variable binding |
| `If` ✅ | `if` | conditional |
| `Else` ✅ | `else` | |
| `Return` ✅ | `return` | |
| `Fn` ✅ | `fn` | function expression |
| `For` 🔧 P5 | `for` | iteration |
| `In` 🔧 P5 | `in` | `for x in ...` |
| `Break` 🔧 P5 | `break` | loop early exit |
| `Continue` 🔧 P5 | `continue` | loop next iteration |
| `While` 🔧 P8 | `while` | loop (deferred — requires mutable bindings) |

> **Note:** `in` could remain an `Ident` and be recognised contextually by the parser
> (`for x <ident:"in"> ...`), avoiding a keyword reservation that blocks `in` as a
> variable name. Decision pending Phase 5.

### 1.3 Punctuation and structure tokens

| Token | Lexeme | Notes |
|-------|--------|-------|
| `LBrace` / `RBrace` ✅ | `{` `}` | block, component body |
| `LParen` / `RParen` ✅ | `(` `)` | call args, grouping |
| `LBracket` / `RBracket` ✅ | `[` `]` | array literal, index |
| `Comma` ✅ | `,` | |
| `Dot` ✅ | `.` | constructor call, method call |
| `Eq` ✅ | `=` | assignment, named body item |
| `Semicolon` ✅ | `;` | optional statement terminator |
| `Eof` ✅ | — | end of input |
| `DotDot` ❓ | `..` | range literal — only if range syntax added |

### 1.4 Arithmetic operator tokens

| Token | Lexeme | Notes |
|-------|--------|-------|
| `Plus` ✅ | `+` | add; also string concat |
| `Minus` ✅ | `-` | subtract; overloaded as unary negation |
| `Star` ✅ | `*` | multiply |
| `Slash` ✅ | `/` | divide |
| `Percent` ✅ | `%` | remainder |

### 1.5 Comparison operator tokens

| Token | Lexeme | Notes |
|-------|--------|-------|
| `EqEq` ✅ | `==` | equality |
| `BangEq` ✅ | `!=` | inequality |
| `Lt` ✅ | `<` | less than |
| `Gt` ✅ | `>` | greater than |
| `LtEq` ✅ | `<=` | less or equal |
| `GtEq` ✅ | `>=` | greater or equal |

### 1.6 Logical operator tokens

| Token | Lexeme | Notes |
|-------|--------|-------|
| `AmpAmp` ✅ | `&&` | logical and (short-circuit) |
| `PipePipe` ✅ | `\|\|` | logical or (short-circuit) |
| `Bang` ✅ | `!` | logical not (unary) |

### 1.7 Pipe and query operator tokens

| Token | Lexeme | Notes |
|-------|--------|-------|
| `PipeGt` ✅ | `\|>` | forward pipe — evaluates as `f(expr)`; no query meaning |
| `Arrow` ✅ | `->` | query/dispatch operator — lowest precedence infix operator |

---

## 2. Expression AST nodes

### 2.1 Literal expressions

| Node | Rust | Evaluates to |
|------|------|--------------|
| `Expression::String(String)` ✅ | `"hello"` | `Value::String` |
| `Expression::Number(f64)` ✅ | `3.14` | `Value::Number` |
| `Expression::Bool(bool)` ✅ | `true` / `false` | `Value::Bool` |
| `Expression::Null` ✅ | `null` | `Value::Null` |
| `Expression::Array(Vec<Expression>)` ✅ | `[1, 2, 3]` | `Value::Array` (eval each element) |

### 2.2 Name and reference expressions

| Node | Rust | Evaluates to |
|------|------|--------------|
| `Expression::Identifier(Ident)` ✅ | `x` | looks up `x` in env → the stored `Value` |

### 2.3 Call expressions

| Node | Rust | Evaluates to |
|------|------|--------------|
| `Expression::Call(CallExpression)` ✅ | `foo(a, b)` | return value of the called function |

`CallExpression` fields:

```rust
pub struct CallExpression {
    pub callee: Box<Expression>,
    pub args: Vec<Expression>,
}
```

For a plain function call, `callee` is typically:

```rust
Expression::Identifier(Ident("foo".into()))
```

For a method-style call on a value, `callee` is a normal expression rooted in the dot
operator:

```rust
Expression::BinaryOp {
    op: BinOpKind::Dot,
    lhs: Box::new(Expression::Identifier(Ident("obj".into()))),
    rhs: Box::new(Expression::Identifier(Ident("method".into()))),
}
```

So `obj.method(a, b)` is represented as a regular `CallExpression`, not as a separate
`Expression::MethodCall` AST node.

`emit(ce)` is a special-cased builtin. All other callees are looked up in the env as
`Value::Function` and called with the Pratt-evaluated args. Built-in functions (`range`)
are also dispatched here.

**Method-style call on a value** 🔧 P7 — `x.method(args)` where `x` is a
`Value::ComponentObject`.

This is distinct from:

- constructor chaining on a component type name (`T.position(...)`)
- body-time builder calls inside a component expression

But it does **not** need a separate AST node anymore. The parser already distinguishes:

- uppercase-leading `Type.method(args)[.method2(...)]` → `Expression::Component(...)`
- lowercase/value-leading `obj.method(args)` → `Expression::Call(...)` whose callee is a
  dot-expression

What is still deferred is the evaluator/runtime dispatch for dot-callee calls on live
values, not the surface syntax representation.

### 2.4 Component expression

| Node | Rust | Evaluates to |
|------|------|--------------|
| `Expression::Component(ComponentExpression)` ✅ | `T { ... }` | `Value::ComponentObject` (Phase 6) or `StoredValue::ComponentExpr` (v1) |

`ComponentExpression` fields: `component_type: Ident`, `constructor: Option<ConstructorCall>`,
`body: Vec<ComponentBodyItem>`.

See [component-expression-format.md](component-expression-format.md) for full grammar.

### 2.5 Function expression

```rust
Expression::Function {  // ✅
    params: Vec<Ident>,
    body: BlockStatement,
}
```

| Example | Evaluates to |
|---------|--------------|
| `fn(x, y) { x + y }` | `Value::Function { params, body, captured_env }` |

Named functions (`fn foo(args) { }`) desugar to `let foo = fn(args) { }` — no extra AST
node needed. See [functions-and-closures.md](../analysis/functions-and-closures.md).

### 2.6 Binary operator expression

```rust
// Phase 2 (Pipe added later)
Expression::BinaryOp {
    op: BinaryOpKind,
    lhs: Box<Expression>,
    rhs: Box<Expression>,
}

pub enum BinaryOpKind {
    // arithmetic
    Add, Sub, Mul, Div, Rem,
    // comparison
    Eq, NotEq, Lt, Gt, LtEq, GtEq,
    // logical (short-circuit)
    And, Or,
    // forward pipe — `expr |> f` evaluates as `f(expr)`; no query semantics
    Pipe, // ✅
    // query/dispatch operator — `"selector" -> handler` or `comp_obj -> "selector" -> handler`
    Query, // ✅ — always rewritten by QueryDesugarTransform before eval
    // dot operator — `obj.method` as the callee part of a CallExpression
    Dot, // ✅
}
```

`Query` nodes (from `->`) are **always** rewritten by `QueryDesugarTransform` into
`query()`/`query_all()` calls before the evaluator runs. `Pipe` nodes (from `|>`) are
evaluated directly as function application — `f(expr)`. See [script-runner.md](script-runner.md)
for the pipeline and [mms-query.md](../draft/mms-query.md) for the rewrite rules.

`Dot` nodes are regular expression structure. When followed by `(...)`, they appear as the
callee of a normal `Expression::Call`. They are not rewritten into a special method-call AST.

### 2.7 Unary operator expression

```rust
// Phase 2
Expression::UnaryOp {
    op: UnaryOpKind,
    operand: Box<Expression>,
}

pub enum UnaryOpKind {
    Neg,  // -x
    Not,  // !x
}
```

### 2.8 Index expression

```rust
// Phase 5 (needed alongside arrays + for)
Expression::Index {
    object: Box<Expression>,
    index: Box<Expression>,
}
```

| Example | Notes |
|---------|-------|
| `arr[0]` | zero-based; evaluates `object` → `Value::Array`, then indexes |
| `arr[i]` | `i` must evaluate to `Value::Number`; cast to `usize` |

> **Note:** Out-of-bounds index — runtime error or `Value::Null`? ❓ Decide in Phase 5.

### 2.9 If expression ❓

`if` is currently `Statement::If` (statement only). Whether `if` should also be usable
as an expression (`let x = if cond { 1 } else { 2 }`) is an open question.

If added:
```rust
// ❓ Phase 3 or later
Expression::If {
    condition: Box<Expression>,
    then_branch: BlockStatement,
    else_branch: Option<BlockStatement>,
}
```

> **Note:** If `if` is only a statement, `let x = if ...` is not valid syntax. This is
> a real ergonomics limitation. Recommendation: add `Expression::If` in Phase 3 alongside
> the evaluator implementation. Parser already produces `Statement::If`; extending to
> `Expression::If` is a small addition.

---

## 3. Operator precedence

Highest to lowest. All binary operators are left-associative unless noted.

Binding powers from `parser.rs` (`l_bp`, `r_bp`):

| Level | Operators | `(l_bp, r_bp)` | Notes |
|-------|-----------|----------------|-------|
| 9 | `!`, unary `-` | `(—, 17)` | right-associative (prefix unary) |
| 8 | `*`, `/`, `%` | `(14, 15)` | |
| 7 | `+`, `-` | `(12, 13)` | |
| 6 | `<`, `>`, `<=`, `>=` | `(10, 11)` | non-associative (chaining `a < b < c` is a parse error) |
| 5 | `==`, `!=` | `(8, 9)` | |
| 4 | `&&` | `(6, 7)` | |
| 3 | `\|\|` | `(4, 5)` | |
| 2 | `\|>` | `(2, 3)` | left-associative — `a \|> f \|> g` is `g(f(a))` |
| 1 | `->` | `(0, 1)` | lowest; query/dispatch operator |

Parentheses `(expr)` always have highest precedence.

> **Note:** Non-associativity for comparisons (`<`, `>`, etc.) prevents confusing
> `a < b < c` being silently parsed as `(a < b) < c` (comparing a bool to a number).
> This matches Python / Rust. Parsers typically implement this by tracking "last operator
> level" and erroring when two comparison operators appear at the same precedence level
> without parens.

---

## 4. Runtime `Value` types

Defined in `src/meow_meow/object.rs`. These are the values that exist at evaluation time,
stored in the `ObjectWorld` env and passed between expressions.

| Variant | Status | Notes |
|---------|--------|-------|
| `Value::Null` | ✅ | `null` literal, missing values |
| `Value::Bool(bool)` | ✅ | `true` / `false` |
| `Value::Number(f64)` | ✅ | single numeric type; cast to `f32`/`usize` at component boundary |
| `Value::String(String)` | ✅ | `"..."` |
| `Value::Array(Vec<Value>)` | ✅ current implementation | Inline array storage with value semantics today. This is now considered transitional: see [../task/heap-backed-arrays-and-single-objectworld-heap.md](../task/heap-backed-arrays-and-single-objectworld-heap.md). The intended direction is heap-backed arrays with stable identity across closures/function calls. |
| `Value::Identifier(String)` | ✅ | bare symbolic flag (e.g. `Left`, `Aim`) — kept distinct from `String` so enum-like identifiers survive to the component registry |
| `Value::ComponentExpr(Box<ComponentExpression>)` | ✅ pre-P6 | unresolved component expression; placeholder until Phase 6 live reply channel |
| `Value::ComponentObject(ComponentId)` | 🔧 P6 | live engine component (unattached); replaces `ComponentExpr` once reply channel exists |
| `Value::Function { params, body, captured_env }` | ✅ | closure; `captured_env` is a snapshot of the env at definition time |
| `Value::Object(ObjectId)` | struct only | heap-allocated map/record; `ObjectId` indexes into `Heap`; not yet creatable from MMS syntax — reserved for future record literals |

> **Note on `Value::Object`:** no MMS syntax creates an `Object` yet. The `{` token is
> overloaded for component bodies — object literal syntax would need disambiguation
> (e.g. `#{ key: value }` vs component body `{ key = value }`). ❓ open question.
>
> **Array storage note:** the runtime still uses inline `Value::Array(Vec<Value>)`, but that
> should no longer be read as the target memory model. The planned direction is to move arrays
> into the `ObjectWorld` heap and keep one heap per running MMS session so indexed mutation and
> closure-visible shared array identity work correctly.

---

## 5. `StmtEffect` — evaluator unwind signals

Internal enum used by the evaluator to propagate early exits through block evaluation.

| Variant | Added | Purpose |
|---------|-------|---------|
| `None` | P1 | statement had no special effect |
| `Bind(String, Value)` | P1 | `let` binding to insert into env |
| `Return(Value)` | P4 ✅ | unwind call frame; value is the function's return |
| `Break` | 🔧 P5 | exit enclosing `for` loop |
| `Continue` | 🔧 P5 | skip to next iteration of enclosing `for` loop |
