# Emission semantics and component value model

This document explores what happens to component expressions depending on *where* they appear
in an MMS program — and what the runtime object model looks like for component expressions that
are evaluated but not immediately emitted to the engine.

---

## The program is a block expression

A `.mms` file parses into `Vec<Statement>`. This is structurally identical to a
`BlockStatement` — an implicit outermost block that contains any number of statements.

```mms
BGC.rgba(0.62, 0.80, 1.00, 1.0)
AL.rgb(0.18, 0.18, 0.22)
T.with_position(0.0, 0.0, 0.0) { GLTF.new("rei.glb") }
XR.on()
```

There is no single "root node." The program is a flat sequence of statements, and each
free-standing component expression becomes a separate tree root in the engine. A `.mms` scene
file naturally describes multiple independent subtrees that all get added to the world.

---

## What determines whether a component expression is emitted?

The rule is based entirely on where the component expression appears in the AST — not on any
runtime value or context flag:

| Position | Example | Effect |
|---|---|---|
| Free-standing component literal in any block | `T { }` as a statement | **Emitted** (static — `EmitLiftTransform`) |
| Bare variable holding a `ComponentObject` | `x` as a statement | **Emitted** (runtime check) |
| Function call returning a `ComponentObject` | `f()` as a statement | **Emitted** (runtime check) |
| Any other `Statement::Expression` → `ComponentObject` | — | **Emitted** (runtime check) |
| `emit()` builtin | `emit(x)` | **Emitted** (explicit; redundant under Option B but still valid) |
| Let binding (RHS) | `let x = T { }` | **Captured** as `ComponentObject`; not emitted |
| Return expression | `return T { }` | **Returned** to caller as `ComponentObject` |
| Inside an array | `[T { }, R { }]` | **Values** in the array; not emitted |
| Function argument | `f(T { })` | **Value** passed to callee; not emitted |

**The emission rule (Option B):** the `EmitLiftTransform` handles the static case — component
expression literals in statement position become `Statement::Emit`. For everything else, the
evaluator applies a runtime check: evaluate any `Statement::Expression`; if the result is
`Value::ComponentObject`, emit it. This covers bare variables, function calls, and any
expression chain. See [emission policy options](emission-policy-options.md) for alternatives
and the path to typed function annotations.

---

## AstTransform: formalizing the emit lift

The parser produces `Statement::Expression(Expression::Component(...))` for a free-standing
component expression. A post-parse, pre-eval structural pass — an **AstTransform** — desugars
this into an explicit call to the `emit` built-in:

```
Statement::Expression(Expression::Component(ce))
    →  Statement::Expression(Expression::Call(CallExpression {
           callee: Box::new(Expression::Identifier(Ident("emit".into()))),
           args: vec![Expression::Component(ce)],
       }))
```

This is `EmitLiftTransform`. The result is a normal call expression — no new `Statement`
variant is needed. `emit(T { })` written explicitly in source and `T { }` as a bare statement
are identical after the transform. The evaluator never needs to distinguish them.

An **AstTransform** is a structural, rule-based transformation on the AST that runs after
parsing and before evaluation. It inspects AST shape only — no runtime values, no type
information. Transformations apply recursively to all blocks (top-level program, function
bodies, `if` branches, etc.).

The same concept applies in both directions of the MMS pipeline:

- **Parse direction (pre-eval):** `EmitLiftTransform` — desugars free-standing component
  expressions into `emit(...)` calls. Applied recursively to all blocks.
- **Unparse direction (pre-print):** AstTransforms normalize the un-parsed AST before
  printing. For example: `ShortformTransform` (replace `Transform` with `T`, etc.),
  `DefaultPruneTransform` (drop named assignments matching component defaults). The inverse
  of `EmitLiftTransform` can also run here to strip `emit(...)` wrappers back to bare
  component expressions in the printed output.

Whether AstTransform needs a formal trait or stays as a named convention with standalone
functions is an open question. For v1, standalone functions are fine.

---

## Nesting: children are not independently emitted

Inside `T { R { } }`, the `R { }` is a `ComponentBodyItem::Child` — it is part of `T`'s
body, not a free-standing statement. The `EmitLiftTransform` only fires at the `Statement`
level. Component expressions nested inside other component expressions (as body children) are
never independently emitted. The whole tree is one `Emit`.

---

## Function bodies and emission

The `EmitLiftTransform` applies to every block uniformly, including function bodies. This
means a function whose body is just a free-standing component expression emits that component
when called:

```mms
let make_cube = fn(r, g, b) {
    R.cube() {
        C.rgba(r, g, b, 1.0)
    }
}

make_cube(1.0, 0.0, 0.0)   // ← emits a red cube (world root — top-level call)
make_cube(0.0, 1.0, 0.0)   // ← emits a green cube (world root — top-level call)

T.with_position(0, 0, 0) {
    make_cube(0.0, 0.0, 1.0)  // ← emits a blue cube as a child of T
}
```

`R.cube() { ... }` is free-standing in the function body → `EmitLiftTransform` converts it
to `Statement::Emit`. Calling `make_cube(...)` runs that emit. The emission target (world root
vs. child) is determined by the **emit context** at the call site — see the emit context
section below. The function definition itself has no special case: it just emits.

A function that **returns** a `ComponentObject` instead of emitting uses `return`:

```mms
let build_cube = fn(r, g, b) {
    return R.cube() { C.rgba(r, g, b, 1.0) }
}

let x = build_cube(0.5, 0.5, 0.5)   // x is a ComponentObject; nothing emitted yet
emit(x)                               // explicit emission
```

---

## ComponentObject: a live handle with a mutation API

