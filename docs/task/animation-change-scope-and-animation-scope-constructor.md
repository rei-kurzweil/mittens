# AnimationChangeScope And `Animation.scope(...)`

Task doc for making animation target resolution explicitly scoped and movable at runtime.

This doc is about animation scoping generally, not pose capture specifically.

## Goal

Support authored animation like:

```mms
Animation.scope("#avatar_root") {
    Keyframe.at(0) {
        Action.set_position("#Hand.R", [0, 0, 0])
    }
}
```

and support runtime rebinding like:

```rust
IntentValue::AnimationChangeScope {
    animation: some_animation_component,
    scope: some_component_tree_root,
}
```

so one animation subtree can be attached to an arbitrary component tree at runtime without globally ambiguous target resolution.

## Problem

Current `ActionComponent` target resolution has two modes:

- `ResolveTargetsMode::OnAttach`
- `ResolveTargetsMode::OnPlay`

but both ultimately resolve queries by walking world roots. That is not enough for reusable authored animation because:

1. the same animation subtree can be instantiated multiple times
2. target names like `#Hand.R` are only meaningful relative to one avatar subtree
3. a global world-root query can bind to the wrong instance
4. later retargeting to another live subtree has no explicit runtime hook

The existing animation docs already point toward animation-local scoping, but there is not yet:

- an `AnimationComponent` field for scope root
- an MMS constructor for scope
- an intent to change scope after spawn

## Proposed API

## `Animation.scope(target)`

Add a constructor/builder surface for authored MMS:

```mms
Animation.scope("#avatar_root") {
    Keyframe.at(0) { ... }
    Keyframe.at(1) { ... }
}
```

Accepted input should match normal component-reference authoring conventions:

- selector string like `"#avatar_root"`
- later possibly `@uuid:...`
- later possibly a live component object passed from MMS evaluation context

Recommendation:

- model this as authored unresolved scope source, not an eagerly resolved `ComponentId`

Suggested internal shape:

```rust
pub struct AnimationComponent {
    pub state: AnimationState,
    pub resolve_targets: ResolveTargetsMode,
    pub length_beats: Option<f64>,
    pub scope_source: Option<ComponentRef>,
    pub resolved_scope: Option<ComponentId>,
}
```

Where:

- `scope_source` is the durable authored form
- `resolved_scope` is the current runtime binding

## `IntentValue::AnimationChangeScope`

Add a runtime intent:

```rust
IntentValue::AnimationChangeScope {
    animation: ComponentId,
    scope: ComponentId,
}
```

Meaning:

- set the runtime scope root for this animation to `scope`
- clear any cached query resolution that depended on the old scope
- future action resolution should happen relative to the new scope

This intent is the runtime rebinding primitive.

## Semantics

## Scope meaning

The animation scope is the root component under which `ComponentRef::Query` lookups for child `ActionComponent`s should be resolved.

That means:

- if an action targets `#Hand.R`
- and the animation scope is one avatar root
- the lookup searches only inside that avatar subtree

It should not search from world roots unless no scope is defined and we are preserving legacy behavior.

## Precedence

Recommended precedence:

1. if `AnimationChangeScope` has set a runtime scope, use that
2. else if `Animation.scope(...)` authored a `scope_source`, resolve and use that
3. else fall back to legacy behavior

This lets authored animations define a default scope while still allowing runtime retargeting.

## Resolution timing

The scope concept should work with both existing action target resolution modes.

### `ResolveTargetsMode::OnAttach`

At animation registration / first processing:

1. resolve animation scope
2. resolve all child actions relative to that scope
3. cache the concrete target ids

If scope later changes:

1. clear cached action resolutions
2. force re-resolution against the new scope before next playback/use

### `ResolveTargetsMode::OnPlay`

At action fire time:

1. resolve animation scope
2. resolve action targets relative to that scope
3. fire the action

This mode is naturally compatible with changing scope, but still needs the current scope lookup path.

## Recommended initial behavior on scope change

When `AnimationChangeScope` is received:

1. update `resolved_scope`
2. if the animation is `OnAttach`, mark all child actions unresolved again
3. clear any runtime cached target ids derived from old scope
4. do not implicitly restart playback

Playback restart should remain a separate concern.

## Query behavior

## Relative query root

The missing engine seam is query evaluation relative to a provided root. Current `ComponentRef::Query` resolution used by animation actions walks world roots.

