# Kinetic response

Kinetic response is an **opt-in** behavior layer for kinematic colliders.

- Collision detection/queries work without it.
- Collision signals still emit without it.
- Adding kinetic response controls **automatic movement** in response to overlaps (a simple character-controller / push-out / pushable-style behavior).

This doc describes how `KineticResponseComponent` is expected to be used, its modes, and how it integrates with transforms/collision/gravity.

## Topology and integration

### Required topology

`KineticResponseComponent` must be attached as a direct child of a `CollisionComponent` (which itself should be a direct child of a `TransformComponent`).

Example topology:

```rust
TransformComponent {
  CollisionComponent::KINEMATIC() {
    CollisionShapeComponent { ... }
    KineticResponseComponent::push() { ... }
  }
  RenderableComponent { ... }
}

GravityComponent {
  TransformComponent {
    CollisionComponent::KINEMATIC() {
      CollisionShapeComponent { ... }
      KineticResponseComponent::push() { ... }
    }
  }
}
```

### Transform updates

Kinetic response ultimately moves objects by mutating the owning `TransformComponent` (via `UpdateTransform` intents).

- The kinematic collider’s transform is considered the *source of truth* for world position.
- After the transform changes, `TransformSystem` recomputes cached world matrices and updates dependent systems.

### Gravity

`GravityComponent` can influence kinetic response:

- Any `KineticResponseComponent` nested under a `GravityComponent` will have gravity applied.
- `GravityComponent` can live anywhere in the scene graph and affect an entire subtree.
- If multiple gravity fields exist in the ancestor chain, the nearest enabled one wins.

## Modes

`KineticResponseComponent` supports multiple response policies.

### `slide` (`KineticResponseComponent::slide()`)

Classic kinematic “push out of statics” behavior.

- Each tick, if overlapping static colliders, pushes the transform out along the minimum-penetration axis (AABB).
- Useful for camera rigs and players sliding along level geometry.

### `push` (`KineticResponseComponent::push()`)

“Pushable” behavior.

- Accumulates a runtime velocity away from overlapping **non-static** colliders.
- Integrates that velocity each tick.
- Still resolves overlaps against static colliders.
- Includes a simple horizontal bounce on static side-wall contacts (X/Z velocity reflection) so bodies don’t just stick while being corrected.

## Tuning fields

(Encode/decode keys shown.)

- `enabled: bool` — master toggle.
- `mode: "slide" | "push"`
- `max_iterations: u32` — max static push-out iterations per tick.
- `push_out_epsilon: f32` — tiny extra separation to reduce jitter at exact contact.
- `push_strength: f32` — strength of push-mode acceleration from non-static overlaps.
  - Builder: `with_push_strength(f32)`
- `max_speed: f32` — clamp on push-mode speed (world units/sec).
- `friction: f32` — per-second velocity damping applied every tick in push-mode.
  - Off by default (`0.0`).
  - Builder: `with_friction(f32)`
- `friction_y: f32` — per-second damping applied to **Y velocity only**, and only when resolving a **vertical (Y-axis) static overlap** (e.g. floor/roof contact).
  - Off by default (`0.0`).
  - Builder: `with_friction_y(f32)`

## Runtime state

- `velocity: [f32; 3]` is runtime-only (not serialized).

## Related code

- Component:
  - src/engine/ecs/component/kinetic_response.rs
- Systems:
  - src/engine/ecs/system/kinetic_response_system.rs
  - src/engine/ecs/system/collision_system.rs
  - src/engine/ecs/system/transform_system.rs

## Notes / future work

- More robust shape support (beyond AABB-style minimum penetration response).
- Better contact manifold handling (stability, stairs, slopes).
- A dedicated character-controller component separate from general kinematic response.
