# ComponentRef And MMQ Relative Root Prefixes

Date: 2026-06-23

Related analysis:

- [component-ref-relative-scope-use-cases.md](/home/rei/_/cat-engine/docs/analysis/component-ref-relative-scope-use-cases.md)

## Summary

This draft proposes a shared root-prefix model for selector-backed `ComponentRef` queries and MMQ
 v2 queries.

The goal is to make local authored references ergonomic while still allowing explicit upward and
 world-rooted lookup without requiring an adjacent scope parameter in the common cases.

Proposed root contexts:

1. **Implicit local subtree**: bare selector string, e.g. `#xr_pose`
2. **Parent-relative**: `../...`
3. **World-rooted**: `/...`

This draft also proposes:

- whitespace after the root prefix is **optional**
- world-root direct-child queries should use the direct-child combinator form, e.g. `/ > #something`
- `/#something` and `/ #something` should both mean "search anywhere under the world root"
- repeated leading parent climbs such as `../../#something` are allowed as root selection
- mid-query `../` traversal is **not** part of the first implementation

## Motivation

The strongest pressure comes from selector-backed component-reference APIs such as:

- `translation_basis(...)`
- `TransformParent.target(...)`
- `IKChain.target(...)`
- `Selection.root(...)`
- `Animation.scope(...)`

These are not arbitrary world queries. They are durable, pointer-like references authored inside a
 component tree. In those cases, a local implicit root is usually what the author means.

The related use cases are laid out in:

- [component-ref-relative-scope-use-cases.md](/home/rei/_/cat-engine/docs/analysis/component-ref-relative-scope-use-cases.md)

## Root Prefix Model

### 1. Bare query string: local subtree

```mms
translation_basis("#xr_pose")
target("#hand_target")
scope("#avatar_root")
```

Meaning:

- search within the subtree rooted at the component that owns the `ComponentRef`
- matching should be **self-inclusive**: the owner root may match if the selector matches it

For `ComponentRef` resolution, this becomes the default.

### 2. Parent-relative: `../`

```mms
translation_basis("../#xr_pose")
translation_basis("../ #xr_pose")
scope("../#avatar_root")
```

Meaning:

- change the query root from the referencer-owned subtree to the referencer's parent scope
- then evaluate the remaining selector from there

Whitespace after `../` is optional:

- `../#xr_pose`
- `../ #xr_pose`

Both are equivalent.

### Repeated leading parent climbs

```mms
../../#xr_pose
../../ #xr_pose
```

Meaning:

- climb upward from the local referencer scope before selector evaluation begins
- then evaluate the remaining selector from that computed root

This is still part of **root selection**, not a query-step operator.

### 3. World-rooted: `/`

```mms
translation_basis("/#xr_pose")
translation_basis("/ #xr_pose")
```

Meaning:

- start from the world root set instead of the referencer-local subtree

Whitespace after `/` is optional:

- `/#xr_pose`
- `/ #xr_pose`

Both are equivalent.

## Direct-Child Semantics At World Root

This draft distinguishes:

- **world-root descendant search**: `/#something` or `/ #something`
- **world-root direct-child search**: `/ > #something`

### Descendant search

```mms
/#something
/ #something
```

Meaning:

- start at the world root
- find any matching component anywhere under any world root

This is analogous to today's global selector walk.

### Direct-child search

```mms
/ > #something
/> #something
/>#something
```

Meaning:

- start at the world root
- only match components that are direct children of a world root

This is useful when the author wants:

- "the top-level scene child named `something`"

but not:

- "any descendant anywhere in the world named `something`"

This should follow the same semantics as the direct-child combinator in the shared query system.

## Whitespace Rules

Whitespace immediately after the root prefix should be optional.

Equivalent examples:

```text
#hero
../#hero
../ #hero
../../#hero
../../ #hero
/#hero
/ #hero
/>#hero
/> #hero
/ > #hero
```

The parser should normalize optional whitespace between:

- `../` and the next selector/combinator token
- `/` and the next selector/combinator token

Whitespace inside the remainder of the selector/MMQ expression should continue following the normal
 query-language rules.

## Proposed Semantics For ComponentRef

`ComponentRef::Query(String)` should be interpreted as a scoped query string with an implicit root
 mode.

Conceptually, parsing should produce something closer to:

```rust
enum QueryRootMode {
    SelfSubtree,
    ParentScope { levels_up: usize },
    WorldRoot,
}

struct ScopedQuery {
    root_mode: QueryRootMode,
    selector: String,
}
```

Notes:

- the stored on-disk form can remain a single authored string
- the runtime resolver should parse out the root prefix before passing the remainder into the
  selector/MMQ parser
- bare strings default to `SelfSubtree`
- one or more leading `../` segments should collapse into `ParentScope { levels_up: n }`
- once the selector body begins, `../` is no longer interpreted by the v1 resolver

