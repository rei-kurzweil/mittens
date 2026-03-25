# Component query selectors

This document proposes a synchronous, main-thread query API on `Universe` for finding components inside a subtree using **CSS-like selectors**.

The motivating use cases are things like:

```rust
universe.find_component(vtuber_component_id, "[name='J_Bip_L_Lower_Arm']")
universe.find_all_components(some_parent_id, ".transform .renderable")
```

No code changes are proposed here; this is a design/spec document only.

---

## 0. Unified query language — design direction note

> **Cross-cutting design note added later.** The query language described in this doc
> should be the **single** selector language used across all of cat-engine and MMS:
>
> - `World` / `Universe` live component queries (the main subject of this doc)
> - MMS module import / CE-tree queries (see `docs/meow_meow/analysis/module-import-export.md`)
> - Future REPL / editor find-in-scene commands
> - Any other place that needs to locate components or component expressions by structure
>
> The selector string grammar (`[name='foo']`, `.transform > .renderable`, etc.) is the
> same in all contexts. What varies is the **root** — where the search starts.
>
> ### Root as part of the query
>
> Currently the root is a separate `ComponentId` argument:
> ```rust
> world.find_component(root: ComponentId, selector: &str)
> ```
>
> The direction is to make the root **part of the query** — either embedded in the
> selector string, or as a field in a `ComponentQuery` struct — so the same query value
> can be passed around, stored, and executed in different contexts (world, MMS file,
> REPL):
>
> ```rust
> // Proposed query struct — root + selector together
> struct ComponentQuery {
>     root: QueryRoot,
>     selector: String,   // or a parsed Selector value
> }
>
> enum QueryRoot {
>     Component(ComponentId),                         // live world
>     MmsFile { path: String, index: Option<usize> }, // MMS file, optional CE index
>     Implicit,                                       // caller supplies root at execution time
> }
> ```
>
> The string representation of a rooted query (for editor/REPL/MMS use):
>
> ```text
> // Root by component name (within an already-known subtree):
> [name='avatar_root'] [name='J_Bip_L_Hand']
>
> // Root pinned to a specific MMS file emission index:
> "scene.mms"[0] [name='torso'] .transform
>
> // Root by GUID (stable across sessions, editor use):
> #550e8400-e29b-41d4-a716-446655440000 [name='spine']
> ```
>
> An `Implicit` root means "the caller provides the root when executing the query" —
> this is the current API behaviour and remains valid for programmatic use where a
> `ComponentId` is already in scope.
>
> ### Why this matters
>
> - A query written in an MMS script (`"scene.mms"[0] .transform`) is structurally
>   identical to a query run against the live world — only the root type differs.
> - Editor tools, REPL commands, and MMS scripts can share the same selector parser and
>   evaluator. Only the root resolver branch changes.
> - Queries become first-class values: storable in variables, passed to functions,
>   serialized to disk.
>
> This is a **design direction**, not yet implemented. The rest of this doc specifies
> the selector grammar; the root encoding is an open design question (§17).

---

## 1. Current state

Today the publicly encouraged read-only query surface is very small:
- `Universe::parent_of(id)`
- `Universe::children_of(id)`
- `Universe::get_component_by_id_as::<T>(id)`

That is enough for systems and hand-written traversal code, but it is not ergonomic for:
- finding a specific named bone inside a spawned glTF subtree,
- finding all renderables beneath a transform subtree,
- authoring/editor tools that want concise structural queries,
- future scripting / inspector workflows.

So the answer to “do we already have this?” is effectively:

> No — we have basic topology primitives, but not a selector/query language.

---

## 2. Goal

Provide a small, synchronous, main-thread query API on `Universe` that:
- searches **within a subtree root**,
- uses **selector strings** inspired by CSS,
- returns component IDs,
- is easy to use from examples/tools/editor code,
- does not require callers to manually write DFS/BFS traversal boilerplate.

This is a **query API**, not a mutation API.

It should follow the same design direction as the existing `Universe` wrappers:
- common/public operations live on `Universe`,
- raw `World` access remains available internally but is not the primary user-facing path.

---

## 3. Proposed public API

Minimal first-pass API:

