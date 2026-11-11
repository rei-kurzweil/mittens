# STA rocket launcher

Status: in progress. Scope now includes the native acceleration and projectile
collision support needed to eliminate per-frame MMS flight callbacks.

## Design and progress

- Reviewed the laser: the input adapter invokes a firing device, which owns cadence
  and muzzle effects; independent world-root laser slots own raycasts and report
  impacts to a callback. Sakura's impact callback calls `Damage.at` with radius
  0.28 and routes the hit to the renderable's optional `on_hit` handler.
- The rocket should be a separate `assets/components/projectiles/` component
  driven by `Velocity`. A native acceleration component drives Velocity, which
  drives the projectile transform. Author acceleration in local space along
  the flight Z axis (currently negative Z), with a target speed. Native code
  stops accelerating at that speed while Velocity continues moving the rocket.
  The local-space Acceleration slice is implemented using ancestor providers
  bounded by nested Velocity components; see
  [the velocity-driver task](gravity-and-acceleration-velocity-drivers.md).
  Give the projectile a bounded lifetime and clean up its native motion,
  collision resources, and event bindings on impact or expiry. The launcher
  owns trigger policy and the 100 ms launch delay; copy the world pose at launch
  so the rocket remains independent of the held weapon.
- Inspected both new GLBs on 2026-11-11: neither contains an `animations`
  array or animation clips. The launcher has nodes `rocket`, `back_half`, and
  `STA_launcher`; the projectile contains a `rocket` node. The requested launch
  clip needs an asset containing it or an explicitly chosen MMS-driven effect.
- The projectile's authored node is centered at approximately
  `[-0.19136, 1.50759, 0.11189]`, with its long axis along Z after its
  node rotation and a length of about 0.346 m. Its origin is therefore not
  at the rocket: compensate for that offset when placing it at a muzzle.
  The launcher embeds a separate rocket node; that geometry must be hidden or
  otherwise handled during launch to avoid drawing two rockets.
- `Damage.at` is consumed by the destroyable system for overlapping
  `Destroyable` surfaces; it does not dispatch target `on_hit` or `on_blast`
  handlers. Sakura explicitly calls those handlers for its cans. Their current
  `on_blast` uses a fixed 0.65 m radius, so a larger rocket blast needs an
  explicit radius-aware target contract as well as `Damage.at`.
- Verify the existing collision detection's behavior for a fast velocity-driven
  projectile, including whether it sweeps between positions and how it reports
  impact attribution. Do not assume an event-driven raycast can report a miss:
  its configured maximum distance has no runtime setter and a miss produces no
  `RayIntersected` event.

## Remaining work

- [ ] Establish model orientation, scale, barrel position, and launch effect.
- [ ] Implement launcher firing and 100 ms delay; integrate held input in Sakura.
- [x] Implement native local-space acceleration driving Velocity, including a
      target-speed limit, and replace the smoke-test MMS acceleration handler.
- [x] Implement native swept projectile collision against existing wall zones.
      Register an impact handler on the rocket's zone; invoke MMS only on an
      actual impact. Validate high-speed collision, bounded lifecycle, and
      impact attribution.
- [ ] Implement two-tone sinuous fading ribbon material and shader; validate
      transparency, geometry, and cleanup.
- [x] Connect the expanding sphere effect and area damage to native impacts;
      verify a destructible wall is cut.
- [ ] Extend blast callbacks to other targets if needed (cans need separate
      radius-aware blast routing).
- [ ] Validate launch timing, independent motion, collision at high speed,
      expiry, multiple concurrent rockets, and visual appearance in the example.

## Native flight and impact API

```mms
let hit_zone = Zone.sphere(0.055) { ProjectileImpact {} }
let motion = Velocity { T { hit_zone /* projectile model */ } }
Acceleration.local([0.0, 0.0, -12.0]).max_speed(18.0) { motion }
on(hit_zone, "ProjectileHit", fn(event) {
    // event.zone, event.other_zone, event.point, event.normal
})
```

Acceleration is in the Velocity parent frame, in m/s². `max_speed` caps the
velocity projection along its acceleration direction (m/s), preserving lateral
velocity. Providers add within the ancestor segment and stop at nested Velocity
boundaries. This slice provides authored local acceleration; world-space and live
setter APIs remain future work.

