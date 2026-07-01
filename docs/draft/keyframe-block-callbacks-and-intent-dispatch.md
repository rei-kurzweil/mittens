# Keyframe Blocks Instead of Action Components

## Summary

Retire the `ActionComponent` animation model and replace it with **timed keyframe
blocks**:

- `Keyframe { beat: ... }` remains the timing primitive.
- The body of a keyframe is an imperative block.
- When the keyframe becomes due, the entire block executes immediately.
- Work inside the block happens by calling methods on **live component
  references**.
- Those methods do not mutate the world directly; they dispatch intents into the
  existing signal pipeline.

This keeps timing in the animation system, keeps mutation semantics in the
intent pipeline, and avoids needing an `Action.*` constructor for every
property of every component.

## Problem

The current animation stack is centered on stored `ActionComponent` payloads:

- `AnimationComponent`
- `KeyframeComponent`
- child `ActionComponent`s
- `AnimationSystem` firing those actions when a keyframe becomes due

That model has several problems.

### 1. It does not scale with component surface area

Every new mutable property wants:

- a new `Action.*` constructor or equivalent
- new target-resolution rules
- new serialization rules
- new evaluator rules
- often duplicated wiring that already exists somewhere else as a component
  method or intent

This grows a second API surface in parallel with the actual component/mutation
API.

### 2. It fights the live MMS object model

Recent MMS work is moving toward live component references and method calls on
those references. `ActionComponent` stores a detached description of something
to do later, while the newer object model wants:

- component expressions to materialize into live components
- references to keep pointing at those live components
- methods on those references to dispatch intents

`ActionComponent` is a serialized deferred command object in a system that is
otherwise trending toward live object references.

### 3. It forces target indirection where direct references are better

The current approach often stores selectors or component ids inside action
payloads and resolves them later. That creates scoping and lifecycle problems,
especially when the animated subtree is instantiated multiple times.

If the keyframe body already has live references to the components it wants to
drive, most of that indirection disappears.

### 4. It duplicates the intent API

We already have the engine’s mutation path:

- code obtains a live component reference
- a method on that reference emits an intent
- the intent pipeline executes the mutation at drain points

`ActionComponent` is effectively a second command language layered beside that.
The more complete the intent/method model becomes, the less value the action
layer provides.

## Proposal

Treat `Keyframe` as a timed callback block rather than a container for child
`ActionComponent`s.

Example authoring shape:

```mms
let glow = Emissive.on().intensity(0.2)

Animation.looping(duration_beats: 5.0) {
  Keyframe(beat: 0.0) {
    glow.set_intensity(2.5)
  }

  Keyframe(beat: 1.0) {
    glow.set_intensity(1.0)
  }
}
```

Semantics:

- the `Keyframe` body is stored as executable MMS code, not lowered into child
  `ActionComponent`s
- when the animation system reaches that beat, it invokes the block once
- every statement in the block executes in the same keyframe firing
- component methods inside the block require a live world / live component
  backing
- those methods emit intents rather than mutating components directly

The execution unit is the block, not an action payload component.

## Execution model

### Keyframe authoring

`Keyframe { ... }` should be treated as a closure-like body with access to the
captured lexical environment from the surrounding MMS scope.

That means keyframe code can refer to:

- local variables
- component references produced earlier in the same component body
- arrays / loops / helper functions, subject to normal MMS semantics

### Materialization order

The keyframe block must run only after the referenced components are live.

That implies:

1. component expressions referenced by the animation are pre-registered with a
   component id before attachment
2. once materialized, the corresponding MMS object reference resolves to the
   live component
3. keyframe callbacks are scheduled only in a runner mode that has:
   - live `RenderAssets` for procedural constructors
   - a live host world for method-dispatched intents

This is the same lifecycle requirement already exposed by recent MMS issues:
deferred code that calls component methods cannot run correctly against a purely
detached component-expression world.

### Runtime firing

When a keyframe becomes due:

1. `AnimationSystem` identifies the due keyframe.
2. The keyframe callback is invoked exactly once for that firing.
3. The callback evaluates its statements against the current live MMS/host
   context.
