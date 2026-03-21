# Functions and closures in MMS

Design decisions for Phase 4 of the MMS roadmap.

---

## Syntax

Named functions are `let` bindings with a function expression on the RHS. No separate
`fn name(args)` declaration syntax is needed — `let` covers it uniformly:

```mms
let make_cube = fn(r, g, b) {
    R.cube() { C.rgba(r, g, b, 1.0) }
}
make_cube(1, 0, 0)
```

A `fn name(args) { }` shorthand at the top level could be added as sugar later, but `let`
is the canonical form and sufficient for v1.

---

## Closures vs plain functions

**The question:** do MMS functions close over their lexical environment?

### Option A: plain functions (no capture)

A function can only see its own parameters. No access to outer `let` bindings.

```mms
let color = [1, 0, 0, 1]
let make_cube = fn() {
    R.cube() { C.rgba(color) }   // ← error: color not in scope
}
```

- Simple to implement: a function body is just evaluated with a fresh env seeded by params.
- Forces all inputs to be explicit parameters — arguably better style.
- Not how most scripting languages work; likely surprising.

### Option B: full lexical closures

A function captures the env at definition time:

```mms
let color = [1, 0, 0, 1]
let make_cube = fn() {
    R.cube() { C.rgba(color) }   // ← color captured from outer scope
}
```

- Natural and expected for a scripting language.
- Implementation: capture a snapshot of `env` at the point where `fn(...)` is evaluated.
  For a flat `HashMap<String, Value>` (v1), this is just `env.clone()`.
- Captured values are by value (immutable snapshot), not by reference. MMS v1 has no
  mutation of let bindings, so this is fine.

**Recommendation: Option B (closures).** The implementation cost is low (clone the env),
the ergonomics are significantly better, and MMS values are copy/clone types in v1.

---

## Scope rules

### v1: flat env with call-frame push/pop

The simplest implementation that supports closures:

- Top-level env: `HashMap<String, Value>`
- On function call: clone the closure's captured env, bind params on top, evaluate body
  in that env, discard on return
- `let` inside a function body mutates the call-frame env (visible to the rest of the body)
- No block scoping within a function (inner `{ }` blocks don't create a new scope frame)

This is "call-frame scoping" — not full lexical block scoping, but good enough for v1 and
easy to reason about.

### v2: lexical scope chain

A `Vec<HashMap<String, Value>>` scope chain. Each `{ }` block pushes a frame; closing it
pops the frame. `let` inside `if { }` is not visible outside the block.

Required for correctness in patterns like:

```mms
let x = if cond {
    let temp = T { }
    temp                    // temp returned here
} else {
    R.cube() {}
}
// temp should NOT be visible here — only with scope chain
```

Defer to v2 unless this pattern appears in real scripts.

---

## `return` semantics

`return expr` exits the current function call frame and yields `expr` as the call's value.
At the top level (not inside a function), `return` is either a no-op or an error.

The evaluator needs an unwind mechanism — the simplest is a `Result`-style early exit:
- `StmtEffect::Return(Value)` — propagates up through block evaluation
- When the call frame catches it, it becomes the function's return value
- Same mechanism needed for `break`/`continue` in loops (Phase 5/8)

---

## `fn` as a value

`fn(...)` is an `Expression::Function` — it can appear anywhere an expression is valid:

```mms
let fns = [fn(x) { x }, fn(x) { x + 1 }]   // array of functions
let result = fns[0](42)                        // call by index
```

This requires `Value::Function { params, body, captured_env }` as a runtime value. In v1,
higher-order functions like this are probably not needed in practice, but the architecture
should not preclude them.