We need a shared helper shaped more like:

```rust
fn resolve_component_ref_in_scope(
    world: &World,
    scope_root: ComponentId,
    source: &ComponentRef,
) -> Option<ComponentId>
```

Rules:

- `Guid` can still resolve globally by guid map
- `Query(selector)` should search from `scope_root`

That gives a clean split:

- guid references stay absolute
- selector references become subtree-relative when scope exists

## Why scope belongs on `AnimationComponent`

The scope is an animation-level concern, not an action-level concern.

Reasons:

1. all actions inside one animation usually target one avatar/tree instance
2. authoring `Action.scope(...)` on every keyframe child would be noisy and repetitive
3. runtime retargeting usually means "move the whole animation to another avatar", not "change one action"

An animation-level scope also maps directly to the pose-capture phase-2 need: generate one animation from one pose library and bind it to one avatar subtree.

## Serialization shape

Not implementation-critical for this task, but the intended MMS surface should round-trip as:

```mms
Animation.scope("#avatar_root").looping() {
    Keyframe.at(0) {
        Action.set_position("#Hand.R", [0, 0, 0])
    }
}
```

or equivalently, depending on builder ordering rules:

```mms
Animation.looping().scope("#avatar_root") {
    ...
}
```

Recommendation:

- allow `.scope(...)` as a normal builder call on `Animation`
- store it in `AnimationComponent::to_mms_ast()`

## Intent execution

`AnimationChangeScope` should likely be handled as a direct mutation intent, not as a userland `ActionComponent` semantic expansion.

Reasons:

- it mutates animation runtime binding state
- it should invalidate cached animation resolution state
- it does not naturally map to a low-level world mutation like `UpdateTransform`

That suggests:

- add the intent variant
- handle it in mutation execution or a small animation-specific intent path
- notify `AnimationSystem` runtime state as needed

## Runtime state implications

Today `AnimationSystem` caches:

- registered keyframes
- fired state
- attach-resolved action state

Scoped rebinding means it may also need to treat target resolution caches as invalidatable.

Recommended minimal addition:

- a per-animation "resolution generation" or simple `attach_resolved = false` reset when scope changes

No broader playback-state reset should happen unless explicitly requested.

## Open questions

## 1. should scope be stored as `ComponentRef` or only as runtime `ComponentId`?

Recommendation:

- both

Reason:

- `ComponentRef` is needed for authored MMS round-trip
- `ComponentId` is needed for efficient runtime use

## 2. what happens if the authored scope query stops resolving?

Recommendation:

- treat it as unresolved scope
- log once or on bind attempts
- skip firing unresolved query-targeted actions rather than binding globally by surprise

Do not silently fall back to world-root resolution once a scope was explicitly requested.

## 3. should `AnimationChangeScope` accept a query instead of `ComponentId`?

Recommendation:

- first version should accept `ComponentId`

Reason:

- the caller already knows the live subtree root when rebinding at runtime
- it avoids nesting another resolution layer inside the rebinding intent

If needed later, add a separate authored/runtime API for query-based scope change.

## 4. should scope affect only `Query(...)` or also `Guid(...)`?

Recommendation:

- only `Query(...)`

Guid references are already absolute and durable by design.

## Proposed implementation order

1. extend `AnimationComponent` with authored scope source + runtime resolved scope
2. add `Animation.scope(...)` builder/constructor support in the component registry
3. update `AnimationComponent::to_mms_ast()` to emit scope when present
4. add `IntentValue::AnimationChangeScope`
5. add a helper to resolve `ComponentRef` relative to a provided scope root
6. update `AnimationSystem` action resolution to use animation scope when present
7. invalidate cached attach-time resolutions when scope changes
8. add tests covering:
   - two avatar instances with one reused animation prefab
   - runtime rebinding from avatar A to avatar B
   - `OnAttach` and `OnPlay` behaviors
   - unresolved scope failure behavior

## Relation to pose capture

This task is a dependency for the cleaner version of pose-capture phase 2 animation generation.

Without scoped animation resolution, generated animations from pose libraries either:

- must bake live `ComponentId`s and stay scene-local, or
- risk binding queries to the wrong instance

With this task done, pose-generated `Animation {}` trees can be authored against subtree-relative selectors and rebound onto arbitrary avatars more safely.
