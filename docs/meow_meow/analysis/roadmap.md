# MMS development roadmap

Phases are ordered by dependency. Each phase has a checklist and flags open design
decisions (🔷) that need a doc or a decision before implementation.

---

## Phase 1: Core component expressions ✅ DONE

- [x] Tokenizer (`Let`, `If`, `Else`, `Return`, `True`, `False`, `Null`, literals, idents)
- [x] Parser: component expressions, `let` bindings, `if`/`else`, `return`, blocks
- [x] AST: `Statement`, `Expression`, `ComponentExpression`, `ComponentBodyItem`
- [x] `EmitLiftTransform` — bare CE in statement position → `emit(ce)` call
- [x] Evaluator thread + ring buffer protocol (`EvalRequest` / `EvalResponse`)
- [x] `StoredValue` — `let x = T { }` stores the `ComponentExpression`
- [x] `SpawnComponentTree` intent + `RxIntentExecutor` handler
- [x] Component registry — 30+ component types
- [x] `MeowMeowRunner` — synchronous intent collection helper
- [x] `vr-input-mms.rs` end-to-end scene spawn

**Known gaps from Phase 1 (carry-forward):**
- [ ] Emit context stack — function calls inside component bodies should emit as children,
      not world roots. Gated on Phase 5 (reply channel).
      → [emission-and-component-value-model.md](emission-and-component-value-model.md)
- [ ] `let x = T { }` stores `ComponentExpression`, not a live `ComponentId`.
      Gated on Phase 5.

---

## Phase 2: Expression evaluation (arithmetic, boolean, comparison)

**What's missing from tokenizer:**

- [ ] Arithmetic: `+`, `-`, `*`, `/`, `%`
- [ ] Comparison: `==`, `!=`, `<`, `>`, `<=`, `>=`
- [ ] Logical: `&&`, `||`, `!`
- [ ] Unary minus: `-x`

**What's missing from AST:**

- [ ] `Expression::BinaryOp { op, lhs, rhs }`
- [ ] `Expression::UnaryOp { op, operand }`

**What's missing from evaluator:**

- [ ] `eval_expr()` — recursive expression evaluator, returns a runtime `Value`
- [ ] Arithmetic on `Value::Number`
- [ ] Comparison returning `Value::Bool`
- [ ] Logical ops on `Value::Bool`

**Checklist:**
- [ ] Add operator tokens to `token.rs`
- [ ] Add `BinaryOp` / `UnaryOp` to `Expression` in `ast/expression.rs`
- [ ] Parse binary/unary expressions with correct precedence in `parser.rs`
- [ ] Implement `eval_expr()` in `evaluator.rs` (replaces the current pattern-matched
      handful of expression cases)
- [ ] Tests: arithmetic, comparison, boolean, precedence

🔷 **Design decision: number types**
Currently `Expression::Number(f64)`. Should MMS distinguish integers from floats?
Many component fields take `f32`, some take `usize` (counts). Options: keep everything
`f64` and cast at boundary; add `Int(i64)` and `Float(f64)` variants; infer from context.
→ needs a short decision note (no existing doc)

🔷 **Design decision: operator precedence table**
Standard (PEMDAS + C-style logical)? Or explicit parens required for mixing arithmetic
and logical? → add to a future `expression-evaluation.md`

---

## Phase 3: `if`/`else` evaluation

**What the parser already has:**
- `Statement::If(IfStatement)` with `condition`, `then_branch`, `else_branch` ✅
- `Statement::Block(BlockStatement)` ✅

**What the evaluator currently does:**
- `Statement::If(_) | Statement::Block(_) => Ok(StmtEffect::None)` — ignored

**Checklist:**
- [ ] Implement `eval_stmt` arm for `Statement::If` — evaluate condition, pick branch,
      evaluate chosen block statements
- [ ] Implement `eval_stmt` arm for `Statement::Block` — evaluate inner statements
- [ ] `if` without `else` — condition false → `Value::Null`
- [ ] Tests: `if true { T { } }`, `if false { T { } }`, `if/else` branching,
      `if` with a let-bound condition variable

🔷 **Design decision: `if` as expression vs statement**
Parser currently produces `Statement::If`. Should `if` also be usable as an expression
(returning the value of the taken branch)? This affects `let x = if cond { T { } } else { R { } }`.
No existing doc — needs a decision.

