# Component query selectors

This document proposes a synchronous, main-thread query API on `Universe` for finding components inside a subtree using **CSS-like selectors**.

The motivating use cases are things like:

```rust
universe.find_component(vtuber_component_id, "[name='J_Bip_L_Lower_Arm']")
universe.find_all_components(some_parent_id, ".transform .renderable")
```

No code changes are proposed here; this is a design/spec document only.

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

## 16. Recommended current stance

The best next-step API is:
- add subtree-scoped selector queries on `Universe`,
- use a small CSS-like selector language,
- support exact-name matching and descendant/direct-child queries first,
- return component IDs,
- keep the implementation simple and synchronous.

In short:

> We do not have this query API yet, and it would be very useful — especially for avatar/bone lookup and editor tooling.
