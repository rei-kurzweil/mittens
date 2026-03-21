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
| `Fn` 🔧 P4 | `fn` | function expression |
| `For` 🔧 P5 | `for` | iteration |
| `In` 🔧 P5 | `in` | `for x in ...` |
| `While` 🔧 P8 | `while` | loop |
| `Break` 🔧 P8 | `break` | ❓ needed for `while`/`loop` |
| `Continue` 🔧 P8 | `continue` | ❓ same |

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
| `Plus` 🔧 P2 | `+` | add; also string concat |
| `Minus` 🔧 P2 | `-` | subtract; overloaded as unary negation |
| `Star` 🔧 P2 | `*` | multiply |
| `Slash` 🔧 P2 | `/` | divide |
| `Percent` 🔧 P2 | `%` | remainder |

### 1.5 Comparison operator tokens

| Token | Lexeme | Notes |
|-------|--------|-------|
| `EqEq` 🔧 P2 | `==` | equality |
| `BangEq` 🔧 P2 | `!=` | inequality |
| `Lt` 🔧 P2 | `<` | less than |
| `Gt` 🔧 P2 | `>` | greater than |
| `LtEq` 🔧 P2 | `<=` | less or equal |
| `GtEq` 🔧 P2 | `>=` | greater or equal |

> **Note:** `<` and `>` are not currently used anywhere in MMS syntax (component type
> names use plain idents, not generics). No ambiguity.

### 1.6 Logical operator tokens

| Token | Lexeme | Notes |
|-------|--------|-------|
| `AmpAmp` 🔧 P2 | `&&` | logical and (short-circuit) |
| `PipePipe` 🔧 P2 | `\|\|` | logical or (short-circuit) |
| `Bang` 🔧 P2 | `!` | logical not (unary) |

> **Note:** `!` is unambiguous since MMS has no `!=` as a single token scan — `!` is
> consumed first, then `=` check follows. Handle in lexer as: if `!` then peek next;
> if `=` emit `BangEq`, otherwise emit `Bang`.

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

> **Note:** `Expression::Array` is parsed but not yet evaluated — the evaluator returns
> `StoredValue::Primitive` as a placeholder. Full evaluation lands in Phase 2 alongside
> `eval_expr()`.

### 2.2 Name and reference expressions

| Node | Rust | Evaluates to |
|------|------|--------------|
| `Expression::Identifier(Ident)` ✅ | `x` | looks up `x` in env → the stored `Value` |

> **Note:** The evaluator currently special-cases `Identifier` only to check if it holds a
> `ComponentExpr` for Option B emission. Phase 2 generalises this to full `eval_expr()`.

### 2.3 Call expressions

| Node | Rust | Evaluates to |
|------|------|--------------|
| `Expression::Call(CallExpression)` ✅ | `foo(a, b)` | return value of the called function |

`CallExpression` fields: `callee: Ident`, `args: Vec<Expression>`.

Current evaluator only handles `callee == "emit"` specially and ignores all other calls.
Phase 2/4 generalises this.

**Method call on a value** 🔧 P7 — `x.method(args)` where `x` is a `Value::ComponentObject`.
This is distinct from `ComponentBodyItem::Call` (which is constructor-time) and from
`ConstructorCall` (which is the `.method(args)` immediately after a component type name).
Needs a new AST node:

```rust
// Phase 7
Expression::MethodCall {
    receiver: Box<Expression>,
    method: Ident,
    args: Vec<Expression>,
}
```

> **Note:** `foo.bar(args)` currently parses as a component expression `foo` with
> constructor `bar` — the parser can't yet disambiguate value method calls from component
> constructor calls. Phase 7 needs to resolve this, likely by checking whether `foo` is
> known to be a component type name or a variable name.

### 2.4 Component expression

| Node | Rust | Evaluates to |
|------|------|--------------|
| `Expression::Component(ComponentExpression)` ✅ | `T { ... }` | `Value::ComponentObject` (Phase 6) or `StoredValue::ComponentExpr` (v1) |

`ComponentExpression` fields: `component_type: Ident`, `constructor: Option<ConstructorCall>`,
`body: Vec<ComponentBodyItem>`.

See [component-expression-format.md](component-expression-format.md) for full grammar.

### 2.5 Function expression