`ProjectileImpact` opts a sphere zone directly under the driven transform into
native continuous collision against enabled static wall boxes (including yaw)
and destroyed triangle meshes. A retained broad phase filters candidates. The
engine stops and disables Velocity at the earliest contact, emits one
`ProjectileHit` scoped to that zone, and leaves the MMS handler to remove the
projectile, create the explosion, and apply damage. Other projectile shapes and
static primitive shapes are outside this first slice. Flight has no MMS
FrameTick subscription. Expiry remains an animation at the default clock tempo.

## Work log

- Implemented native local acceleration, sphere projectile sweeps, and the
  zone-scoped `ProjectileHit` event. The rocket now applies `Damage.at` with a
  provisional 1.5 m radius and amount 8, plays the expanding sphere, and removes
  itself on impact. It still expires after six seconds at 120 BPM. Exhaust and
  visual/muzzle/damage calibration remain pending. Earlier work-log entries
  below describe the superseded smoke-test stages.
- Validation: four focused rocket tests pass (native speed cap and provider
  boundaries, nearest high-speed thin-wall hit/miss/initial overlap, rotated
  flight and expiry, impact/explosion/completed wall cut). All 13 existing
  Velocity tests and the Sakura scene test pass. Visual/headset acceptance
  and calibration remain for the next slice.

- Updated flight design: native code owns acceleration, movement, and collision
  detection. The rocket must not poll a sweep query from MMS or subscribe to
  FrameTick for flight. A zone impact event calls the authored MMS handler only
  when a collision occurs; collision-free travel causes no MMS callbacks.
  Detect the earliest contact along each native movement step, stop at contact,
  and deliver a single impact with the other zone/collidable, contact point,
  normal, and projectile position. Keep projectile/tool/user attribution for
  the explosion and damage callback. Exact component and event names remain
  to be designed; this is a required native feature, not an existing MMS API.
  This supersedes the earlier proposal for an MMS `sweep_world` polling loop.

- Collision API check: Sakura walls already have `Zone.cube` with
  `Collidable.static()`. The existing response modes are only static and slide;
  there is no projectile stop/impact response. `zone_query` provides synchronous
  overlap/contact queries in Rust, but these are not exposed as MMS component
  methods. `Zone.enable_events()` observes slide movers, not arbitrary static
  zones, and its crossing sweep is specialized for downward capsule/floor
  crossings. It cannot substitute for a general rocket sweep against walls.
  Adding a rocket zone alone therefore does not provide reliable impact events.
  Next collision work should add a native projectile impact
  response using the existing wall zones, including earliest contact and hit
  position/normal, rather than authoring slide behavior for the rocket.

- Flight smoke-test slice: the firing device now spawns `projectiles/rocket.mms`
  after its delay. Native Velocity starts at 2 m/s; the controller adds 12 m/s²
  up to 18 m/s and removes its frame binding at cruise. A six-second animation
  removes the flight root and acceleration binding. Values assume the default
  120 BPM animation clock. Collision/impact, exhaust, and muzzle calibration
  remain pending. The Sakura pedestal scene integration test passes; headset
  firing and flight have not been verified in this environment.
- Smoke test: load `examples/sakura-drone-conversion.mms`, grab the STA launcher
  on the pistol table, and press the held hand's trigger. Each press should
  launch one visible rocket after 100 ms; moving the launcher should leave the
  rocket flying independently. For this flight slice rockets pass through
  scenery and expire after six seconds.

- Added `projectiles/rocket_explosion.mms`, an independent expanding sphere with
  a transition and timed cleanup. It is not yet connected to a projectile.
- Attempted validation with the existing `target/debug/mittens-engine`; this
  binary does not recognize `--validate-only` and could not initialize a window
  in the current environment. The effect has not been runtime-validated.
- Added the standalone `toon_rocket_exhaust` material and fragment shader.
  The shader compiles with `glslangValidator -V`; it has not yet been attached
  to ribbon geometry or visually tested. The two grey wavy bands fade toward
  UV y=1. Launch effect still needs a decision or an animation-bearing asset.
- Rechecked the exhaust shader with `glslangValidator -V` successfully. The
  collision, target-damage, and model-offset findings above still need runtime
  validation and implementation; launcher firing is not wired into Sakura yet.
- Replaced one pistol in the XR Sakura example with a grabbable launcher prop.
  Placement and grip still need visual verification; firing is not wired up.
- Added a trigger-driven firing device and connected Sakura's held input. Its
  animation emits a launch event after 100 ms and supplies the muzzle pose and
  captured user to an optional callback. A trigger press commits the delayed
  event even if released or dropped; no flash or projectile is connected yet.
  The Sakura scene test loads the updated scene but still expects three pistols
  and fails after finding two. Timing and input need runtime verification.