---

## Phase 4: Functions

**What's missing from tokenizer:**
- [ ] `fn` keyword token

**What's missing from AST:**
- [ ] `Expression::Function { params: Vec<Ident>, body: BlockStatement }`
      (anonymous; named functions are `let f = fn(...) { }`)

**What's missing from evaluator:**
- [ ] `StoredValue::Closure { params, body, captured_env }` (or non-capturing function)
- [ ] `eval_call()` — look up callee in env as `StoredValue::Closure`, bind args, eval body
- [ ] Scope chain for function calls — currently a flat `HashMap`; needs push/pop on call

**Checklist:**
- [ ] Add `Fn` token to `token.rs` and `tokenizer.rs`
- [ ] Add `Expression::Function` to AST
- [ ] Parse `fn(params) { body }` in `parser.rs`
- [ ] Add `StoredValue::Closure` to evaluator
- [ ] Implement `eval_call()` with a scope frame pushed for the call
- [ ] `EmitLiftTransform` must recurse into function bodies (already planned; confirm it does)
- [ ] `return` statement evaluation — unwind call frame, return value to caller
- [ ] Tests: define + call a function, function returning a CE, function with args,
      recursive function (stretch)

🔷 **Design decision: closures vs plain functions**
Do MMS functions close over their lexical environment, or only see their arguments?
Full closures require capturing the env at definition time. Plain functions (no capture)
are simpler to implement and may be sufficient for authoring use cases.
→ needs `functions-and-closures.md`

🔷 **Design decision: scope rules**
Currently flat `HashMap<String, StoredValue>`. Phase 4 needs at minimum a call-frame
scope (push on call, pop on return). Should `let` inside an `if` branch be visible
outside it (dynamic/flat scoping) or go out of scope with the block (lexical scoping)?
Lexical is correct but requires a scope chain.
→ needs `functions-and-closures.md`

🔷 **Design decision: named functions vs `let f = fn(...)`**
Named function syntax (`fn foo(args) { }`) is syntactic sugar for `let foo = fn(args) { }`.
Does MMS need both forms or just the `let` form? The `let` form is more uniform but
`fn foo` reads better for top-level definitions.

🔷 **Design decision: return type annotations**
`fn(x: f32) -> ComponentObject { ... }` enables the typed emission policy (Option D).
v1 doesn't need this but the AST should leave room for it.
→ [emission-policy-options.md](emission-policy-options.md)

---

## Phase 5: Arrays and `for` loops

**What's missing from tokenizer:**
- [ ] `for` keyword token
- [ ] `in` keyword token (or treat as ident?)

**What's missing from AST:**
- [ ] `Statement::ForIn { binding: Ident, iterable: Expression, body: BlockStatement }`

**What already exists:**
- `Expression::Array(Vec<Expression>)` — in AST ✅
- `TokenKind::LBracket` / `RBracket` — in tokenizer ✅
- Array literal parsing — check parser (likely partial)

**What's missing from evaluator:**
- [ ] `Value::Array(Vec<Value>)` — currently `StoredValue::Primitive` for everything
      non-component
- [ ] `eval_expr` for `Expression::Array` → evaluate each element
- [ ] Iterate `Value::Array` in `Statement::ForIn`
- [ ] `range(n)` builtin or `0..n` syntax for numeric iteration

**Checklist:**
- [ ] Add `For`, `In` tokens
- [ ] Add `Statement::ForIn` to AST
- [ ] Parse `for x in expr { body }`
- [ ] Implement array eval + for-in eval
- [ ] Tests: `for x in [1, 2, 3]`, emit in a loop, nested for

🔷 **Design decision: `range()` builtin vs `0..n` syntax**
Numeric iteration is essential (`for i in range(10)`). Options: `range(n)` builtin
returning an array-like; `0..n` range syntax (needs new `DotDot` token and `Range`
expression); `[0, 1, 2, ...]` is unwieldy. Most likely: `range(n)` as a builtin first.

🔷 **Design decision: `break`/`continue`**
Needed for useful loops. Requires unwind mechanism through the evaluator (similar to
`return`). Adds `Break`/`Continue` tokens and statement variants. Can be deferred past
initial for-loop support but should be planned.

---

## Phase 6: Live `ComponentId` reply channel