4. Component method calls emit intents.
5. Those intents drain through the normal signal pipeline.

Everything in the block is considered simultaneous at keyframe granularity.
Ordering within the block still exists for normal language semantics, but the
intended mental model is “these statements fire together at this beat.”

## Why this scales better

### Component methods become the authored API

Instead of inventing:

- `Action.set_color(...)`
- `Action.set_text(...)`
- `Action.set_emissive_intensity(...)`
- `Action.update_transform(...)`
- many more procedural animation-specific constructors

we expose:

- `color.set_rgba(...)`
- `text.set_text(...)`
- `emissive.set_intensity(...)`
- `transform.set_position(...)`

That is the same API shape scripts, handlers, and future tools should use
outside animation.

### Coverage grows with component capability, not animation-specific wrappers

If a component has a live method that emits the right intent, it is already
usable inside keyframes. Animation support stops being a bespoke integration
task for each property.

### Scope becomes lexical instead of selector-driven

The keyframe block can close over the exact component refs it cares about. That
is a better fit for repeated factory instantiation than global or animation-root
selector lookup.

## Relationship to intents

This proposal does **not** replace the intent system.

It replaces the `ActionComponent` layer with direct method-dispatched intents.

Desired layering:

- MMS keyframe block expresses *when* a set of operations should fire
- component methods express *what intent* should be emitted
- the intent pipeline remains the only mutation path

This aligns with the direction in
[`docs/analysis/intent-migration-audit.md`](../analysis/intent-migration-audit.md):
reduce parallel mutation entry points and make side effects flow through one
executor path.

## Required runtime capabilities

### 1. Live component references in MMS

Component-valued variables captured by a keyframe must resolve to a live
component object by the time the keyframe fires.

### 2. Callback storage for keyframe bodies

The engine needs to store executable keyframe bodies, not only static action
payload data. That likely means a compiled block / closure representation owned
by the MMS runner or animation runtime rather than ECS child components.

### 3. Runner modes must be explicit

A runner that evaluates animation callbacks must be configured with:

- host world access
- component method dispatch
- procedural renderable construction support when the authored scene needs it

If those capabilities are absent, keyframe callbacks should fail at setup time,
not later during playback.

### 4. Method vocabulary must map to intents

For this to scale, component methods used from MMS should be thin intent
emitters. Avoid direct mutation in those methods except where the engine
explicitly treats something as local non-world state.

## Migration shape

### Phase 1: document the new semantic model

- `ActionComponent` becomes legacy.
- New animation authoring guidance uses `Keyframe { ... }` blocks.
- MMS docs should state clearly that component methods inside timed/evented
  blocks require live component references and a live host world.

### Phase 2: support keyframe callback storage/execution

- parse and retain keyframe block bodies
- schedule them from `AnimationSystem`
- invoke them through the live runner context

### Phase 3: bridge existing action constructors

Existing `Action.*` constructors can be lowered to temporary compatibility code
or removed outright. The preferred end state is to stop authoring actions and
call component methods directly.

### Phase 4: remove action-specific targeting logic

Once keyframes mostly operate on captured refs, animation no longer needs a
special target-selector resolution layer for ordinary property updates.

## Open questions

### Simultaneity vs exact statement order

The intended author model is “all statements at this keyframe fire together,”
but the runtime still needs a precise rule for observable ordering if one
statement reads something another statement just changed.

### Serialization format

If keyframes now own executable blocks, scene serialization should preserve the
original MMS code shape instead of synthesizing `ActionComponent` children.

### Introspection / tooling

The editor may still want a structured view of timeline contents. That should
be derived from parsed keyframe bodies, not from requiring those bodies to be
stored as ECS action components.

## Recommendation

Adopt the rule:

- `Keyframe` is a timed executable block
- mutations inside a keyframe happen by methods on live component refs
- those methods dispatch intents
- `ActionComponent` is not the long-term animation abstraction

That gives animation the same mutation vocabulary as the rest of MMS and avoids
building an ever-growing parallel action DSL for every component property.
