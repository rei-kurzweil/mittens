# Velocity / AngularVelocity Components — WIP

Date: 2026-05-07

Sketch for promoting velocity from a private field hidden inside
`KineticResponseComponent` to first-class components attached to a `TransformComponent`.
Goal: a single source of truth for "this thing is moving" / "this thing is rotating",
shared by kinetic response, dynamic chains (leash / spring), and any future system
that needs motion state (collision response, audio doppler, motion blur, jiggle that
follows whole-body motion, etc.).

Not yet a task. This is the design surface to settle before refactoring `src/`.

---

## Today

- `KineticResponseComponent` owns a private `velocity: [f32; 3]`
  (`docs/spec/kinetic-response.md` lines 92-94, runtime-only, not serialized).
- That velocity is integrated, damped, and consumed in
  `KineticResponseSystem::tick_with_queue` and never exposed outside that system.
- Spring bones own their own per-stage angular-velocity state inside
  `TransformStreamSystem` (keyed by stage path).
- IK is fully stateless — no velocity anywhere.
- Nothing has access to a TC's velocity except the system that authored it.

This is fine until two systems need to read or write the same motion state. Three
upcoming reasons that pressure starts to land:

1. **Dynamic chains** (leash + Y-leash + unified spring/IK) need per-joint
   angular velocity for damping and momentum, and would benefit from reading the
   linear velocity of the anchor TC (so jerks propagate down the chain).
2. **Audio / FX**: a system that triggers footstep events or doppler shifts
   needs to read TC linear velocity without depending on `KineticResponseComponent`.
3. **History-aware behavior**: temporal IK, motion-blur authoring, predictive
   collision — all want "what was this transform doing N ticks ago?"

---

## Proposed components

### `VelocityComponent`

```rust
pub struct VelocityComponent {
    /// Current linear velocity in world units / second, world-space.
    pub current: [f32; 3],

    /// Ring buffer of previous values, oldest-first. Length = history_capacity.
    /// Updated once per tick by VelocitySystem (or by whichever system owns
    /// integration for this TC).
    history: VecDeque<[f32; 3]>,

    /// How many past samples to retain. 0 = no history (current only).
    pub history_capacity: usize,

    /// Source of integration. See "Authority" below.
    pub source: VelocitySource,
}

pub enum VelocitySource {
    /// Computed by VelocitySystem from frame-over-frame world-position deltas.
    /// TC is the source of truth for position; velocity is derived.
    DerivedFromTransform,

    /// Owned by an external system (e.g. KineticResponseSystem). VelocitySystem
    /// must not overwrite `current`. The owning system is responsible for
    /// pushing into history each tick.
    OwnedBy(SystemTag),
}
```

Public API:

```rust
impl VelocityComponent {
    pub fn new() -> Self;                              // current=0, history_capacity=0
    pub fn with_history(self, n: usize) -> Self;
    pub fn previous(&self, ticks_ago: usize) -> Option<[f32; 3]>;
    pub fn average(&self, window: usize) -> [f32; 3];  // smoothed velocity
    pub fn speed(&self) -> f32;                        // current magnitude
}
```

### `AngularVelocityComponent`

Same shape, with the value being angular velocity. Two reasonable
representations:

```rust
pub struct AngularVelocityComponent {
    /// Axis-angle scaled: direction = rotation axis (world-space),
    /// magnitude = radians/second. Zero vector = no rotation.
    pub current: [f32; 3],

    history: VecDeque<[f32; 3]>,
    pub history_capacity: usize,
    pub source: VelocitySource,
}
```

Axis-angle scaled is preferred over a quaternion delta because:
- it composes linearly under integration: `q_next = quat_integrate(q, omega, dt)`
- damping is a scalar multiply
- spring forces add cleanly: `omega += alpha * dt`
- zero is unambiguous

Public API parallels `VelocityComponent`.

### `SystemTag`

A small enum naming which system owns the value. Used to make ownership debuggable
and to let `VelocitySystem` skip TCs whose velocity is authoritatively maintained
elsewhere.

```rust
pub enum SystemTag {
    KineticResponse,
    DynamicChain,
    Animation,
    UserScript,
}
```

---

## Authority and update timing

The hard part is "who writes `current` each tick".

### Rule

Exactly one writer per `VelocityComponent` per tick. Writer identity is declared
via `source`.

- `DerivedFromTransform` → `VelocitySystem` writes after `TransformSystem` has
  propagated world matrices, by computing `(world_pos_now - world_pos_prev) / dt`.
  Authoritative for purely-animated TCs (skeletal animation, scripted move).
- `OwnedBy(KineticResponse)` → `KineticResponseSystem` writes during its tick.
  `VelocitySystem` reads but does not overwrite. Pushing into history is the
  owner's responsibility (or `VelocitySystem` does the push pass last).
- `OwnedBy(DynamicChain)` → analogous, for chain-driven TCs.

### History push

