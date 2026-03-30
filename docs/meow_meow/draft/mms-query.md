# ᓚᘏᗢ MMS Query — Design Draft

> **Status: draft.** Not yet implemented.
> This is the canonical reference for MMS query syntax.
> Both module query and world query use the same selector language and API.

---

## One query system, two contexts

Query in MMS is a single concept — `thing.query(selector)` — that works across two
execution contexts with different subjects:

| Subject | Context | Returns | Requires |
|---|---|---|---|
| *(none)* | Live ECS world | `ComponentObject?` or `[ComponentObject]` | HostCall (Phase 6+) |
| `ComponentObject` | Live ECS subtree | `ComponentObject?` or `[ComponentObject]` | HostCall (Phase 6+) |
| `Module` | Module CE tree (pre-spawn, static) | `ComponentExpr?` or `[ComponentExpr]` | Module import |

The selector string syntax is identical across all three. The subject determines **where**
to search and **what kind of handles** come back. The API — `query()`, `query_all()`, the
callback parameter, the `->` sugar — is the same in every context.

---

## API

### `query(selector)` — single result

```mms
let t = query("#hero_transform")          // world — ComponentObject? (null if not found)
let t = hero.query("T")                   // subtree — first T child of hero
let t = scene.query("#hero_transform")    // module — ComponentExpr? (null if not found)
```

Returns the **first** match or `null`. For selectors that are expected to match at most
one thing (`#id`). Return type is nullable: always guard before using.

```mms
let hero = query("#hero")
if !hero { return }
hero.set_position(0, 1, 0)
```

### `query_all(selector)` — multiple results

```mms
let enemies = query_all(".Enemy T")       // world — [ComponentObject], empty if none
let ts = hero.query_all("T")             // subtree — all T descendants of hero
let parts = scene.query_all(".R")        // module — [ComponentExpr]
```

Returns all matches as an array. Empty array if none. For type/structural selectors.

### Callback parameter

Both functions accept an optional second argument — a handler called with each result.

```mms
// single: handler called once with ComponentObject? (may be null)
query("#hero", fn(t) {
    if !t { return }
    t.set_position(0, 1, 0)
})

// all: handler called once per result (never null — null means not in the list)
query_all(".Enemy T", fn(enemy) {
    enemy.set_position(0, 0, 0)
})
```

When the callback form is used, the return value of `query`/`query_all` is `null` (it's
used for its side effects). The handler is called synchronously — no async / callback
queue.

---

## `->` syntax sugar

The callback form avoids the closing-paren problem but the opening is still heavy:
`query("#hero", fn(t) {` on one side, `})` on the other. The `->` operator is sugar that
moves the handler to a trailing position:

```mms
query("#hero", fn(t) {          // explicit callback form
    if !t { return }
    t.set_position(0, 1, 0)
})

"#hero" -> fn(t) {             // -> sugar: identical semantics
    if !t { return }
    t.set_position(0, 1, 0)
}
```

`->` is a statement-level operator (not an expression). The general forms:

```
// world query — string is the selector
"selector" -> fn(result) { ... }
"selector" -> method_name(args)          // method shorthand — see below

// scoped query — first operand is the scope, second is the selector
scope_expr -> "selector" -> fn(result) { ... }
scope_expr -> "selector" -> method_name(args)
```

Desugaring rules:

```mms
"selector" -> handler
  ↓
query("selector", handler)

scope -> "selector" -> handler
  ↓
scope.query("selector", handler)
```

For `query_all`, the `->` form calls the handler once per result — whether single or
multiple is inferred from the selector (see "Single vs multiple" below).

### Method shorthand

When the rhs of `->` is a bare method call (not a full `fn(...) { }`), the receiver is
the implicit query result:

```mms
"#hero_transform" -> set_position(0, 1, 0)
// desugars to:
query("#hero_transform", fn(t) {
    if !t { return }
    t.set_position(0, 1, 0)
})

".Enemy T" -> set_position(0, 0, 0)
// desugars to:
query_all(".Enemy T", fn(t) {
    t.set_position(0, 0, 0)
})
```

### Chaining scope with `->`

```mms
// find all Ts in hero's subtree and set their position:
hero -> ".T" -> set_position(5, 0, 0)
// desugars to:
hero.query_all(".T", fn(t) { t.set_position(5, 0, 0) })

// or with a full callback:
hero -> ".T" -> fn(t) {
    t.set_position(5, 0, 0)
}
// desugars to:
hero.query_all(".T", fn(t) {
    t.set_position(5, 0, 0)
})
```

---

## Selector syntax

CSS-inspired. Component nodes have a **type** (the component class — `T`, `R`, `C`, etc.)
and an optional **name** (a string attribute set at construction time).

### Type selector

```
T           matches all TransformComponents
R           matches all RenderableComponents
C           matches all ColorComponents
```

Component type names use the same short aliases as in MMS source, resolved through the
component registry. Full names (`Transform`, `Renderable`) are also accepted.

### ID selector (`#`)

```
#hero       matches the component node whose name is "hero"
#bg_root    matches the component node whose name is "bg_root"
```

`#name` expects exactly one match. If no node has that name, `query` returns null,
`query_all` returns `[]`.

### Descendant combinator (space)

```
#hero T     all TransformComponents anywhere inside the node named "hero"
T C         all ColorComponents anywhere inside any Transform
```

### Child combinator (`>`)

```
#hero > T   TransformComponents that are **direct children** of "hero"
T > R       Renderables that are direct children of any Transform
```

### Multi-selector (`,`)

```
T, R        all Transforms OR Renderables (union of both result sets)
```

### Attribute selectors (future)

