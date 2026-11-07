# Native velocity sleeping and wake

Status: investigation and implementation slices; sleeping is not implemented.
The pickup control-power change keeps native physics enabled. It deliberately
does not simulate sleep by disabling Gravity or Velocity.

## What currently determines rest

- `VelocitySystem::step` (`src/engine/ecs/system/velocity_system.rs`) runs at
  120 Hz. Each step scans components for enabled Velocity components, clears
  `grounded`, applies gravity, integrates nonzero velocity and emits transform
  updates. There is no awake/sleeping state or retained active-body registry.
- `StaticContactSystem::resolve_contact_velocity`
  (`src/engine/ecs/system/static_contact_system.rs`) sets `grounded` for an
  upward support normal, removes inward linear velocity, and applies surface
  friction. `grounded` means supported in the latest contact pass; a truck can
  still slide or rotate while grounded. It is not a settled/sleeping test.
- `SystemWorld` runs velocity, transform propagation and static contacts between
  fixed substeps. Contacts also run outside those substeps. With gravity enabled,
  a supported body still receives small downward motion/contact correction.
- `CollidableBvh::movers` (`src/engine/ecs/system/collidable_bvh.rs`) contains
  enabled `Collidable.slide` components. Membership is based on collision mode,
  not actual motion. A resting truck remains in that contact work list.
- Collider surfaces are a retained BVH with dependency-driven refresh/refit.
  The truck uses separate static obstacle proxies and sliding movement zones.
  "Static" here describes the query/correction role; it does not establish a
  dynamic rigid body's mass, inertia or sleeping state.
- Destruction changes/removes geometry and animates debris. It does not currently
  apply a blast impulse/torque to a truck. Velocity accepts externally supplied
  linear/angular motion, but this is not a mass/inertia rigid-body solver.
  Keeping physics enabled removes the script freeze; it does not add blast
  forces, wheel contact suspension or realistic rollover.

The removed MMS parking policy polled `grounded()` every 0.5 seconds and then
zeroed velocity and disabled gravity. That was neither native sleep nor a
low-speed test. Power buttons replace that policy without polling for support.

## Required separation

| State/registry | Role |
| --- | --- |
| Control power | Gates MMS driving updates/input and NPC drive commands. Independent of physical motion. |
| Native awake body set | Bodies requiring integration/contact work this substep. |
| Native sleeping body set | Bodies at rest with validated stable support; logically enabled, not omitted from collision/query registration. |
| Collision/raycast/damage indices | Retain sleeping bodies so they can be hit, selected, overlapped and woken. |

Do not switch a collider from Slide to Static to fake sleep: the existing modes
have different responsibilities. Do not remove a sleeping truck's obstacle
proxies or damage targets. Avoid a script timer or per-frame MMS rest query.

## Incremental implementation

1. Instrument awake/resting candidates, Velocity scans, integrations, transform
   writes, contact zones, support changes and wake reasons. Measure with both
   orchard trucks off, approached, driven and left behind. The power change
   intentionally increases legitimate physics work versus the frozen baseline.
2. Retain a native Velocity body registry instead of scanning the entire scene
   every 120 Hz step. Invalidate on component lifecycle, enable changes,
   reparenting and driven-transform changes. Preserve nested gravity scopes.
3. Track support identity/contact data and low linear/angular speed over stable
   fixed substeps. Specify thresholds and use a stable-contact duration to prevent
   jittering between states; grounded alone is insufficient. Add eligible bodies
   to a sleeping set and skip integration/transform churn/contact resolution only
   while that support remains valid.
4. Wake before integration on linear/angular commands, relevant pose edits,
   gravity changes, support movement/removal/destruction, and collider edits.
   A coarse spatial generation can be a conservative first invalidator but must
   be measured: unrelated movement must not repeatedly wake the whole scene.
5. Query sleeping collision targets when active bodies/impacts reach them. Wake
   affected bodies before resolving contact. Add explosion impulse/torque and
   dynamic body response as explicit follow-up capabilities, rather than claiming
   existing static-contact correction transfers momentum between trucks.
6. Retain and test active/sleeping contact work lists independently of the query
   BVH. Keep disabled components distinct from sleeping components. Unloading
   removes registry entries and pending support references safely.

