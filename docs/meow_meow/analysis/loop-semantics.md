# ₍˄·͈༝·͈˄₎ MMS Loop Semantics

Design analysis for Phase 5 (`for`/`while`) and a proposed extension: **tree-traversal
loops** as a MMS-native concept.

See roadmap Phase 5 for the implementation checklist. This doc covers the design decisions
not captured there.

---

## 1. Basic `for` loop — iterating over arrays

The canonical `for` loop iterates over a `Value::Array`:

```mms
for x in [1, 2, 3] {
    T.position(x, 0, 0) { R.cube() {} }
}
```

The binding `x` is scoped to the loop body. Each iteration:
1. Binds `x` to the next element of the array.
2. Evaluates the body in that environment.
3. Any `emit()` / free-standing component expression in the body fires under the current
   **emit context** — so if the `for` is inside a component body, all iterations' outputs
   attach as siblings under the same parent.

---

## 2. Numeric range iteration

Arrays of numbers are unwieldy to write out (`[0, 1, 2, ..., 99]`). Two options:

### Option A: `range(n)` builtin

```mms
for i in range(10) { ... }        // i ∈ [0, 9]
for i in range(2, 8) { ... }      // i ∈ [2, 7]
for i in range(0, 100, 5) { ... } // i ∈ [0, 5, 10, ..., 95]
```

`range(n)` returns a `Value::Array` of `Value::Number`s. No new syntax required beyond
what Phase 5 already plans.

**Downside:** materializes the whole array in memory. Fine for small counts (dozens to
hundreds); awkward for very large ranges.

### Option B: `..` range syntax

```mms
for i in 0..10 { ... }   // exclusive end: i ∈ [0, 9]
for i in 0..=10 { ... }  // inclusive end: i ∈ [0, 10]
```

Requires a new `DotDot` (`..`) and `DotDotEq` (`..=`) token, a `Expression::Range` AST
node, and a lazy iterator value (or eager materialization). Does not need `Value::Array`
materialization.

**Recommendation:** start with `range(n)` (Option A) for Phase 5 since it requires no new
tokens and the array size is bounded by practical use (cloud puffs, bone counts, etc.).
Add `..` syntax if lazy ranges become important.

---

## 3. `while` loop

```mms
let i = 0;
while i < 10 {
    T.position(i * 0.5, 0, 0) { R.cube() {} }
    let i = i + 1;   // ❓ rebinding vs mutation — see §6
}
```

`while` evaluates its condition before each iteration. Exits when the condition is `false`
or `null`. Requires Phase 2 (boolean eval) to be useful.

`while true { ... break ... }` is the `loop` construct — no separate `loop` keyword is
needed if `break` is implemented.

---

## 4. `break` and `continue`

Both require an **unwind mechanism** through the evaluator — the same stack unwinding used
by `return` (Phase 4):

```rust
enum StmtEffect {
    None,
    Emit(IntentValue),
    Bind(String, Value),
    Return(Value),  // Phase 4
    Break,          // Phase 5/8
    Continue,       // Phase 5/8
}
```

`Break` and `Continue` propagate up through the block evaluator until caught by the
enclosing `for`/`while` handler. `Return` propagates past the loop (caught by the
function frame).

