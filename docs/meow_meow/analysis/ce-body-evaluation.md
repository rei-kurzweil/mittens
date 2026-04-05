# ₊˚ʚ CE Body Evaluation — Design Analysis

Analysis of the current split-evaluation model for component expression bodies and the
path to full MMS language support inside them.

---

## The core insight: a CE body is a closure

A CE body is structurally identical to a closure. It closes over the outer env, it's a
`BlockStatement`, and the "receiver" (the component being built) is just the implicit
context — exactly like `self` in a method, or a `with` block in other languages.

```mms
T.position(x, y, 0) {
    let hue = compute_hue(team)
    C.rgba(hue[0], hue[1], hue[2], 1.0)
    scale(0.9, 0.9, 0.9)
}
```

This is equivalent to:

```
(new CE builder for T with ctor args [x, y, 0])
eval body as BlockStatement with:
    env     = outer env + T's builder methods (lower priority)
    emits   = &mut builder.children          ← the only real difference
```

Two things change relative to a regular block evaluation:

1. **`ctx.emits` points at the CE builder's children** instead of the top-level emit
   vec — so CE emissions inside the block become children of this node, not global
   spawns.

2. **Builder methods are injected into the env** — `scale`, `rgba`, `speed`, etc. are
   added as `Value::Function` bindings for the duration of the block. They close over a
   mutable ref to the builder and record their call when invoked. Normal env lookup
   priority applies: a user-defined `scale` would shadow the builder's.

Everything else — `let`, `if`, `for`, function calls, `while`, closures, imports — runs
through the existing eval machinery unchanged.

---

## The current problem

Right now CE bodies are `Vec<ComponentBodyItem>` AST nodes, not `BlockStatement`. They
are passed to `component_registry::spawn_tree` on the **main thread**, which can only
evaluate literals via `eval_literal`. Split-brain:

```
Evaluator thread                       Main thread (registry)
────────────────────────               ──────────────────────
subst_body_items:                      eval_literal(expr):
  expands if/for only    ──► CE ──►      handles literals only
  leaves other exprs as-is              fails: variables, calls,
                                        arithmetic, etc.
```

The `Vec<ComponentBodyItem>` shape and the `eval_literal` on the main thread are both
consequences of the same root assumption: that CE bodies are data structures to be
interpreted by the registry, not code to be evaluated by the evaluator.

---

## The fix: CE body = BlockStatement evaluated by the evaluator

### AST change

```rust
// Before
struct ComponentExpression {
    component_type: Ident,
    constructor: Option<ConstructorCall>,
    body: Vec<ComponentBodyItem>,
}

// After
struct ComponentExpression {
    component_type: Ident,
    constructor: Option<ConstructorCall>,
    body: BlockStatement,   // ← same type as function body, if body, etc.
}
```

`ComponentBodyItem` goes away as a separate AST node type. The parser produces a
`BlockStatement` for the body, exactly as it does for `fn`, `if`, and `for`.

### Evaluator change

`eval_expr` for `Expression::Component(ce)`:

1. Evaluate constructor args against the current env — just `eval_expr` each arg, no
   special-casing.
2. Create a `CeBuilder { type_name, ctor_args, children: vec![], calls: vec![] }`.
3. Inject builder methods into a child env: for each known method of this component
   type, add a `Value::Function` that records the call into the builder.
4. Evaluate the body block with:
   - `env = child env` (captures outer scope + builder methods)
   - `ctx.emits = &mut builder.children` (redirects CE emissions to parent)
5. After the block, materialize the builder into a `ComponentExpression` with concrete
   literal values only.
6. Return `Value::ComponentExpr(Box::new(materialized_ce))` as before.

### What the registry receives

After this change, `spawn_tree` receives a `ComponentExpression` where every value is
already a concrete literal — the same invariant `eval_literal` was trying to enforce,
but now guaranteed structurally. `eval_literal` and `subst_body_items` are both deleted.
The registry becomes purely a "take concrete spec, build components" layer.

### Nested CEs

Each nested CE evaluation pushes its own `CeBuilder` and redirects `ctx.emits`
temporarily. On exit, restores the parent's `ctx.emits`. The nesting is handled
naturally by the eval call stack — no explicit builder stack needed, just the normal
Rust call stack.

```mms
T {
    for x in range(3) {        // normal for loop — uses existing eval machinery
        T.position(x, 0, 0) {  // nested CE — pushes new CeBuilder, redirects emits
            R.cube()            // child CE — captured by inner builder
        }
    }
    // three child T CEs in outer builder's children
}
```

---

## Mapping: ComponentBodyItem → BlockStatement

Every current `ComponentBodyItem` variant needs a block-statement equivalent. Four map
cleanly; two are open questions.

### `If` / `For` — direct mapping

```
ComponentBodyItem::If  { ... }  →  Statement::If(...)
ComponentBodyItem::For { ... }  →  Statement::ForIn { ... }
```

These are identical. The block statement versions already exist in the AST and evaluator.
No new mechanism needed.

### `Child(CE)` — CE emission redirect

```
ComponentBodyItem::Child(ce)  →  Statement::Expression(Expression::Component(ce))
```

CE expression statements already evaluate and emit. The only change is what *emit*
means inside a CE body: instead of `ctx.emits` (the top-level intent queue), emissions
go to the current CE builder's children list. This is the `ctx.emits` redirect
described above — one context change, not a new AST node.

### `Call(method, args)` — builder methods in env