Reserved syntax; not implemented in v1.

```
T[name=hero]      Transform with name attribute = "hero"
R[material=glass] Renderable with material flag "glass"
```

---

## Single vs multiple — selector contract

`query` (single) and `query_all` (multiple) are separate functions. The selector string
itself does not dictate which to use — the caller chooses.

**Convention** (not enforced by the language):
- `"#id"` selectors — use `query()` (unique by design; `query_all` works but is unusual)
- Type/structural selectors — use `query_all()` (inherently multiple)

The `->` sugar follows the same convention: `"#id" ->` desugars to `query()`;
`".Type" ->` or `"A B" ->` desugars to `query_all()`. The heuristic: if the selector
contains a `#` at the root level with no combinators after → single; otherwise → all.

This heuristic can be overridden by using the explicit function form.

---

## Module query

When a module is imported with a namespace alias, `.query()` and `.query_all()` are
available on the module value. They search the module's **emitted CE tree** — the
sequence of `ComponentExpr` values produced when the module was evaluated.

```mms
import "level.mms" as scene         // namespace import (not yet in v1 — see module spec)

let hero_ce = scene.query("#hero")            // ComponentExpr? — not yet spawned
let all_enemies = scene.query_all(".Enemy")   // [ComponentExpr]

// Re-emit found CEs to actually spawn them:
if hero_ce { hero_ce }              // bare CE statement → emit → spawn
for e in all_enemies { e }
```

Module query is a **static** operation — no engine state needed, no HostCall. The module
was evaluated (its CE tree is in memory); query walks that tree.

### `->` with a module scope

```mms
scene -> ".Enemy R" -> fn(r) {
    // r is a ComponentExpr, not a ComponentObject
    // mutation methods are not available on pre-spawn CEs
    // but you can inspect/filter and re-emit selectively
    r
}
```

Pre-spawn CEs don't support mutation methods — you can't call `.set_color()` on something
that hasn't been spawned yet. Module query results are for **selecting and re-emitting**,
not for mutating live state.

> **Note:** `import X as namespace` is not yet implemented in v1 (see
> [module-import-export.md](../spec/module-import-export.md)). Module query depends on it.
> Named exports (`import { name }`) work today; module-level `.query()` is a future addition.

---

## World query and HostCall

`query()` and `query_all()` as free functions (no subject) are world queries — they
search the entire live ECS. These are HostCalls (see
[function-dispatch.md](../spec/function-dispatch.md), dispatch kind 4): they need live
engine state, cannot be resolved at compile time, and cross the script/host boundary.

```mms
import { query, query_all } from "world"   // hypothetical — binds world query functions
```

Or exposed automatically as part of a global prelude when world query is available. TBD.

Per-target binding for `query("#hero")`:

| Target | Strategy |
|---|---|
| cat-engine evaluator | `EvalResponse::Query { kind: QueryByName("hero") }` → spin-wait reply |
| cat-engine transpiled Rust | `engine.world().find_by_name("hero")` → direct |
| Offline / baked | Compile error — no live world |
| JS / WASM | Depends on scene model; likely `await host.query("#hero")` |

`ComponentObject.query()` is also a HostCall — it's a live subtree lookup using the
component's `children` and `descendants` data in the ECS.

---

## Summary

```
query("selector")                   // world query, single result (ComponentObject?)
query("selector", fn(r) { ... })    // world query + callback
query_all("selector")               // world query, all results ([ComponentObject])
query_all("selector", fn(r) { ... })

comp.query("selector")              // subtree query, single
comp.query_all("selector")          // subtree query, all
comp.query("selector", fn(r) { })  // subtree + callback

module.query("selector")            // module CE tree query, single (ComponentExpr?)
module.query_all("selector")        // module CE tree query, all

// -> sugar (statement only):
"selector" -> handler               // → query("selector", handler)
"selector" -> method(args)          // → query("selector", fn(r) { r.method(args) })
scope -> "selector" -> handler      // → scope.query("selector", handler)
```

---

## Open questions

1. **`->` vs `|>`** — `->` conflicts with the return-type arrow in function signatures
   (`fn(x: Num) -> Num`). The parser can tell them apart by position (type annotation
   context vs statement context), but it's an ambiguity worth noting. `|>` (pipe) avoids
   the conflict but is less common in game scripting. TBD.

2. **Single/multi heuristic for `->` sugar** — is inferring `query` vs `query_all` from
   the selector shape (`#id` → single, else → all) too magic? Alternative: always
   `query_all` (never null, always iterate), and the caller uses `[0]` for the single case.

3. **Module namespace import** — `import "level.mms" as scene` is required for
   `scene.query(...)`. This import form is not in v1. When added, it produces a `Value::Module`
   that exposes `.query()` and `.query_all()`.

4. **Pre-spawn mutation on module CEs** — if you query a module and want to modify a CE
   before spawning it (e.g. change a color), what's the API? Probably CE-specific setters
   that mutate the AST node before it's emitted. Distinct from the live `.set_color()` on
   a `ComponentObject`. Design deferred.

5. **Selector scope root** — `scope.query("T C")` — does `T` need to be a direct or
   indirect descendant of `scope`? Current proposal: `T C` means C anywhere inside
   the T, and T itself can be anywhere inside scope. Consistent with CSS descendant rules.

6. **Live query subscription / reactive** — `observe "selector" -> fn(r) { }` — register
   a handler that fires whenever the world's matching set changes (spawn/despawn). Deferred;
   document the reserved keyword.

7. **Query result ordering** — `query_all(".T")` — in what order are results returned?
   Depth-first tree order seems natural. Document the guarantee once implemented.