When a component expression is **captured** (in `let`, `return`, array, or function argument
position) rather than emitted, the result is a **`ComponentObject`**.

A `ComponentObject` is not an inert description or AST snapshot. It is a **live handle** to
an engine component — it holds the `ComponentId` of a component that has been created in the
world (but is not yet attached to any parent). Through the `ComponentObject`, MMS code can:

- mutate the component's properties (emitting the appropriate intents to the main thread)
- attach it to a parent (becoming a child in the component tree)
- emit it as a world root (`emit(x)`)
- pass it to a function that operates on component handles
- store it in an array or data structure

```rust
// runtime value model
pub enum Value {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<Value>),
    ComponentObject(ComponentId),   // ← live engine component; unattached until emitted/attached
    // future: Closure, Handle, ...
}
```

Creating a `ComponentObject` **does** touch the engine — the component is allocated and
initialized. What `let x = T { }` is NOT doing is making that component a world root or
attaching it anywhere. Emission (`emit(x)` or free-standing statement position) is what
causes attachment/registration.

This is the natural analog to the Rust API:

```rust
let id = universe.world.add_component(TransformComponent::new());  // allocated, unattached
universe.add(id);                                                   // now a world root
```

### The `emit()` builtin

`emit(x)` is useful for explicitly emitting a previously captured `ComponentObject`:

```mms
let cube = R.cube() { C.rgba(1, 0, 0, 1) }
// ... conditional logic, configuration ...
if should_show {
    emit(cube)
}
```

Without `emit(x)`, the only way to conditionally emit is to construct the component
expression inline inside the branch:

```mms
if should_show {
    R.cube() { C.rgba(1, 0, 0, 1) }   // free-standing → Statement::Emit
}
```

Both patterns are valid. The inline form is cleaner when the condition is simple. `emit(x)` is
needed when the object has been configured or passed around before emission.

---

## Emit context: where do emitted components attach?

Every `emit()` call — whether from `EmitLiftTransform`, `emit(x)`, or Option B runtime check
— produces a `SpawnComponentTree` intent. That intent carries an optional `parent`:

```
SpawnComponentTree {
    root: ce,
    parent: None,            // → world root
    // or:
    parent: Some(parent_id), // → attached as a child of parent_id
}
```

**The emit context stack** determines which applies:

- At top level (program body), the emit context stack is **empty** → `parent: None` → world root.
- When evaluating a `ComponentExpression` body, the parent component's `ComponentId` is
  **pushed** onto the emit context stack for the duration of that body's evaluation.
- Any `emit()` call that fires during body evaluation — including inside free-standing function
  calls made from the body — uses the **top of the stack** as the parent.
- When the body finishes evaluating, its entry is **popped**.

This applies transitively through function calls (dynamic scoping of the emit context):

```mms
let make_cube = fn(r, g, b) {
    R.cube() { C.rgba(r, g, b, 1.0) }   // emits; parent depends on call site
}

make_cube(1, 0, 0)                         // emit context empty → world root

T.with_position(0, 0, 0) {
    make_cube(1, 0, 0)                     // emit context = T → child of T
    make_cube(0, 1, 0)                     // emit context = T → another child of T
}
```

The function definition is the same in both cases. Where it emits is determined by where it
is called — top-level call sites produce world roots, body call sites produce children of the
enclosing component.

For nested bodies:

```mms
T {
    A {
        make_cube(1, 0, 0)    // emit context = A (innermost) → child of A
    }
    make_cube(0, 1, 0)        // emit context = T → child of T
}
```

**Compatibility with Option B:** The runtime check "if expression-statement yields a
`ComponentObject`, emit it" still applies. The emit context stack just determines the parent
for that emission. The rule is uniform: emit always goes to the current emit context.

---

## What gets emitted: one component tree, one `SpawnComponentTree`

When `Statement::Emit(ce)` evaluates:

```
SpawnComponentTree {
    root: ce,
    parent: None,   // or Some(ComponentId) from the emit context stack
}
```

The main thread executor walks the `ComponentExpression` tree and creates all components.
Multiple `Statement::Emit` in one file → multiple `SpawnComponentTree` intents → multiple
independent world roots (when called at top level). `vr-input.mms` has nine root-level
statements, producing nine independently rooted subtrees.

---

## Open questions

**Q1: What happens to an unattached `ComponentObject` that is never emitted?**

If `let x = T { }` creates the component and the script ends without emitting `x`, that
component exists in the world but is unreachable from any root. This is a potential leak.

Options:
- The MMS runtime tracks all unattached `ComponentObject`s and cleans them up at script end
  if not emitted.
- Unattached components are valid (they can be emitted later, in a subsequent script or via
  the REPL) — the host is responsible for cleanup.
- v1: ignore the problem; the REPL will let you inspect and remove stray components.

**Q2: `ComponentObject` mutation API — what does it look like?**

The mutation API on `ComponentObject` is host-defined, not a language primitive. Possible
forms:
- `x.set(rotation = [0, 0, PI])` — named assignment syntax, mirrors body syntax
- `x.call(with_intensity(1.5))` — call syntax
- `x.attach(y)` — attach another `ComponentObject` as a child

This is future design space. v1 just needs the `ComponentObject` value type to exist.

**Q3: Can a `ComponentObject` be used as a body child?**

```mms
let cube = R.cube() { C.rgba(1, 0, 0, 1) }
T {
    cube    // ← is this a positional ComponentObject reference?
}
```

This would allow pre-built subtrees to be spliced into component expressions. It requires the
evaluator to recognize a `ComponentObject`-valued identifier in `Child` position and use
`Attach` rather than `SpawnComponentTree`. Deferred to a future revision.