```rust
impl Universe {
    pub fn find_component(
        &self,
        root: ComponentId,
        selector: &str,
    ) -> Option<ComponentId>;

    pub fn find_all_components(
        &self,
        root: ComponentId,
        selector: &str,
    ) -> Vec<ComponentId>;

    pub fn matches_selector(
        &self,
        component: ComponentId,
        selector: &str,
    ) -> bool;
}
```

Semantics:
- `root` scopes the search to that component and its descendants.
- `find_component(...)` returns the **first match in a defined traversal order**.
- `find_all_components(...)` returns **all matches** in traversal order.
- `matches_selector(...)` checks whether a specific component matches a selector, independent of root-scoped search.

### Recommended traversal order

Use **preorder DFS**:
- visit node,
- then recursively visit children in stored child order.

Why:
- stable and predictable,
- aligns with existing topology order,
- natural for “first match” behavior.

---

## 4. Selector design goals

The selector language should be:
- familiar enough to read quickly,
- small enough to implement/debug,
- scoped to ECS topology rather than DOM/CSS styling.

This should be **CSS-like**, not “full CSS”.

That means:
- we borrow a few good ideas,
- we do not attempt to support the whole CSS grammar.

---

## 5. What selectors should match against

A component selector needs a few queryable facts.

## 5.1 Component kind / type

Examples:
- `transform`
- `renderable`
- `gltf`
- `controller_xr`

This corresponds to the runtime component type, e.g. `Component::name()`.

## 5.2 Component display/debug name

Examples:
- `J_Bip_L_Lower_Arm`
- `left_hand_target`
- `avatar_root`

This corresponds to a human-readable node/component name if one exists in the world record.

## 5.3 Topology relationships

Examples:
- descendant of X
- direct child of X
- same-subtree under root

These come from the component tree structure itself.

## 5.4 Future optional attributes

Possibly later:
- GUID
- initialized state
- enabled/disabled flags
- component-specific properties

But these should not be part of v1 unless we really need them.

---

## 6. Proposed v1 selector syntax

## 6.1 Type selector

Match component kind/type name.

Examples:

```text
transform
renderable
gltf
controller_xr
```

Interpretation:
- matches components whose `Component::name()` equals that identifier.

### Optional sugar: `.transform`

The user suggested:

```rust
universe.find_all_components(root, ".transform .renderable")
```

To support that style, we can allow `.foo` as a synonym for a type selector.

Examples:
- `.transform`
- `.renderable`
- `.controller_xr`

This is intentionally CSS-ish, even though here “class” really means component kind.

Recommended rule:
- bare `transform` and dotted `.transform` both mean the same thing in v1.

---

## 6.2 Name attribute selector

Match a component’s debug/display name.

Examples:

```text
[name='J_Bip_L_Lower_Arm']
[name="J_Bip_L_Lower_Arm"]
```

Interpretation:
- matches if the component’s stored world name equals the given string.

This is the most important selector for imported armatures / glTF bone lookup.

### Note on the example typo

The prompt shows:

```text
[name='J_Bip_L_Lower_Arm]
```

That appears to be missing the closing quote. The intended syntax should be:

```text
[name='J_Bip_L_Lower_Arm']
```

---

## 6.3 Descendant combinator

A space means “descendant of”.

Example:

```text
.transform .renderable
```

Interpretation:
- find renderables that are descendants of a transform node,
- within the search root.

Another example:

```text
[name='avatar_root'] .renderable
```

Interpretation:
- find any renderable under the named `avatar_root` component.

---

## 6.4 Direct-child combinator

Use `>` for immediate child.

Example:

```text
.transform > .renderable
```

Interpretation:
- match renderables that are **direct children** of a transform.

This is especially useful because ECS topology often distinguishes:
- immediate attached helper/style components,
- deeper descendants.

---

## 6.5 Compound selector

Allow multiple simple tests on the same component.

Examples:

```text
.transform[name='left_hand_anchor']
renderable[name='sword_mesh']
```

Interpretation:
- the component must satisfy **all** simple tests in the compound selector.

This is enough for the common “kind + exact name” case.

---

