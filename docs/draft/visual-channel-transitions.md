# Visual Channel Transitions for Color, Emissive, and Opacity

## Summary

Extend the existing transform-only transition runtime to support:

- `ColorComponent.rgba`
- `EmissiveComponent.intensity`
- `OpacityComponent.opacity`

The recommended rule is:

- `TransitionComponent` only applies when it is an **immediate child of the
  target property component**
- it is **not** inherited from ancestors
- it is **not** allowed to sit above a set of unrelated visual property
  components

So this is valid:

```mms
Color.rgba(1.0, 1.0, 1.0, 1.0) {
  Transition.duration_beats(0.5).ease_in_out_cubic()
}
```

and this is not the intended model:

```mms
Transition.duration_beats(0.5) {
  Color.rgba(1.0, 1.0, 1.0, 1.0)
  Opacity.opacity(0.4)
}
```

The runtime should treat transition policy as metadata owned by one concrete
mutable component, not as a styling ancestor.

## Why direct-child only

For `Color`, `Emissive`, and `Opacity`, ancestor semantics would get ambiguous
quickly:

- which descendant property instance does the transition own?
- if a container has multiple styled descendants, does one `Transition`
  interpolate all of them together?
- what happens when one subtree introduces a more local override?
- what is the replacement key when one ancestor policy fans out to many runtime
  renderable effects?

Those questions do not help the runtime. They create coupling between style
inheritance and transition ownership.

The cleaner rule is:

- the property component owns the value
- the property component may have one immediate child `TransitionComponent`
- that transition policy applies only to mutations targeting that property
  component

This keeps transition lifetime, replacement, and lookup local and
deterministic.

## Current implementation state

Today:

- `TransformComponent` has real runtime transition support
- `SetEmissiveIntensity` exists, but applies immediately
- `SetColor` exists, but applies immediately
- `SetOpacity` does **not** exist yet as a canonical intent

Relevant implementation points:

- `src/engine/ecs/system/transition_system.rs` only stores active transform
  transitions
- `src/engine/ecs/system/system_world.rs` checks for a child
  `TransitionComponent` on transform updates and upgrades the mutation
- `src/engine/ecs/rx/signal.rs` defines `SetColor` and `SetEmissiveIntensity`
- `SetColor` mutates `ColorComponent` immediately via the intent executor
- `SetEmissiveIntensity` mutates `EmissiveComponent` immediately via the
  mutation executor

So the docs/spec already point toward generic property transitions, but runtime
support is only real for transforms.

## Proposed architecture

### 1. Keep one transition runtime, add visual channel records

Do not make a separate system per property type.

Instead, extend `TransitionSystem` to own additional active-transition
collections:

- active transform transitions
- active color transitions
- active emissive transitions
- active opacity transitions

These can stay as separate typed vectors/maps initially. There is no need to
force everything into one giant enum on day one if that makes the code harder
to land.

Example runtime shapes:

```rust
struct ActiveColorTransition {
    component: ComponentId,
    from: [f32; 4],
    to: [f32; 4],
    start_beat: f64,
    duration_beats: f64,
    easing: TransitionEasing,
    last_sampled_beat: Option<f64>,
}

struct ActiveEmissiveTransition {
    component: ComponentId,
    from: f32,
    to: f32,
    start_beat: f64,
    duration_beats: f64,
    easing: TransitionEasing,
    last_sampled_beat: Option<f64>,
}

struct ActiveOpacityTransition {
    component: ComponentId,
    from: f32,
    to: f32,
    start_beat: f64,
    duration_beats: f64,
    easing: TransitionEasing,
    last_sampled_beat: Option<f64>,
}
```

The existing easing functions and beat-based timing model can be reused.

### 2. Add explicit transition-target registries

Instead of scanning a component’s children every time a mutation arrives, track
supported transition attachments explicitly.

Recommended runtime indexes:

- `color_transition_targets: HashSet<ComponentId>`
- `emissive_transition_targets: HashSet<ComponentId>`
- `opacity_transition_targets: HashSet<ComponentId>`
- optionally `transform_transition_targets: HashSet<ComponentId>` later, for
  consistency with the same pattern

Population rule:

- if a `TransitionComponent` is attached directly under a `ColorComponent`, add
  that parent component id to `color_transition_targets`
- if attached directly under an `EmissiveComponent`, add parent to
  `emissive_transition_targets`
- if attached directly under an `OpacityComponent`, add parent to
  `opacity_transition_targets`
- otherwise do not register it as a supported visual transition target

Removal rule:

- on detach/remove of the `TransitionComponent` or its parent property
  component, remove the parent property id from the relevant set

This matches the user-facing rule exactly: transitions are only meaningful when
parented to one supported property component.

### 3. Make `SetOpacity` canonical

`OpacityComponent` should have a first-class intent the same way color and
emissive already do.

Recommended addition:

```rust
IntentValue::SetOpacity {
    component_ids: Vec<ComponentId>,
    opacity: f32,
}
```

Reason:

- it gives opacity the same mutation vocabulary as the other visual channels
- it creates one clear interception point for transitions
- it avoids ad hoc direct mutation from MMS/component methods

`multiple_layers` should stay discrete metadata on `OpacityComponent`; the
transitioned channel is only the scalar `opacity` field.

### 4. Intercept at canonical mutation paths