```rust
// Phase 4
Expression::Function {
    params: Vec<Ident>,
    body: BlockStatement,
}
```

| Example | Evaluates to |
|---------|--------------|
| `fn(x, y) { x + y }` | `Value::Function { params, body, captured_env }` |

> **Note:** Named functions (`fn foo(args) { }`) are `let foo = fn(args) { }` with
> no additional AST node — the `let` statement + `Expression::Function` covers both.
> See [functions-and-closures.md](../analysis/functions-and-closures.md).

### 2.6 Binary operator expression

```rust
// Phase 2
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
}
```

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

| Level | Operators | Notes |
|-------|-----------|-------|
| 6 | `!`, unary `-` | right-associative (prefix unary) |
| 5 | `*`, `/`, `%` | |
| 4 | `+`, `-` | |
| 3 | `<`, `>`, `<=`, `>=` | non-associative (chaining `a < b < c` is a parse error) |
| 2 | `==`, `!=` | |
| 1 | `&&` | |
| 0 | `\|\|` | lowest |

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

| Variant | Rust | Notes |
|---------|------|-------|
| `Value::Null` ✅ | `null` literal, missing values | |
| `Value::Bool(bool)` ✅ | `true` / `false` | |
| `Value::Number(f64)` ✅ | all numerics | single numeric type; casts to `f32`/`usize` at component boundary |
| `Value::String(String)` ✅ | `"..."` | |
| `Value::Array(Vec<Value>)` ✅ | `[...]` | heap-allocated in Rust; value-semantics (clone) for now |
| `Value::Identifier(String)` ✅ | bare symbolic flag | preserved as distinct from `String` so enum-like identifiers (`Left`, `Aim`) survive to the component registry |
| `Value::Object(ObjectId)` ✅ | `{ key: val }` | heap-allocated map; `ObjectId` is an index into `Heap`; not yet creatable from MMS syntax |
| `Value::ComponentObject(ComponentId)` ✅ (struct) 🔧 live (P6) | `T { }` when fully live | currently `StoredValue::ComponentExpr` in evaluator v1; becomes a real `ComponentId` in Phase 6 |
| `Value::Function { params, body, captured_env }` 🔧 P4 | `fn(x) { ... }` | closure; `captured_env` is a snapshot of the env at definition time |

> **Note on `StoredValue` vs `Value`:** The evaluator currently uses its own internal
> `StoredValue` enum (`ComponentExpr`, `Primitive`) as a v1 placeholder. `Value` in
> `object.rs` is the designed target. Phase 2 migrates the evaluator to use `Value`
> directly and removes `StoredValue`.

> **Note on `Value::Identifier`:** this exists to pass bare identifier tokens (e.g.
> `Left`, `Aim`, `Primary`) through to component constructors that expect enum variants.
> Once MMS has a type system, these would become typed enum values. For now, the registry
> treats `Value::Identifier("Left")` the same as the string `"Left"` but can distinguish
> it from a user-supplied `String("Left")` if needed.

> **Note on `Value::Object`:** no MMS syntax creates an `Object` yet. Reserved for when
> record/map literals are added (`{ key: value, ... }`). The `{` token is currently
> overloaded for component bodies — object literal syntax would need disambiguation
> (probably: `{ key = value }` for component body items vs `#{ key: value }` for object
> literals, or require a constructor to construct records). ❓ open question.

---

## 5. Evaluator internal types (v1 → target migration)

The evaluator in `evaluator.rs` uses a simplified internal representation that does not
match `Value` yet. This table tracks the migration:

| v1 `StoredValue` | Target `Value` | Migrates in |
|------------------|----------------|-------------|
| `ComponentExpr(Box<ComponentExpression>)` | `ComponentObject(ComponentId)` | Phase 6 (reply channel) |
| `Primitive(String)` | `Bool` / `Number` / `String` / `Null` | Phase 2 |

The `StmtEffect` enum (`None`, `Emit`, `Bind`) will also need to grow:

| v1 `StmtEffect` | Target | Added in |
|-----------------|--------|----------|
| `None` | same | — |
| `Emit(IntentValue)` | same | — |
| `Bind(String, StoredValue)` | `Bind(String, Value)` | Phase 2 |
| — | `Return(Value)` | Phase 4 (needed for function return unwind) |
| — | `Break` / `Continue` | Phase 5/8 |