```
ComponentBodyItem::Call(CallExpression { callee, args })
  →  Statement::Expression(Expression::Call(CallExpression { callee, args }))
```

A bare `scale(0.9, 0.9, 0.9)` call. In the block, it resolves as a normal function
call. Builder methods for the current component type are pre-injected into the block's
env. Normal env lookup priority: user-defined `scale` shadows the builder's version.

### `NamedAssignment { name, value }` — open question

`intensity = 0.9` in a CE body means "set the intensity property on this component."
In a block statement, `x = expr` (no `let`) is `Statement::Reassign` — it requires `x`
to already be bound in scope, otherwise it's an error.

Three options:

**Option A: Drop it.** Named assignments become method calls: `intensity(0.9)` instead
of `intensity = 0.9`. The `=` form is v1 sugar that doesn't survive the transition to
full block evaluation. Simple, no ambiguity.

**Option B: Fallthrough.** If `name` is not in scope as a variable, `name = expr` is
treated as a builder call `name(expr)`. Implicit, fragile — whether `x = 5` assigns a
variable or sets a property depends on what's in scope at runtime.

**Option C: Explicit `self`.** The component being built is available in the body as
`self`. Property assignment is `self.intensity = 0.9`. Explicit, unambiguous, consistent
with how method calls work on live ComponentObjects. Changes the CE body syntax.

Current lean: **Option A** — named assignments aren't widely used and the method-call
form is already supported. Deferring to the open questions section.

### `Positional(expr)` — expression capture rule

`CUBE`, `"hello text"`, `[1, 0, 0, 1]` — bare expressions whose evaluated value is the
argument, not a side effect. In a regular block, expression statements are evaluated and
their values discarded. In a CE body block, these need to be captured.

Proposed rule: in a CE body block, an expression statement whose result is
`Value::Identifier | Value::String | Value::Number | Value::Array` is captured as a
positional arg by the CE builder. `Value::ComponentExpr` / `Value::ComponentObject`
goes to children (as above). `Value::Null` and `Value::Function` are discarded.

This rule is applied at the CE body's expression-statement handler, not in `eval_expr`
itself. Regular blocks outside CE context are unaffected.

---

## Ordering: lexical source order, unified

In the `BlockStatement` model, builder calls, child emissions, and intermediate MMS
code are all statements in the same sequence. They execute in source order. There is no
separate "apply builder calls first, then attach children" phase — the CE builder
records everything in the order produced by block evaluation.

```mms
T {
    let base = 0.5
    scale(base, base, 1.0)       // builder call — recorded 1st
    C.rgba(base, 0, 0, 1.0)      // child emission — 1st child
    if show_mesh { R.cube() }    // child emission — 2nd child (conditional)
    scale(1.0, 1.0, 1.0)         // builder call — recorded 2nd
}
```

The registry receives the calls and children in that interleaved order and applies them
sequentially. For most component properties this is commutative; for positional args and
ordered children it is not, and source order is the correct semantic.

This also means positional body items (`Text { "hello"; "world" }`, `CUBE`, etc.) are
just expressions in statement position whose evaluated value is captured by the CE
builder. They interleave with other statements the same way any expression statement
does.

---

## Disambiguation: builder call vs. free call

`scale(0.9, 0.9, 0.9)` inside a CE body: is it a builder method or a user function?

Normal env lookup resolves it. Builder methods are injected into the env at lower
priority. If the user has defined `fn scale(x, y, z) { ... }` in their script, their
version takes precedence. Otherwise the builder's version fires.

No special disambiguation logic needed — standard env shadowing handles it.

---

## What happens to `subst_body_items` / `subst_ce`

Both are deleted. The work they did (expanding `if`/`for`, evaluating expressions
against the current env) is now just normal block evaluation. The `if`/`for` nodes in
the CE body are `Statement::If` and `Statement::ForIn` — the same AST nodes used
everywhere else, handled by the same `eval_if` / `ForIn` arms in `eval_stmt`.

---

## Current state vs. end state

| Feature | Current | After this change |
|---------|---------|---------|
| `if`/`for` in CE body | ✓ (special-cased) | ✓ (free, via normal eval) |
| Variables in constructor args | ✗ | ✓ |
| Variables in call/assignment args | ✗ | ✓ |
| Arithmetic in CE body | ✗ | ✓ |
| Function calls producing children | ✗ | ✓ |
| Closures emitting children | ✗ | ✓ |
| `while`, `break`, `continue` in body | ✗ | ✓ |
| Full MMS in CE body | ✗ | ✓ |
| `eval_literal` / `subst_body_items` | required | deleted |

---

## Open questions

| Question | Stakes |
|----------|--------|
| `NamedAssignment`: drop in favour of method calls, fallthrough, or explicit `self`? | Syntax compatibility; Option A (drop) is simplest |
| `Positional` capture rule: capture Identifier/String/Number/Array from expr stmts, or require explicit syntax? | `CUBE`, `"text"` as positionals; implicit capture is convenient but magic |
| `ctx.emits` redirect: swap the pointer or add a `ce_builder` field to `EvalContext`? | API shape; builder field is more explicit and avoids type erasure |
| Can a CE body `return`? | Probably not — no value to return to; treat as an error or no-op |
| Builder method registry: static per component type, or dynamic? | Needs to be accessible at eval time (before spawn); static list is sufficient |
| Can `emit()` inside a CE body escape to the outer scope? | Escape hatch for "emit sibling" patterns — probably not in v1 |
