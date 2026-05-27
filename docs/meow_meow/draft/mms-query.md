# ᓚᘏᗢ MMS Query — Design Draft

> **Status: draft.** Not yet implemented.
> This is the canonical reference for MMS query syntax.
> Both module query and world query use the same selector language and API.

> **Direction update (2026-04-26):** the original CSS-like shared-selector direction is
> still valid long-term, but the immediate MVP should be **MMQ first**:
> simple component-type and type+name matching such as `T` and `T#hero`.
> The CSS-flavored system should be treated as a later generalization, not the first
> required integration step.

## Current implementation snapshot

This doc is still the right place to centralize query semantics, but the full MMS query
runtime described below is **not** built yet.

What exists today:

- `QueryDesugarTransform` exists and rewrites `->` query/dispatch sugar at the AST layer.
- The evaluator / HostCall path for live `query()` / `query_all()` against the ECS world is
   still not implemented.
- The transition demo currently uses a **specialized authoring-time target resolver** via
   `Action.update_transform("#name", ...)`.

That last piece is important: `Action.update_transform("#hero", ...)` is **not** the MMS
query system. It is a narrow helper in the component registry that:

1. resolves the string selector while spawning the authored component tree,
2. converts it immediately into a concrete `ComponentId`, and
3. stores a normal `ActionComponent` with a regular `IntentValue::UpdateTransform`.

So at runtime, animation/actions are not doing string-based query lookups every frame.
They are just firing normal intents with already-resolved target ids.

Current supported forms for this specialized action-target lookup are intentionally narrow:

- `#name`
- `[name="..."]`
- bare exact label match

The shared `src/query` backend is also still incomplete:

- CSS parsing exists
- MMQ parsing does not
- evaluator combinator semantics are not integrated into the live world query path

This helper should be treated as **temporary authoring convenience**, not as the canonical
query runtime. As the real MMS query / HostCall system lands, selector-based action targeting
should either reuse that shared selector-resolution model or be explicitly documented as a
separate load-time binding feature.

> **Planned replacement:** see
> [`docs/task/action-target-scoping-and-factory-handlers.md`](../../task/action-target-scoping-and-factory-handlers.md).
> The current global, eval-time resolution will be replaced by lazy, **subtree-scoped**
> resolution rooted at the enclosing Animation's parent — so reusable component factories
> can spawn multiple instances without action selectors clashing across them.

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

## `->` — dispatch arrow

`->` is the **dispatch arrow**. Its LHS is always a string selector; its RHS is always
a handler or method shorthand. It is a world query + dispatch in one expression.

```mms
"#hero" -> fn(t) {             // dispatch arrow: query world for #hero, call handler
    if !t { return }
    t.set_position(0, 1, 0)
}
```

`->` **never** does scope injection. It always searches the full live ECS. To scope a
query to a subtree, use `component."selector"` (dot-selector access) for navigation,
not the dispatch arrow.

`|>` is the separate **forward pipe** operator: `expr |> f` means `f(expr)`. It has no
query meaning.

### Implementation: AstTransform, not parser or evaluator

The parser produces `BinOp(Query, lhs, rhs)` from `->` unconditionally. The rewrite from
dispatch arrow into `query()`/`query_all()` calls is performed by **`QueryDesugarTransform`**,
an AST pass that runs between parsing and evaluation (see
[script-runner.md](../spec/script-runner.md)).

This means:
- The parser stays context-free and single-responsibility.
- The evaluator's `Pipe` arm only ever sees `expr |> fn_value` (pure function application).
- All `Query` nodes (from `->`) have been rewritten into explicit `query()`/`query_all()` calls before eval.

### General forms

```
"selector" -> fn(result) { ... }     // world dispatch with full handler
"selector" -> method_name(args)      // world dispatch with method shorthand
```

The LHS of `->` is always a string selector. A `ComponentObject` on the LHS is a type
error — use `component."selector"` for subtree navigation instead.

Desugaring:

```mms
"selector" -> handler
  ↓
query_all("selector", handler)
```

### Method shorthand

When the RHS of `->` is a bare method call (not a full `fn(...) { }`), the receiver is
the implicit query result. The query runs against the world and the intent is batched
across all matches:

```mms
"#hero" -> set_position(0, 1, 0)
// desugars to:
query_all("#hero", fn(t) { t.set_position(0, 1, 0) })
// emits: SetPosition { component_ids: [hero_id], position: [0,1,0] }

".Enemy T" -> set_position(0, 0, 0)
// desugars to:
query_all(".Enemy T", fn(t) { t.set_position(0, 0, 0) })
// emits: SetPosition { component_ids: [e1, e2, ...], position: [0,0,0] }
```

### Subtree dispatch (via dot-selector + dispatch arrow on array)

To dispatch to matches within a subtree, navigate first with `component."selector"` or
`component.all("selector")`, then use the dispatch arrow on the resulting array:

```mms
// single result — just call the method directly:
box."C".set_color(1, 0, 0)

// multiple results — dispatch arrow on the array:
box.all("R > C") -> set_color(1, 0, 0)
```

---

## Selector syntax

### MVP note

For the first integrated world/subtree query path, prefer MMQ:

- `T`
- `T#hero`
- `Renderable`
- `Renderable#left_hand`

This is enough to support live subtree lookup for `ComponentObject` handles without
blocking on the full CSS-style selector feature set below.

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

The `->` operator follows the same convention: `"#id" ->` desugars to `query()`;
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

// -> query operator (always a query/dispatch — no string-literal detection needed):
"selector" -> handler               // → query("selector", handler)
"selector" -> method(args)          // → query("selector", fn(r) { r.method(args) })
comp_obj -> "selector" -> handler   // → comp_obj.query("selector", handler)  (ComponentObject LHS = subtree query)

// standard forward pipe (function application — unrelated to query):
expr |> f                           // → f(expr)
a |> f |> g                         // → g(f(a))
```

---

## Open questions

1. **Single/multi heuristic for `->` sugar** — is inferring `query` vs `query_all` from
   the selector shape (`#id` → single, else → all) too magic? Alternative: always
   `query_all` (never null, always iterate), and the caller uses `[0]` for the single case.

2. **Module namespace import** — `import "level.mms" as scene` is required for
   `scene.query(...)`. This import form is not in v1. When added, it produces a `Value::Module`
   that exposes `.query()` and `.query_all()`.

3. **Pre-spawn mutation on module CEs** — if you query a module and want to modify a CE
   before spawning it (e.g. change a color), what's the API? Probably CE-specific setters
   that mutate the AST node before it's emitted. Distinct from the live `.set_color()` on
   a `ComponentObject`. Design deferred.

4. **Selector scope root** — `scope.query("T C")` — does `T` need to be a direct or
   indirect descendant of `scope`? Current proposal: `T C` means C anywhere inside
   the T, and T itself can be anywhere inside scope. Consistent with CSS descendant rules.

5. **Live query subscription / reactive** — `observe "#selector" -> fn(r) { }` — register
   a handler that fires whenever the world's matching set changes (spawn/despawn). Deferred;
   document the reserved keyword.

6. **Query result ordering** — `query_all(".T")` — in what order are results returned?
   Depth-first tree order seems natural. Document the guarantee once implemented.
