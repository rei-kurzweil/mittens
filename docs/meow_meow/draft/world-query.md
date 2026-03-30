# ᓚᘏᗢ MMS World Query — Design Draft

> **Status: draft.** Nothing here is implemented.
> Explores the "find components in the live world and act on them" pattern —
> the complement to `component-addressing.md` (which covers navigating a tree
> you already have a handle on).

---

## The gap

Direct addressing works when you spawned the component and captured the handle:

```mms
let hero = T.position(0, 1, 0) { R { CUBE } }   // you made it; you have hero
hero[0].set_color(1, 0, 0, 1)                    // navigate by index
```

But often you don't have a prior handle:

- A handler script running in response to a tick event wants to nudge all `Enemy` nodes
- A spawned prefab wants to find its own parent or a sibling by name
- A UI panel wants to locate a named component anywhere in the scene
- A post-process effect wants every `R` with a given material flag

The pattern needed: **describe what you're looking for → get handles → act on them.**
This is world query.

---

## Two kinds of query (important distinction)

### Module query (existing concept, not this doc)

Querying a `.mms` file's **output tree** — what a module emitted. Operates on the CE
AST before or during spawn. Static — the scene doesn't need to be running.

```mms
let scene = import "level.mms"
scene.query("T")          // all TransformComponents in the module output
scene[0].query(".R")      // CEs inside the first emitted root
```

### World query (this doc)

Querying the **live ECS world** — components that are running in the engine right now.
Runtime. Requires a HostCall (engine state). Returns live `ComponentObject` handles.

```mms
query("#hero T")           // live TransformComponent named "hero" in the running world
```

The rest of this doc is about world query only.

---

## Core syntax: the `->` select-and-act form

```mms
"#hero_transform" -> set_position(0, 1, 0)

"#hero_transform" -> fn(t) {
    if !t { return }
    t.set_position(0, 1, 0)
}
```

`selector -> handler` is a statement. It means:

1. Evaluate `selector` as a world query → get a result (one component or many)
2. Call `handler` with the result
3. Discard the return value (it's a statement, not an expression)

The left side is a **selector** (string, or eventually native syntax).
The right side is either a **function literal** or a **method call shorthand**.

The `->` token is new. It reads as "send the result of this query into this handler" —
similar to a pipe (`|>`), but with the query-then-act semantics baked in.

---

## Selector syntax

Selectors are CSS-inspired strings. The engine's component tree is a tree of named nodes,
each with a type (TransformComponent, RenderableComponent, etc.) — the mapping onto CSS
is natural.

### Basic selectors

| Selector | Matches |
|---|---|
| `"#hero"` | The component node named `"hero"` |
| `".T"` or `"T"` | All TransformComponents in the world |
| `"#hero T"` | All TransformComponents descended from the node named `"hero"` |
| `"#hero > T"` | TransformComponents that are **direct children** of `"hero"` |
| `"T > C"` | ColorComponents that are direct children of any Transform |
| `"T C"` | ColorComponents anywhere inside any Transform subtree |

Component type names use the same short aliases as in MMS source (`T`, `R`, `C`, `I`, etc.)
and resolve through the same component registry.

### Attribute selectors (stretch)

```
"T[name=hero]"        — Transform with name attribute equal to "hero"
"R[material=glass]"   — Renderable with material flag "glass" set
```

Whether component nodes have queryable attributes (beyond type and name) depends on what
metadata the engine exposes. Defer until needed.

### World root vs subtree scope

By default, selectors search the **entire world**. To scope to a subtree:

```mms
hero -> ".T"       // find all T descendants of hero's subtree
```

When the lhs of `->` is a `ComponentObject` (not a string), the selector on the rhs
is interpreted relative to that subtree. This enables:

```mms
let hero = query("#hero")
hero -> ".R" -> fn(r) { r.set_color(1, 0, 0, 1) }    // chained: find hero, find Rs within, set color
```

---

## Single vs multiple results

Two query modes:

### `query(selector)` → single result (`ComponentObject?`)

Finds the **first** matching component. Returns `null` if none found. For unique-named
selectors (`"#hero"`).

```mms
let t = query("#hero_transform")   // ComponentObject? — null if not found
if t {
    t.set_position(0, 1, 0)
}
```

### `query_all(selector)` → multiple results (`[ComponentObject]`)

Finds **all** matching components. Returns empty array if none. For type or structural
selectors (`.Enemy T`).

```mms
let enemies = query_all(".Enemy T")
for e in enemies {
    e.set_position(0, 0, 0)   // reset all enemies to origin
}
```

The `->` shorthand chooses between them based on the selector:

- `"#name"` → implicitly `query()` (single; `#` implies unique)
- `".Type"` or any structural selector → implicitly `query_all()` (multiple)

Or we could require explicit `query` / `query_all` on the lhs and always use the callback
form on the rhs. TBD — see open questions.

---

## The callback form

```mms
"#hero_transform" -> fn(t) {
    if !t { return }
    t.set_position(0, 1, 0)
}
```

**Single query:** `t` is `ComponentObject?`. The `if !t { return }` guard is the standard
null-check pattern (same as any nullable in MMS).

**Multi query:** the callback is called once per result:

```mms
".Enemy T" -> fn(transform) {
    transform.set_position(0, 0, 0)
}
// equivalent to:
for t in query_all(".Enemy T") { t.set_position(0, 0, 0) }
```

The callback is a standard MMS function — it can contain any MMS code, not just mutation.
It can `return` (exits the current callback invocation), use `continue`/`break` in a loop,
call other functions, spawn new components, etc.

---

## The method shorthand

```mms
"#hero_transform" -> set_position(0, 1, 0)
```

Desugars to:

```mms
"#hero_transform" -> fn(t) {
    if !t { return }
    t.set_position(0, 1, 0)
}
```

The rhs method call `set_position(0, 1, 0)` is interpreted as a method name + args to
apply to each result. The receiver (`t`) is implicit.

For multi-match:

```mms
".Enemy T" -> set_position(0, 0, 0)
// desugars to: for each result, call result.set_position(0, 0, 0)
```

---

## `query()` as an expression

The `->` shorthand is convenient for fire-and-forget mutations. For more complex patterns,
`query()` and `query_all()` are first-class functions:

```mms
let hero = query("#hero_transform")
let enemies = query_all(".Enemy")

if hero {
    for e in enemies {
        let delta = hero.position() - e.position()   // hypothetical position() getter
        if length(delta) < 5.0 {
            e.set_color(1, 0, 0, 1)                  // enemies close to hero turn red
        }
    }
}
```

These are HostCalls — they cross the script/engine boundary. See
[function-dispatch.md](../spec/function-dispatch.md) dispatch kind 4.

---

## Static query vs reactive query

Everything above is **static** — run once when the statement is evaluated. Two richer
modes worth considering:

### Reactive: `on_match` / `observe`

```mms
// hypothetical
observe ".Enemy T" -> fn(t) {
    // called whenever a new component matches ".Enemy T"
    // (when an Enemy is spawned and gains a T child)
}
```

This is an observer pattern: the engine notifies MMS when the query result set changes.
Very powerful but requires engine-side query subscription infrastructure. Out of scope for
v1 world query; document the direction.

### Polling in a tick handler

A simpler reactive pattern without new engine machinery: call world query inside a tick
or animation callback that runs every frame.

```mms
// hypothetical tick handler
on_tick -> fn(dt) {
    ".Enemy T" -> fn(t) {
        t.translate(0, 0, dt)   // move all enemies forward each tick
    }
}
```

This is effectively the entity-system "system" concept expressed in MMS. The query runs
every tick and produces fresh results. No subscription needed.

---

## Relationship to HostCall dispatch

`query("#hero_transform")` has no MMS body. The engine must perform the lookup using live
ECS state. This is a HostCall (dispatch kind 4 in function-dispatch.md):

```mms
import { query, query_all } from "host"
```

The `->` select-and-act syntax desugars to `query`/`query_all` calls from `"host"`.

Per-target bindings:

| Target | How `query("#hero")` is fulfilled |
|---|---|
| cat-engine evaluator thread | `EvalResponse::Query { kind: QueryByName("hero") }` → wait for `EvalRequest::QueryResult` |
| cat-engine transpiled Rust | `engine.world().find_by_name("hero").map(ComponentObject)` |
| Offline / baked | Compile error — no live world at bake time |
| JavaScript / multiplayer server | TBD — depends on scene model |

---

## Selector string vs native syntax

Currently selectors are strings (`"#hero T"`). Strings are:
- Familiar (CSS muscle memory)
- Opaque to the type checker — the string content is not validated at parse time
- Easy to construct dynamically: `"#" + name`

An alternative is **native selector syntax** — first-class MMS syntax for selectors:

```mms
// hypothetical native syntax
#hero T -> set_position(0, 1, 0)   // # is a selector sigil, not part of ident
```

The `#` sigil is not a valid identifier start in MMS today, so it's available. But it
adds a new token kind and a separate parsing mode. The parser would need to distinguish
`#hero` (selector) from the start of any other expression.

One option: selectors in `->` position are always parsed as selectors (the parser knows
the lhs of `->` is a selector context). Elsewhere, `query("...")` takes a string.

**Recommendation for v1:** string selectors only. Less ambiguity, easier to implement,
composable with string operations. Native syntax is a v2 consideration.

---

## Comparison with component-addressing.md

| | Direct addressing | World query |
|---|---|---|
| Starting point | You have a `ComponentObject` handle | You describe what you want |
| Navigation | `[n]` child index | Selector string matching name, type, structure |
| When you use it | After spawning; tree structure is known | When you don't have a handle, or structure is unknown |
| Fragility | Breaks if tree structure changes | Resilient to tree shape; matches by description |
| Performance | O(1) — direct pointer via id | O(world size) — scan or indexed lookup |
| Requires | Phase 6 (live ComponentObject) | Phase 6 + world query HostCall infrastructure |

Both are needed. Direct addressing is fast and precise when you own the component.
World query is flexible when you don't.

---

## Open questions

1. **`->` vs `|>`** — should the select-and-act operator be `->` (visually "route result
   into handler") or `|>` (pipe, common in FP)? `->` reads more imperative ("go there and
   do this"). `|>` reads more functional ("pass result along"). MMS is somewhere between.
   Also: `->` currently means function return type in the type system draft. Conflict?

2. **Implicit single vs multi from selector shape** — should `"#name" ->` implicitly use
   `query()` (single) and `".Type" ->` use `query_all()` (multiple)? Or always require
   explicit `query()`/`query_all()` on the lhs? Implicit is ergonomic but surprising.
   Explicit is verbose but clear.

3. **What does the callback receive for multi-match?** — one call per result (iterator
   style), or called once with an array? Iterator style is more composable with `break`/
   `continue`. Array style is simpler to reason about.

4. **Chaining** — `hero -> ".T" -> set_color(1,0,0,1)` — does this chain? The first `->`
   produces a result (ComponentObject or [ComponentObject]), the second `->` queries within
   that result's subtree, the third applies a mutation. This is powerful but the parsing
   and semantics need careful definition. Defer to v2.

5. **Getters vs mutations** — can the method shorthand call a getter? `"#hero" -> position()`
   — returns the position of the hero transform. What's the result of a `->` that calls
   a getter? Probably `->` should be statement-only (no return value), and getter use
   requires the expression form: `let pos = query("#hero").position()`.

6. **Naming** — `query` / `query_all` in `"host"` or `"engine"`? Or a dedicated
   `"world"` module (`import { query } from "world"`)? The "world" framing is clearer —
   it's querying the live scene world, not the engine internals.

7. **Reactive queries / observers** — worth designing now even if not implementing.
   The `observe` keyword or `on_match` form would subscribe to world changes. This requires
   the engine to maintain a registry of live queries and notify MMS when results change.
   Significant infrastructure — but the syntax should be reserved so `observe` doesn't
   become a user variable name.

8. **Selector namespacing** — `"#hero"` — is `hero` the component's `name` attribute, its
   type name, or some user-assigned ID? The engine's `ComponentNode` has a `name: Option<String>`
   field. Is that the `#id` target, or do we need a separate explicit ID system?