## 7. Proposed v1 grammar surface

A deliberately small v1 could be:

```text
selector        := sequence (combinator sequence)*
combinator      := '>' | whitespace
sequence        := simple+
simple          := type_selector | class_type_selector | attr_selector
type_selector   := ident
class_type_selector := '.' ident
attr_selector   := '[' 'name' '=' string ']'
```

Where:
- `ident` is an ASCII-ish component identifier like `transform` or `controller_xr`
- `string` supports `'...'` or `"..."`

This is intentionally limited.

No v1 support for:
- comma selector groups,
- sibling combinators,
- `:nth-child`,
- wildcard `*`,
- regex matches,
- arbitrary attribute names,
- boolean expressions.

Those can come later if needed.

---

## 8. Matching examples

## Example A: named lower arm bone

```rust
let lower_arm = universe.find_component(
    vtuber_component_id,
    "[name='J_Bip_L_Lower_Arm']",
);
```

Meaning:
- search the subtree rooted at `vtuber_component_id`,
- return the first component whose world/display name exactly matches that bone name.

## Example B: all renderables under transforms

```rust
let renderables = universe.find_all_components(
    some_parent_id,
    ".transform .renderable",
);
```

Meaning:
- within `some_parent_id`’s subtree,
- find renderables that have a transform ancestor in the matched chain.

## Example C: direct renderable child of a named node

```rust
let direct_renderable = universe.find_component(
    root,
    "[name='weapon_root'] > .renderable",
);
```

Meaning:
- find a renderable attached directly under the node named `weapon_root`.

## Example D: named transform specifically

```rust
let wrist = universe.find_component(
    avatar_root,
    ".transform[name='J_Bip_L_Hand']",
);
```

Meaning:
- exact name match, but constrained to transform components only.

---

## 9. Why scope the search by root

This API should always take a `root` component.

Reasons:
- avoids accidental global searches,
- makes queries naturally local to an avatar/widget/prefab/editor subtree,
- fits the ECS mental model of subtree ownership,
- gives predictable performance bounds,
- aligns with how `Universe::parent_of` / `children_of` already encourage scoped world inspection.

If someone wants a global search, they can pass an actual scene root.

---

## 10. Error handling / parse behavior

Selector parsing should be explicit.

There are two reasonable public API options.

## Option A: infallible search, invalid selector = no match + log

```rust
find_component(root, selector) -> Option<ComponentId>
find_all_components(root, selector) -> Vec<ComponentId>
```

Pros:
- simple call sites.

Cons:
- parse errors are hard to distinguish from “no match”.

## Option B: fallible API

```rust
find_component(root, selector) -> Result<Option<ComponentId>, SelectorParseError>
find_all_components(root, selector) -> Result<Vec<ComponentId>, SelectorParseError>
```

Pros:
- much better debuggability,
- especially important if selectors are editor-authored or script-authored.

Cons:
- slightly noisier call sites.

### Recommended direction

Use the fallible form internally and probably publicly too:

```rust
Result<_, SelectorParseError>
```

That is the better long-term API.

---

## 11. Performance / implementation direction

A first implementation can be simple:
- parse selector string into a tiny selector AST,
- run DFS from `root`,
- test each candidate against the selector chain.

That is likely good enough for:
- editor tools,
- avatar/bone lookup,
- occasional subtree queries.

If it becomes hot later, we can add:
- parsed selector caching,
- name/type indexes inside a subtree cache,
- global indexes for named components.

But v1 does not need pre-optimization.

---

## 12. Relationship to existing topology-query docs

This proposal is complementary to [docs/refactor/topology-queries-and-style-inheritance.md](docs/refactor/topology-queries-and-style-inheritance.md).

That refactor note is about:
- internal helper traversal APIs,
- style inheritance resolution,
- reducing duplicated ancestor/descendant walks.

This selector-query spec is about:
- a public-ish ergonomic `Universe` query surface,
- subtree search by declarative selector strings,
- editor/tooling/script ergonomics.

A likely implementation would reuse the lower-level topology-query helpers proposed there.

---

## 13. Why CSS-like selectors are a good fit here

