# Emission policy options

The question: **when does a `ComponentObject` get emitted to the engine?**

The static case is settled: a component expression literal in statement position
(`T { R { } }` as a bare statement) is lifted to `Statement::Emit` by the
`EmitLiftTransform`. This is unambiguous — the source shape alone determines emission.

The open question is the **dynamic case**: what happens when a non-literal expression in
statement position *happens* to produce a `ComponentObject` at runtime? This document
explores the main policy options.

---

## The scenarios in question

```mms
// 1. Bare variable in statement position
let cube = R.cube() { C.rgba(1, 0, 0, 1) }
cube          // ← what does this do?

// 2. Function call in statement position
maybe_cube()  // ← emits if it returned a ComponentObject? or always noops?

// 3. Function body: tail expression that's a variable
let maybe_stuff = fn() {
    if (condition) {
        cube   // ← last evaluated expression in the if-branch
    }
    // else falls through with Null
}
maybe_stuff()  // ← what happens?
```

---

## Option A — Explicit `emit()` for everything non-literal (current v1 plan)

Component expression **literals** auto-emit when free-standing. Everything else requires
an explicit `emit(x)` call:

```mms
// Literals auto-emit:
R.cube() { C.rgba(1, 0, 0, 1) }

// Variables and call results require explicit emit():
let cube = R.cube() { C.rgba(1, 0, 0, 1) }
emit(cube)

maybe_stuff()           // does NOT emit even if it returns a ComponentObject
emit(maybe_stuff())     // explicit
```

**Pros:**
- Simple, predictable. Emission is always structurally visible in the source.
- The `EmitLiftTransform` is the only place auto-emission happens; no runtime type checks
  needed for emission decisions.
- No surprising side effects from calling a function.

**Cons:**
- Slightly verbose for the variable pattern — `let x = ...; emit(x)` where just `let x = ...`
  followed by `x` would be cleaner.
- Cannot express "emit the result of a function call" cleanly without a wrapper.

**Best suited for:** v1. Explicit, auditable, no surprises.

---

## Option B — Any `ComponentObject` in statement position auto-emits

The runtime rule: evaluate any `Statement::Expression`; if the result is a
`ComponentObject`, emit it. This covers bare identifiers, function calls, and any other
expression that produces a `ComponentObject`:

```mms
let cube = R.cube() { C.rgba(1, 0, 0, 1) }
cube            // ← auto-emits (bare identifier)
maybe_stuff()   // ← auto-emits if it returns a ComponentObject
```

**Pros:**
- Fluent and ergonomic. "If it's a component and it's just sitting there, emit it."
- Consistent with the literal auto-emit rule.
- Functions that return `ComponentObject` compose naturally with the call site.

**Cons:**
- Emission is no longer structurally visible. A function call `f()` in statement position
  might or might not emit depending on what `f` returns — you have to trace the runtime type.
- Makes the "discard" case (evaluate for side effects but ignore result) hard to express for
  a function that incidentally returns a `ComponentObject`.
- Composability is tricky: `let x = f(); x` would emit, but `f()` on its own would also emit
  — it's not obvious which you want.

**Best suited for:** A more dynamic, exploratory scripting style. Higher cognitive load.

---

## Option C — Tail-value propagation: function bodies, not bare identifiers

A middle ground: bare variables in statement position require `emit(x)` (same as Option A),
but a function call in statement position auto-emits if the function's *tail expression*
evaluates to a `ComponentObject`:

```mms
// Bare variables still require explicit emit:
let cube = R.cube() { C.rgba(1, 0, 0, 1) }
emit(cube)          // explicit

// But function calls propagate tail ComponentObjects:
let make = fn(r, g, b) {
    if (r > 0.5) {
        R.cube() { C.rgba(r, g, b, 1.0) }   // Statement::Emit — fires inside make()
    }
    // else: returns Null → call site gets Null → nothing emitted
}
make(1.0, 0.0, 0.0)    // ← emits internally (Statement::Emit inside the function body)
make(0.0, 0.0, 0.0)    // ← no emit (Null returned from else path)
```

The key distinction from Option B: the emission happens **inside the function** via
`Statement::Emit` (the `EmitLiftTransform` fired on the free-standing component expression
inside the function body). The call site just calls `make(...)` and the side effect is
transparent. The call site does not check the return type.

This is actually already what the `EmitLiftTransform` gives us today — the transform fires
on any block, including function bodies. So free-standing component expressions inside a
function body always emit when that function runs. The caller doesn't need to do anything.