## Acceptance tests

- An off truck spawned in the air falls, contacts the floor and eventually
  sleeps natively; gravity/Velocity remain logically enabled.
- A stable resting truck stops generating integration/transform/contact work,
  while raycasts and damage queries can still hit it.
- Removing or moving its supporting deck wakes it and it falls without player
  proximity, a power toggle or a script timer.
- A Velocity command wakes it; returning to zero controls does not erase an
  externally applied impulse or disguise velocity as disabled physics.
- Relevant gravity/parent/collider changes wake or invalidate support correctly.
- When physical impact/explosion response is implemented, an off sleeping truck
  can receive linear/angular impulse and wake; missing capabilities are reported
  rather than replaced by a scripted topple.
- Repeated wake/sleep/unload cycles do not grow registries, lose hit targets or
  leave bodies stuck asleep after support changes.

Related: [pickup control power](pickup-script-power-and-continuous-physics.md),
[vehicle/model schema](../how_to/pickup-model-schema.md).

## Measured active-physics cost and a separate mesh-cache bug

Baseline revision `5f477986` plus the pickup power implementation, 2026-11-06.
CPU-only `profile-ship-cpu`, bridge viewpoint, synthetic 75 Hz, 240 warmup and
480 samples. No GPU rendering or compositor/ALVR is measured. All captures
have no active MMS driving FrameTick handler. The `parked_vehicles` probe is an
explicit diagnostic that disables truck gravity and zeroes motion; that is not
production power-button behavior.

| Capture | Mean update | Mean preparation | Mean CPU total | Mesh uploads in 480 samples |
| --- | ---: | ---: | ---: | ---: |
| Power off, physics active, before CombineMesh fix | 9.489 ms | 1.034 ms | 10.523 ms | 960 |
| Frozen diagnostic, before fix | 3.550 ms | 0.483 ms | 4.032 ms | 0 |
| Power off, physics active, after fix | 8.562 ms | 0.618 ms | 9.181 ms | 0 |
| Frozen diagnostic, after fix | 3.725 ms | 0.603 ms | 4.328 ms | 0 |

[Upload tracing](epic/vulkan-rendering-optimization/data/pickup-power-upload-trace-2026-11-06.log)
identified both orchard dashboard gauge batches. `CombineMesh.keep_transforms()`
hashed source world transforms, so moving the truck incorrectly invalidated a
mesh baked in the batch's local frame. The fix hashes the structural local
transform path below the batch; whole-parent movement updates its instance model
without rebaking geometry. Internal transform edits still rebuild. Transform
operators, XR and layout-managed boundaries retain conservative world-space
invalidation. Nine CombineMesh tests pass, including common-parent translation,
rotation/scale, internal edits and operator boundaries.

The [truck-state capture](epic/vulkan-rendering-optimization/data/pickup-power-truck-states-2026-11-06.json)
measures 8.778 ms update / 0.631 ms preparation / 9.410 ms total with zero mesh
uploads. It records state before/after timing, without adding a FrameTick script
or diagnostic work to sampled updates. All trucks remain enabled with gravity
and report grounded. The hot-dog truck is stationary. The orchard trucks have
resolved linear speeds near floating-point zero, but their poses drift over the
480 samples: one moves approximately `[23.081,5.629,10.538]` to
`[23.262,5.536,10.620]`; the other approximately `[23.089,5.563,19.450]` to
`[23.273,5.459,19.351]`. This is evidence that support/near-zero resolved velocity
alone does not prevent integration/contact pose churn. The fixed-step gravity
and correction sequence is a candidate explanation; native sleeping and stable
support handling are not implemented by the CombineMesh fix.

The remaining active-physics cost is real and substantial. These CPU-only totals
are not frame times or FPS and are not directly comparable with the earlier
1920-per-eye GPU captures, which used the former parked-physics behavior.

Summaries and corresponding `.log` files are preserved under
`epic/vulkan-rendering-optimization/data/` with prefixes
`pickup-power-before-combine-{full,parked_vehicles}-2026-11-06`,
`pickup-power-after-combine-{full,parked_vehicles}-2026-11-06`, and
`pickup-power-truck-states-2026-11-06`.