Each supported channel should have one canonical point where an incoming
mutation can either:

- apply immediately, or
- become a runtime transition

Recommended interception points:

- `SetColor` handling
- `SetEmissiveIntensity` handling
- new `SetOpacity` handling

That interception should happen before the immediate component mutation is
applied.

If the target component does not have registered transition policy, behavior
stays exactly as it is today.

## Transition eligibility rules

For an incoming mutation on a supported visual property component:

1. Resolve the concrete property component ids the intent targets.
2. For each concrete property component id:
   - check whether it is in the channel’s transition-target registry
   - read the immediate child `TransitionComponent` policy if present
3. If no valid policy exists, apply immediately.
4. If a valid policy exists and duration is non-zero, start/replace a runtime
   transition.
5. On each frame sample, apply the interpolated value through the existing
   normal registration path.

Important: the registry is an optimization and a validity filter. The source of
truth for duration/easing/etc. is still the actual `TransitionComponent` on the
property component.

## How each channel should behave

### Color

Input mutation:

- `SetColor { component_ids, rgba }`

Runtime source value:

- the current `ColorComponent.rgba`

Runtime destination value:

- target `rgba`

Sampling:

- lerp each of the four channels independently

Per-frame application:

- update `ColorComponent.rgba`
- emit or directly route the usual `RegisterColor`

Important note: if the `ColorComponent` is a style node above multiple
renderables, that is still fine. The transition is owned by the one
`ColorComponent`; its normal `register_color` behavior already fans the result
out to the relevant renderables.

### Emissive intensity

Input mutation:

- `SetEmissiveIntensity { component_ids, intensity }`

Runtime source value:

- current `EmissiveComponent.intensity`

Runtime destination value:

- target intensity, clamped to `>= 0.0`

Sampling:

- scalar lerp

Per-frame application:

- update `EmissiveComponent.intensity`
- emit or directly route the usual `RegisterEmissive`

As with color, if the emissive component is being used as a style node that
ultimately affects descendant renderables, the transition still belongs to that
one emissive component, not to each renderable.

### Opacity

Input mutation:

- `SetOpacity { component_ids, opacity }`

Runtime source value:

- current `OpacityComponent.opacity`

Runtime destination value:

- target opacity clamped to `0.0..=1.0`

Sampling:

- scalar lerp

Per-frame application:

- update `OpacityComponent.opacity`
- emit or directly route the usual `RegisterOpacity`

`OpacityComponent.multiple_layers` is not interpolated. It stays whatever the
component already says unless an explicit non-transitioned mutation changes it.

## Replacement behavior

Use the same replacement rule already present on `TransitionComponent`:

- `ReplaceSameTarget`
- `AllowParallel`

For visual channels, the practical v1 rule should be:

- replacement key = `(component_id, channel_kind)`

Examples:

- a second opacity transition targeting the same `OpacityComponent` replaces the
  first
- a color transition on one `ColorComponent` does not affect a transition on a
  different `ColorComponent`
- a color transition and an emissive transition on different components can run
  simultaneously

`AllowParallel` does not add much value for a single scalar or rgba property in
v1. If not fully implemented, it can conservatively fall back to replacement,
as already noted in the existing transition checklist.

## Attachment and lifecycle handling

The runtime needs a reliable way to keep the transition-target registries in
sync.

Recommended rule:

- whenever topology changes attach/detach a `TransitionComponent`, inspect its
  direct parent
- if the parent is `ColorComponent`, `EmissiveComponent`, or `OpacityComponent`,
  update the corresponding registry
- if the parent is anything else, ignore it for visual-channel transition
  support

Also:

- when deleting a property component, cancel any active transitions targeting
  that component
- when deleting a `TransitionComponent`, remove the property component from the
  registry and cancel any active transitions for that property if desired

Cancel-on-remove is simpler and avoids sampling into dead components.

## Relationship to style inheritance

This proposal deliberately keeps transition ownership separate from renderable
inheritance.

Example:

- `TextComponent`
  - `ColorComponent`
    - `TransitionComponent`
  - many descendant glyph renderables

The transition is attached to the `ColorComponent`, not to the glyphs. Each
sample updates the `ColorComponent`, and the existing `register_color` logic
propagates that color to the appropriate renderables.

That gives us:

- one transition owner
- one current value
- ordinary existing fan-out behavior

instead of inventing per-renderable transition ownership for inherited styles.

## Suggested implementation order

1. Add `SetOpacity`.
2. Extend `TransitionSystem` with active color/emissive/opacity records.
3. Add small registries for supported direct-child transition targets.
4. Wire attach/detach/remove lifecycle updates for those registries.
5. Intercept `SetColor`, `SetEmissiveIntensity`, and `SetOpacity`.
6. Reuse existing `RegisterColor` / `RegisterEmissive` / `RegisterOpacity`
   application paths for sampled values.
7. Add tests for:
   - direct-child transition works
   - no transition child means immediate behavior
   - ancestor `TransitionComponent` does not apply
   - inherited style node still transitions correctly through one owner

## Recommendation

Support visual-channel transitions by treating `TransitionComponent` as
**direct-child metadata on one property component** and by extending the
existing runtime with explicit per-channel active records and registries.

Do not make `TransitionComponent` an inheritable ancestor styling primitive.
That would blur ownership and make replacement/lifecycle rules much harder than
they need to be.