## Proposed Semantics For MMQ v2

MMQ v2 should support the same root-prefix model for consistency:

- bare query = relative to the current subject/root
- `../...` = evaluate from the parent scope of that subject
- `/...` = evaluate from the world root

This should be treated as a **root-selection layer** above the MMQ body, not as an ordinary MMQ
 selector token.

That keeps the shared mental model simple:

- `#thing` = local
- `../#thing` = parent-relative
- `../../#thing` = climb multiple parent scopes before evaluation
- `/#thing` = world-rooted

## Receiver-Style Queries

Receiver-style APIs such as:

```mms
avatar.query("#wrist")
avatar.query("../#wrist")
avatar.query("/#wrist")
```

should reuse the same root-mode machinery, but with one nuance:

- the receiver supplies the base subject/root for the bare local case

So:

- `avatar.query("#wrist")` means subtree-local under `avatar`
- `avatar.query("../#wrist")` means relative to `avatar`'s parent scope
- `avatar.query("/#wrist")` means world-rooted, ignoring the local receiver root except as call
  context

## Examples

### Example 1: Input translation basis

```mms
I.speed(1.0) {
    InputTransformMode.forward_z() {
        rotation_disabled()
        translation_basis("../#xr_pose")
    }

    T {
        InputXR.on() {
            T {
                name = "xr_pose"
                AVC { }
            }
        }
    }
}
```

Interpretation:

- `InputTransformMode` owns the reference
- bare local subtree would be too narrow
- `../` moves the query root to the containing `Input` subtree parent scope

### Example 2: world-rooted top-level node only

```mms
Selection.root("/ > #scene_root") { }
```

Interpretation:

- do not match nested descendants named `scene_root`
- only match a direct child of a world root

### Example 3: world-rooted anywhere

```mms
Selection.root("/#scene_root") { }
```

Interpretation:

- match any descendant anywhere under the world root set

## Resolution Model

The runtime should resolve scoped queries in two phases:

1. Parse root prefix into root mode.
2. Resolve the selector/MMQ body against the computed root.

For `ComponentRef`:

- `SelfSubtree` → use the referencer component as subtree root
- `ParentScope` → use the referencer's parent as subtree root
- `WorldRoot` → use the world roots

For MMQ v2:

- bare local root is whatever subject/root the query API provides
- parent-relative and world-rooted forms override that local root selection

Important v1 limitation:

- the root prefix is resolved **before** selector/MMQ evaluation
- it is not re-applied after intermediate matches
- therefore `../` in v1 is a root-selection feature, not a path-step combinator

## Compatibility Direction

This draft intentionally changes the meaning of bare `ComponentRef::Query` strings from today's
 effectively-global resolution to local-subtree resolution.

That change is desirable for pointer-like authored references, but it is a behavior change and
 should be rolled out deliberately. APIs that truly want global lookup should author `/...`
 explicitly.

## Non-Goal For First Implementation: Mid-query Upward Traversal

This draft does **not** propose that queries such as:

```text
../ #something ../ #something_else
```

work in the first implementation.

That syntax implies a different evaluation model:

1. choose a root
2. match `#something`
3. move to the parent of each match
4. evaluate `#something_else` downward from there

That is not just root-prefix parsing. It is an **upward combinator / path-step** feature inside the
 query body.

The current query AST and evaluator only model downward combinators (`Descendant` and `Child`), so
 this feature should be treated as a later extension.

### Allowed in v1

```text
#something
../#something
../../#something
/#something
/ > #something
```

These are all root-selection forms.

### Not part of v1

```text
#something ../ #something_else
../ #something ../ #something_else
```

These require a future query-step traversal model.

## Open Questions

1. For `WorldRoot`, should the implementation evaluate against:
   - a synthetic super-root whose children are all top-level world roots, or
   - each world root independently with merged results?
2. If MMQ v2 adds non-CSS syntax, should the root-prefix stripping happen before or after MMQ
   tokenization? Current preference: before.
3. Should bare `ComponentRef` local-subtree matching include the owner component itself? Current
   preference: yes.
4. Do any existing `ComponentRef` consumers need to preserve global semantics temporarily during
   migration, or can all global callers be rewritten to `/...` explicitly?
5. When upward traversal inside the query body is designed later, should it reuse `../` or use a
   separate explicit combinator/operator to avoid conflating root selection with traversal steps?

## Recommended First Adoption Order

1. Parse and resolve scoped `ComponentRef::Query` strings through a shared helper.
2. Adopt the new semantics first in new APIs such as `translation_basis(...)`.
3. Audit existing `ComponentRef` consumers and convert any global-intent call sites to explicit
   `/...`.
4. Reuse the same root-prefix helper when MMQ v2 lands so selector-backed references and MMQ share
   one mental model.