Single dedicated pass at the **end of each tick**, after all writers have run:
`VelocitySystem::push_history(world)` snapshots `current` into the ring buffer
for every velocity component with `history_capacity > 0`. This avoids partial
history (some writers pushing, others not).

### Ordering hazards

- `VelocitySystem::derive` must run **after** `TransformSystem` has settled all
  world matrices for the tick (so the position delta is real).
- `KineticResponseSystem` must run **before** `VelocitySystem::push_history` (so
  the latest value is the one history sees).
- Dynamic-chain / IK consumers reading `previous()` see *last tick's* history,
  which is correct — current tick's value isn't pushed yet.

The natural placement in `SystemWorld::tick`:

```
... existing tick ...
TransformPipeline / Transform / SkinnedMesh
KineticResponse              ← writes velocity.current
DynamicChain (future)        ← writes angular_velocity.current
...
VelocitySystem::derive       ← writes current for DerivedFromTransform sources
VelocitySystem::push_history ← snapshot all into history (final pass)
```

---

## Refactor of `KineticResponseComponent`

`KineticResponseComponent` stops owning velocity. It instead requires a sibling
`VelocityComponent` on its TC (authored explicitly, or auto-spawned at register
time if missing).

```rust
TransformComponent {
    VelocityComponent::owned_by(KineticResponse).with_history(8) {}
    CollisionComponent::KINEMATIC() {
        CollisionShapeComponent { ... }
        KineticResponseComponent::push() { ... }
    }
}
```

`KineticResponseSystem::tick_with_queue` reads/writes
`velocity_component.current` instead of `response.velocity`. Friction, gravity
integration, push accumulation all stay in this system — `VelocityComponent`
is the storage, not the integrator.

What moves out of `KineticResponseComponent`:
- `velocity: [f32; 3]` field → `VelocityComponent.current`

What stays:
- `mode`, `enabled`, `max_iterations`, `push_out_epsilon`, `push_strength`,
  `max_speed`, `friction`, `friction_y`, `gravity_coefficient` — all of these
  are *policy* about how to derive forces and resolve overlaps, not state.

---

## Use by dynamic chains

When the unified `DynamicChain` solver lands (see
`docs/task/wip/dynamic-chain-unification.md`), each driven joint TC carries an
`AngularVelocityComponent` with `source = OwnedBy(DynamicChain)`. The solver
reads it for momentum/damping each tick and writes back the post-solve value.

Joints that are also driven by an animation clip (e.g. a partial blend) would
declare `source = OwnedBy(Animation)` and the chain solver would skip writing
those — the chain runs as an FK passthrough on those joints.

---

## History capacity

History is opt-in per component. Three rough sizings:

| Use case | Capacity | Why |
|---|---|---|
| None / default | 0 | Most TCs don't need it; pay nothing |
| Smoothing | 4–8 | `average(window)` for jitter-free speed |
| Predictive collision / motion blur | 16–32 | Need a few frames of past motion |
| Long-tail debugging / replay | 64+ | Out of scope for v1 |

The `VecDeque` is fine up to ~64. Beyond that, a fixed-size ring buffer
(`[T; N]` + head index) avoids allocator churn — defer that until profiling
shows a problem.

---

## Encode / decode

- `current` is **runtime-only** — not serialized (matches today's
  `KineticResponseComponent.velocity` behavior).
- `history` is runtime-only.
- `history_capacity` and `source` are serialized — they're authored config.

Same pattern as existing transform / kinetic components.

---

## Out of scope for v1

- **Local-space velocity**: today's plan stores world-space. If a system needs
  local-space velocity, derive it on read (`world_to_local(world_velocity)`).
  Adding a parallel local-space field is a v2 question.
- **Per-axis velocity sources**: e.g. "Y from gravity, XZ from script". Build
  this only if a real use case demands it.
- **Velocity-based events** (e.g. `EventSignal::HighSpeedImpact`): natural
  follow-up but not part of the component refactor.
- **Frame-rate-independent history sampling**: history is per-tick. If ticks
  are variable-rate, `previous(N)` means "N ticks ago", not "N seconds ago".
  Document this; revisit if it bites.

---

## Open questions

1. **AngularVelocity for skeletal joints**: should every joint TC of a skinned
   mesh carry one? That's a lot of components. Likely answer: no — only joints
   that participate in a `DynamicChain` get one, lazily added by the chain init.
2. **Velocity for non-TC components** (e.g. a panning audio source whose TC is
   shared with other things): probably out of scope. Audio reads its TC's
   velocity, doesn't have its own.
3. **Reset semantics on teleport**: when a TC's world position jumps
   discontinuously, derived velocity will spike. Need a flag or intent
   (`ResetVelocity { tc }`) so the next tick treats the previous position as
   "no motion". This intersects with the spring/dynamic chain reset story in
   `docs/spec/t_pipeline-spring_bones.md` Open Q #4.
