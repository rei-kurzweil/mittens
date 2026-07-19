# Collision response

Collision response is an **opt-in** behavior layer for movable colliders.

- Collision detection/queries work without it.
- Collision signals still emit without it.
- Adding collision response controls **automatic movement** in response to overlaps.

This doc describes how `CollisionResponseComponent` is expected to be used, its modes, and how it integrates with transforms/collision/gravity.

## Topology and integration

### Required topology

`CollisionResponseComponent` must be attached as a direct child of a `CollisionComponent` (which itself should be a direct child of a `TransformComponent`).

Example topology:

```rust
TransformComponent {
  CollisionComponent::KINEMATIC() {
    CollisionShapeComponent { ... }
    CollisionResponseComponent::push() { ... }
  }
  RenderableComponent { ... }
}

GravityComponent {
  TransformComponent {
    CollisionComponent::KINEMATIC() {
      CollisionShapeComponent { ... }
      CollisionResponseComponent::push() { ... }
    }
  }
}
```

### Transform updates

Collision response emits `UpdateTransform` for its selected movement target.

- The collider transform remains the source of truth for contact geometry.
- With `movement_target(...)`, the resulting world displacement is added to the
  target's current world pose; the collider/transform-stream output is not mutated.
- An unresolved authored target skips movement and velocity changes for that frame.
- Without an authored or runtime target, the immediate parent transform is moved.
- After the transform changes, `TransformSystem` recomputes cached world matrices and updates dependent systems.

### Gravity

`GravityComponent` can influence collision response:

- Any `CollisionResponseComponent` nested under a `GravityComponent` will have gravity applied.
- `GravityComponent` can live anywhere in the scene graph and affect an entire subtree.
- If multiple gravity fields exist in the ancestor chain, the nearest enabled one wins.

## Modes

`CollisionResponseComponent` supports multiple response policies.

### `slide` (`CollisionResponseComponent::slide()`)

Classic kinematic “push out of statics” behavior.

- Each tick, if overlapping static colliders, pushes out along the shared
  shape-pair minimum translation vector.
- Useful for camera rigs and players sliding along level geometry.

### `push` (`CollisionResponseComponent::push()`)

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
- `movement_target_source: Option<ComponentRef>` — optional authored transform
  destination. Rust: `movement_target(ComponentRef)`; MMS: `movement_target(...)`.

## Runtime state

- `velocity: [f32; 3]` is runtime-only (not serialized).

## Related code

- Component:
  - src/engine/ecs/component/collision_response.rs
- Systems:
  - src/engine/ecs/system/collision_response_system.rs
  - src/engine/ecs/system/collision_system.rs
  - src/engine/ecs/system/transform_system.rs

## Notes / future work

- Additional bounds-to-shape inference heuristics beyond AVC's upright capsule.
- Better contact manifold handling (stability, stairs, slopes).
- A dedicated character-controller component separate from general kinematic response.
