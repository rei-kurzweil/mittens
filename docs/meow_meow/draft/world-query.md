# MMS World Query

> **Superseded by [mms-query.md](mms-query.md).**
>
> The original design in this file treated module query and world query as separate
> systems with separate APIs. That was wrong — they are the same query system applied
> to different subjects.
>
> **See [mms-query.md](mms-query.md) for the canonical query design.**

---

## Summary of the unification

| What changed | Old thinking | Unified |
|---|---|---|
| Selector syntax | Two systems, same string syntax | One syntax, defined once in mms-query.md |
| API | `query()` free fn for world; `mod.query()` separately | `thing.query()` on any subject; subject determines context |
| `->` operator | Custom statement form | Sugar for the optional callback param: `query(sel, fn)` |
| Module scope | `mod.query()` returns CEs, world returns ComponentObjects | Both use `.query()`; return type depends on subject |

The key insight: `->` exists to avoid the closing-paren problem, not to introduce a new
concept. `"#hero" -> fn(t) { ... }` is sugar for `query("#hero", fn(t) { ... })`.
A component or module on the left side of `->` is sugar for `.query()` on that subject.

---

## What this file previously said that is still accurate

### World query is a HostCall

`query()` and `query_all()` as free functions need live ECS state. They are HostCalls
(dispatch kind 4, [function-dispatch.md](../spec/function-dispatch.md)).

Module query is **not** a HostCall — it is a static walk of the CE tree in memory.

### Direct addressing vs query

This distinction from the original doc remains valid and is not covered by mms-query.md:

| | Direct addressing (`comp[n]`) | Query (`comp.query("sel")`) |
|---|---|---|
| Starting point | You have a handle, navigate by index | Describe what you want |
| Fragility | Breaks if tree structure changes | Resilient — matches by description |
| Performance | O(1) via stored child list | O(subtree size) scan |
| Precise | Exact — you know the topology | Flexible — topology may vary |

Both are needed. Use direct addressing when you own the tree and know its shape. Use
query when you don't have a handle, or when the tree shape is variable.

See [component-addressing.md](../analysis/component-addressing.md) for the full direct
addressing design.