What the user's sketch adds on top: **a bare variable in the function body** (`cube` inside
an `if` branch) might also propagate, if we want it to. That requires the runtime to check
the type of the tail expression in a block when it is about to be discarded.

```mms
let maybe_stuff = fn() {
    if (condition) {
        cube    // ← if cube is ComponentObject AND this is tail position in the if-block...
                //   does it emit? or does it just return from the function?
    }
}
maybe_stuff()   // ← does this emit?
```

This is where the design space gets complex. Two sub-options:

**C1 — Tail variable propagates (dynamic type check at block exit):**
When the last expression of a block evaluates to a `ComponentObject`, it is returned up the
call stack. If the function call is in statement position, the caller's evaluator applies the
same rule: ComponentObject in statement position → emit. This chains naturally.

**C2 — Only literal component expressions propagate (static in function body):**
Inside a function body, only `Statement::Emit` (from the `EmitLiftTransform`) causes emission.
A bare variable `cube` in the function body does NOT propagate — it just returns the value.
The caller gets a `ComponentObject` back and must handle it explicitly.

C2 keeps the rule static: only literal component expressions in statement position auto-emit,
everywhere. C1 adds a dynamic check.

---

## Option D — Annotated / typed emission

Functions can be declared to emit (rather than return):

```mms
// hypothetical syntax
fn cube_maker(r, g, b) emits {
    R.cube() { C.rgba(r, g, b, 1.0) }
}
cube_maker(1.0, 0.0, 0.0)   // ← caller knows this emits; no ComponentObject returned
```

Or alternatively, functions have a declared return type:

```mms
fn build_cube(r, g, b) -> Component {
    return R.cube() { C.rgba(r, g, b, 1.0) }
}
let c = build_cube(1.0, 0.0, 0.0)
emit(c)
```

**Pros:**
- Statically knowable. The caller can see from the function signature whether calling it
  emits or returns.
- No runtime type checks.

**Cons:**
- Requires a type system or at least annotation syntax — significant added complexity.
- Feels heavier than MMS's lightweight philosophy.

**Best suited for:** A future where MMS has a more developed type/annotation layer.

---

## Summary table

| Option | Literals | Bare variable | Function call | Runtime type check? |
|---|---|---|---|---|
| **A** — explicit emit() | auto | needs `emit(x)` | needs `emit(f())` | No |
| **B** — universal | auto | auto | auto | Yes (call site) |
| **C1** — tail propagation | auto | auto (in tail pos) | auto (if tail emits) | Yes (block exit) |
| **C2** — literal-only + fn-internal | auto | needs `emit(x)` | fn-internal emit only | No |
| **D** — typed | auto | needs `emit(x)` | depends on annotation | No |

---

## Current decision

**Option B — universal.** Any `Statement::Expression` that evaluates to a `ComponentObject`
causes emission. This covers bare variables, function calls, and expression chains — anything
that produces a `ComponentObject` in statement position auto-emits at runtime.

```mms
let sky = BGC.rgba(0.62, 0.80, 1.00, 1.0)
sky        // ← auto-emits (runtime check: sky is ComponentObject)

let cube = R.cube() { C.rgba(1, 0, 0, 1) }
cube       // ← auto-emits

maybe_cube()   // ← auto-emits if maybe_cube() returns a ComponentObject
```

The `EmitLiftTransform` remains in place for the static case (component expression literals),
but the evaluator also applies the runtime check for all other `Statement::Expression` forms.
`emit()` is still a valid explicit builtin but is not required.

**Path to Option D (typed functions):** Option B works without a type system. When MMS gains
function return type annotations, callers will be able to see statically whether a call emits
— but the runtime behaviour under Option B will be the same. The type annotation becomes
documentation and a static check, not a behaviour change.

**The `maybe_stuff()` / tail-propagation sketch** from the earlier discussion is naturally
covered by Option B: if a function's last evaluated expression is a variable that holds a
`ComponentObject`, and the function call is in statement position, the `ComponentObject`
propagates up and the runtime check at the call site fires the emit. No special tail-position
rule needed — Option B subsumes it.

**Note on `Statement::Emit`:** earlier drafts of this doc proposed a new `Statement::Emit`
AST variant. That is superseded — the `EmitLiftTransform` instead desugars free-standing
component expressions into `Expression::Call { callee: "emit", ... }`. No new statement
variant is needed; `emit` is just a built-in callable.