**The gap:** `let x = T { }` stores a `ComponentExpression` (an AST snapshot), not a
live engine component. Scripts cannot reference spawned components after binding them.

**What's needed:**
- [ ] Reply channel from main thread → evaluator thread (currently one-directional)
- [ ] `SpawnComponentTree` response carries the assigned `ComponentId`
- [ ] `StoredValue::ComponentObject(ComponentId)` (replaces `ComponentExpr` for live handles)
- [ ] Emit context stack — body-scoped calls emit as children of the enclosing component

**Checklist:**
- [ ] Design the reply channel (second ring buffer, or extend `EvalResponse`)
- [ ] `SpawnComponentTree` executor echoes the root `ComponentId` back
- [ ] Evaluator receives the ID and upgrades `StoredValue::ComponentExpr` → `ComponentObject`
- [ ] Emit context stack implemented in evaluator

🔷 **Design decision: reply channel shape**
`SpawnComponentTree` currently fires-and-forgets. To get the `ComponentId` back:
- Option A: add `EvalResponse::Spawned { id: ComponentId }` — evaluator polls for it
- Option B: a second dedicated reply ring buffer alongside the response channel
- Option C: synchronous — evaluator blocks until the main thread processes the intent
  (breaks the async model but simplest)
→ [object-world.md](object-world.md) has partial discussion; needs a dedicated decision doc

---

## Phase 7: `ComponentObject` mutation API

Depends on Phase 6 (live `ComponentId`).

**What's needed:**
- [ ] Method call on a `ComponentObject` value: `x.set_color(1, 0, 0, 1)`
- [ ] MMS-side dispatch: `ComponentObject` method name → `IntentValue`
- [ ] A registry of per-component-type mutation methods (parallel to the constructor registry)

**Checklist:**
- [ ] Parse `expr.method(args)` as `Expression::MethodCall` (new AST node, distinct from
      `ComponentBodyItem::Call` which is constructor-time)
- [ ] Evaluate `MethodCall` on `Value::ComponentObject` → emit the appropriate intent
- [ ] Tests: spawn, mutate, verify intent emitted

🔷 **Design decision: mutation method registry vs intent literals**
Should `x.set_color(...)` be a hard-coded dispatch table (like the constructor registry),
or should MMS be able to emit arbitrary `IntentValue`s directly (the `intent()` builtin
discussed in [signal-emission-in-mms.md](signal-emission-in-mms.md))?

---

## Phase 8: `while` and `loop`

Lower priority than `for`. Depends on Phase 4 (functions) and Phase 2 (boolean eval).

- [ ] `While` token
- [ ] `Statement::While { condition, body }`
- [ ] Parser + evaluator for while
- [ ] `loop { }` (infinite loop with `break`) — optional
- [ ] Tests

---

## Phase 9: Module / import system

Long-horizon. Required before `.mms` files can import shared helpers.

- [ ] `import "path/to/file.mms"` syntax
- [ ] Export mechanism (`pub let` or explicit `export`)
- [ ] Module resolution (relative paths, asset system integration)

🔷 **Design decision: import semantics**
Does importing a file *run* it (side effects) or just bind its exports? In most authoring
contexts you want to import helpers without triggering scene construction. Needs a doc.

---

## Phase 10: Type annotations (Option D emission)

Enables the typed emission policy described in
[emission-policy-options.md](emission-policy-options.md). Allows the compiler to know
statically which functions return `ComponentObject` vs other values, enabling deterministic
emission without the runtime Option B check.

- [ ] Type annotation syntax on function params and return types
- [ ] `ComponentObject` as a named type in MMS
- [ ] Static analysis pass (or just a type-checking evaluator mode)
- [ ] Update `EmitLiftTransform` to use static return type info when available

---

## Dependency graph

```
Phase 1 (done)
    └── Phase 2 (expression eval)
            ├── Phase 3 (if/else)
            │       └── Phase 4 (functions)
            │               └── Phase 5 (arrays + for)
            │                       └── Phase 8 (while)
            └── Phase 6 (reply channel)
                    └── Phase 7 (mutation API)

Phase 9 (modules) — depends on Phase 4
Phase 10 (types) — depends on Phase 4
```

**Immediate next: Phase 2**, then Phase 3, then Phase 4. These are the three that unblock
`let`/`if`/function tests and make MMS useful as a scripting language rather than just a
scene description format.