`break value` (breaking with a value, like Rust's `loop { break x }`) is not planned for
v1 — defer unless a concrete need arises.

---

## 5. (=ΦωΦ=) Tree traversal: DFS loops over component expressions

This is the MMS-native extension not present in any other language in the roadmap.

### Motivation

Component trees are the primary data structure in cat-engine. The most natural loop
over a tree is a **depth-first traversal** — visit each node, optionally doing something
to it. Current MMS can *build* trees; it cannot *traverse* an existing one.

Use cases:
- Apply a tint to every `Renderable` child of a GLTF scene
- Find and extract specific bones from an armature subtree
- Generate implicit children based on tree structure (e.g. add a `Color` to every node
  that has a `Renderable` but no `Color`)

### `for node in dfs(expr) { }`

```mms
let avatar = GLTF.new("rei.glb") {}
for node in dfs(avatar) {
    // node is a ComponentObject — a live handle to one component in the subtree
    // DFS order: pre-order (parent before children)
}
```

`dfs(expr)` takes a `ComponentObject` (live handle, Phase 6) and returns a sequence of
`ComponentObject`s in depth-first pre-order — the root, then its children recursively.

**What you can do inside the loop:**
- Read properties via `node.type()` (returns an identifier like `Transform`, `Renderable`)
- Mutate the component via `node.set(...)` / `node.call(...)` (Phase 7 mutation API)
- Emit new components as children (`emit(C.rgba(1,0,0,1))` inside the loop body while the
  emit context stack has `node` as the parent — see §5.2)
- Early exit with `break`

### 5.1 DFS variants

| Form | Traversal |
|------|-----------|
| `dfs(root)` | Pre-order: root → children left-to-right recursively |
| `dfs_post(root)` | Post-order: children recursively → root |
| `bfs(root)` | Breadth-first: level by level |
| `children(node)` | Immediate children only (depth 1) |
| `ancestors(node)` | Walk up to the root |

For v1, `children(node)` is the most immediately useful and simplest to implement (no
recursion required in MMS land — just iterate the direct child list from the engine). `dfs`
follows as the natural generalization.

### 5.2 Emit context inside traversal loops

Inside a `for node in dfs(avatar)` loop, the emit context is **not automatically the current
node**. Emitting inside the loop body emits under whatever context was active when the `for`
started.

To emit as a child of the current node, push the node explicitly onto the emit context:

```mms
for node in dfs(avatar) {
    if node.type() == Renderable {
        with node {                         // ← push node as emit context
            C.rgba(1, 0, 0.8, 1)           // emits as child of node
        }
    }
}
```

`with node { body }` is a proposed **emit context block** — it temporarily pushes `node`
onto the emit context stack for the body. This is the natural complement to component body
syntax (`T { ... }` already pushes T implicitly).

Alternatively, the loop variable could auto-push:

```mms
for node in dfs(avatar) emit_under node {
    C.rgba(1, 0, 0.8, 1)   // always a child of node
}
```

**Recommendation:** implement `with node { }` as the general mechanism; the `for ... emit_under`
form is sugar if the pattern is common enough.

### 5.3 Type-filtered traversal

Traversing an entire tree to find specific component types is a common pattern:

```mms
for node in dfs(avatar) where node.type() == Renderable {
    // only Renderable nodes
}
```

`where` is a filter clause. Equivalent to `if node.type() == Renderable { }` inside the
body but potentially optimized by the runtime (skip subtrees that can't contain a match).

Whether `where` is a keyword or just a convention is open.

### 5.4 Runtime requirements for tree loops

Tree-traversal loops require **Phase 6 (live `ComponentId` reply channel)** as a prerequisite.
`dfs(avatar)` needs `avatar` to be a live `ComponentObject` (i.e., a real `ComponentId`),
not an unresolved `ComponentExpression` AST node.

The engine side needs a query:
- `world.children_of(id) -> Vec<ComponentId>` (already exists in `ComponentNode`)
- `world.component_type_name(id) -> &str` (for `node.type()`)

These are read-only world queries — no new intents needed for traversal itself.

---

## 6. Loop variable scoping and rebinding

MMS v1 has no mutable bindings. `let i = i + 1` inside a loop body creates a **new**
binding named `i` that shadows the outer one — but only in the current block. After the
block exits, the outer `i` is unchanged. This makes `while` loops that count upward
impossible without mutation.

Options:

### Option A: immutable bindings everywhere (current plan)

`let x = expr` always creates a new immutable binding. Rebinding is shadowing. The old
value is gone from the inner scope but restored when the block exits.

**Problem:** `while i < 10 { let i = i + 1 }` doesn't work — the `i` on the LHS of the
rebind is a new local binding that doesn't affect the outer `i`, so the condition never
changes.

`for i in range(10)` still works fine because the loop variable is rebound each iteration
by the loop machinery itself, not by user code.

### Option B: mutable `var` declarations

Introduce `var` for mutable bindings:

```mms
var i = 0;
while i < 10 {
    i = i + 1;   // assignment to a mutable binding (no `let` or `var`)
}
```

`let` = immutable. `var` = mutable. Assignment without a keyword mutates an existing `var`.

This is the most ergonomic but requires:
- New `Var` token
- Distinguishing `let` and `var` in `StoredValue` or `Env`
- Assignment `x = expr` as a statement (distinct from `let x = expr`)

### Option C: side-effect counter via `range(n)` (defer mutable loops)

For the Phase 5 cloud use case, `for i in range(n)` covers all needed iteration. `while`
loops that require mutation can be deferred to a later phase when `var` is introduced.

**Recommendation:** Phase 5 ships `for` with `range(n)` only. Defer `while` and `var`
to Phase 8 with explicit mutable binding design.

---

## 7. `repeat(n) { }` — a component-centric shorthand

The cloud generation pattern is specifically:

```mms
T.position(x, y, z) {
    for i in range(28) {
        T.position(ox, oy, oz) { R.cube() { C.rgba(r, g, b, 1) } }
    }
}
```

A shorthand that makes the component-authoring intent clearer:

```mms
T.position(x, y, z) {
    repeat(28) using i {
        T.position(ox, oy, oz) { R.cube() { C.rgba(r, g, b, 1) } }
    }
}
```

`repeat(n) using i { }` is sugar for `for i in range(n) { }` but reads as a
declarative "create 28 copies" rather than an imperative "loop 28 times."

The `using i` clause is optional — `repeat(28) { }` is valid when the index is not needed.

Whether this is worth a keyword or just a `range`-based idiom is a style question. For now,
`for i in range(n)` is sufficient and `repeat` can be added as sugar later.

---

## 8. Open questions

| Question | Impact |
|----------|--------|
| `range(n)` vs `..` syntax for numeric ranges | Phase 5 token/AST scope |
| `var` for mutable bindings vs immutable-only | `while` loop usability |
| `with node { }` emit context block syntax | DFS traversal ergonomics |
| `dfs` / `children` return type — lazy iterator vs materialized array | Efficiency on large trees |
| `where` clause for filtered traversal | Sugar vs explicit `if` in body |
| `repeat(n)` shorthand | Authoring ergonomics |
| Type of `node.type()` — identifier value or string? | Downstream `==` comparison |
| Can `dfs` loop be used before Phase 6 (unresolved CE)? | Probably not — needs live `ComponentId` |