They are not perfect, but they buy a lot:
- familiar mental model,
- compact syntax,
- easy to read in debug tools / editor commands,
- natural expression of parent/child/descendant relationships.

And because this is an ECS component tree, the most useful parts of CSS already map well:
- node kind/type selector,
- attribute exact-match selector,
- descendant combinator,
- direct-child combinator.

That gives us most of the value without dragging in all of CSS.

---

## 14. Out of scope for v1

Not needed yet:
- full CSS grammar,
- wildcard `*`,
- comma groups (`.transform, .renderable`),
- sibling selectors (`+`, `~`),
- pseudo-classes,
- regex or substring matching,
- component-property inspection like `[enabled=true]`,
- off-thread selector queries.

Also out of scope here:
- mutation selectors like “remove all matching nodes” or “attach under matching parent”.

This is strictly a synchronous query API spec.

---

## 15. Future extensions

Once the base works, likely good additions are:
- `*` wildcard
- `[guid='...']`
- `[type='transform']`
- substring operators like `[name*='Lower_Arm']`
- query caching / compiled selectors
- off-thread selector query intents for future MMS/VM worker usage
- editor inspector / REPL commands that accept the same selector syntax

Example future REPL command:

```text
find [name='J_Bip_L_Lower_Arm']
find-all .transform > .renderable
```

---

## 17. Root encoding — open design question

See §0 for context. The question is: what is the string syntax for a self-contained,
rooted query that works across both the live world and MMS file contexts?

Options:

### Option A: Root as a leading token in the selector string

```text
"scene.mms"[0] [name='torso'] .transform   // MMS file root
#<guid> [name='spine']                      // world root by GUID
```

**Pros:** single string, no extra API surface.
**Cons:** parser must handle two different leading token types; `"..."` already means
something in MMS.

### Option B: `ComponentQuery` struct with explicit root field

```rust
ComponentQuery::new(QueryRoot::MmsFile { path: "scene.mms", index: Some(0) }, "[name='torso'] .transform")
ComponentQuery::new(QueryRoot::Component(id), "[name='spine']")
```

**Pros:** clean separation of root and selector; typed, no parsing ambiguity.
**Cons:** less ergonomic for one-liners; not naturally writable in MMS syntax.

### Option C: Method call on a loaded/resolved root

```mms
load("scene.mms")[0].query("[name='torso'] .transform")
universe.root(avatar_id).query("[name='spine']")
```

**Pros:** feels natural in MMS and in Rust; root and selector are clearly separate;
the loaded module is already the root, no encoding needed.
**Cons:** two-step for ad-hoc cases.

**Leaning:** Option C for the primary API; Option B as the underlying struct that
`.query(...)` constructs. Option A (string-embedded root) probably only for
editor/REPL serialization where a query needs to be a bare string.

### Root is always optional at the selector level

When the context already implies a root (a live `ComponentId`, an open MMS module, the
current scene), the root specifier can simply be omitted — the selector string is just
the CSS-like part:

```text
// No root prefix — caller supplies root:
[name='J_Bip_L_Hand']
.transform > .renderable

// With explicit root for a world component (GUID-based):
#550e8400-e29b-41d4-a716-446655440000 [name='spine']

// With explicit root for an MMS file:
"scene.mms"[0] [name='torso'] .transform
"scene.mms" .transform           // root = whole file's emission sequence
```

The rule: **if the query string starts with `"..."` it is an MMS file root; if it
starts with `#<guid>` it is a world-component root; otherwise the root is `Implicit`
and must be provided by the call site.** Internal / live-world queries almost always
use `Implicit` — the caller already has a `ComponentId` in scope and passes it
separately (or via `.query(...)` on a resolved handle).

This keeps the common case (`world.query(id, "[name='foo']")`) clean while allowing
fully self-contained queries when needed.

---

## 16. Recommended current stance

The best next-step API is:
- add subtree-scoped selector queries on `Universe`,
- use a small CSS-like selector language,
- support exact-name matching and descendant/direct-child queries first,
- return component IDs,
- keep the implementation simple and synchronous.

In short:

> We do not have this query API yet, and it would be very useful — especially for avatar/bone lookup and editor tooling.
