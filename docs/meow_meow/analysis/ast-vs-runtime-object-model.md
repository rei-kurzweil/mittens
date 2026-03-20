# AST vs runtime object model

Meow Meow has (at least) two distinct "shapes" of data:

1. **AST (syntax)**: what the parser produces.
2. **Runtime values**: what evaluation produces and manipulates.

This document sketches the split so the two layers stay well-separated.

---

## AST (current)

The parser/tokenizer lives in `src/meow_meow/`.

- Tokenizer: produces `Token { kind, span }`.
- Parser: produces `Vec<Statement>`.

Key AST types in `src/meow_meow/ast/`:

```rust
// A component expression as it appears in source
pub struct ComponentExpression {
    pub component_type: Ident,
    pub constructor: Option<ConstructorCall>,  // .method(args) before the body
    pub body: Vec<ComponentBodyItem>,           // in-source order: assignments, calls, children, positionals
}

// Body item kinds — source order preserved
pub enum ComponentBodyItem {
    NamedAssignment { name: Ident, value: Expression },
    Call(CallExpression),
    Child(ComponentExpression),
    Positional(Expression),
}
```

The AST mirrors the authoring model closely. It is what you see in the source file, represented
as a tree. Spans and source positions live here.

### AstTransforms

Between parsing and evaluation, **AstTransforms** restructure the AST without changing
semantics. The most important one is `EmitLiftTransform`, which desugars free-standing
component expression statements into explicit `emit(...)` calls:

```
Statement::Expression(Expression::Component(ce))
    →  Statement::Expression(Expression::Call { callee: "emit", args: [Component(ce)] })
```

No new `Statement` variant is needed. After the transform, `T { }` as a bare statement and
`emit(T { })` written explicitly are identical in the AST. The evaluator handles both via the
normal `Statement::Expression` path. See [emission semantics](emission-and-component-value-model.md)
for the full rationale.

---

## Runtime value model

Even compiling directly to engine components, the scripting language needs a runtime value
model for:

- evaluated literals (`"hi"`, `123`, `true`, `null`)
- arrays (`[1, 2, 3]`)
- `ComponentObject` handles (see below)
- later: closures, modules, etc.

```rust
pub enum Value {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<Value>),
    ComponentObject(ComponentId),   // ← live, unattached engine component
    // future: Closure, Handle, ...
}
```

---

## `ComponentObject`: the runtime handle for component expressions

When a component expression is **captured** (not emitted) — appearing as the RHS of a `let`
binding, a `return` value, a function argument, or inside an array — it evaluates to a
`ComponentObject`.

A `ComponentObject` is **not** an inert AST snapshot. It is a live handle to a `ComponentId`:
the component has been created in the engine but is unattached (no parent, not a world root).
Through the handle, MMS code can issue mutations back to the main thread and later emit
(attach) the component.

The key distinction:

| | AST | Runtime |
|---|---|---|
| Type | `ComponentExpression` | `Value::ComponentObject(ComponentId)` |
| Lives in | Parser output / `EmitLiftTransform` input | Evaluator heap |
| Engine state | None — pure data | Component exists in world (unattached) |
| Used for | Structural analysis, transforms, printing | Evaluation, mutation API, emission |

The AST describes *what to do*. The runtime `ComponentObject` is *what exists* after doing it.

---

## Proposed layering

```
.mms source
    ↓ tokenizer → parser
Vec<Statement>  (raw AST; ComponentExpression nodes)
    ↓ AstTransform (EmitLiftTransform, etc.)
Vec<Statement>  (lowered AST; bare Component statements desugared to emit(...) calls)
    ↓ evaluator
  - Statement::Expression(Call("emit", [ce]))  → SpawnComponentTree intent → main thread
  - Statement::Expression(anything → ComponentObject)  → same emit path (Option B rule)
  - Statement::Assignment   → Value::ComponentObject stored in ObjectWorld env
  - Statement::Expression(anything → other Value)  → discarded
  - etc.
```

The evaluator does not see raw `Statement::Expression(Expression::Component(...))` at the
top level — the `EmitLiftTransform` has already desugared those to `emit(...)` calls before
evaluation begins.

---

## Un-parser direction

The un-parser runs in reverse:

```
ComponentId (live engine component)
    ↓ un-parser (reads component state, walks children)
ComponentExpression AST
    ↓ AstTransforms (ShortformTransform, DefaultPruneTransform, etc.)
ComponentExpression AST (normalized)
    ↓ MmsPrinter
.mms source string
```

AstTransforms apply here too — the same named-concept infrastructure, different transforms.
`ShortformTransform` replaces `Transform` with `T`, `Color` with `C`, etc. `DefaultPruneTransform`
removes named assignments whose values match the component's defaults.
