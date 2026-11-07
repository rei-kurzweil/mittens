# Pickup script power with continuous native physics

Status: explicit control power implemented; native sleeping remains follow-up work.
Original investigation recorded after checkpoint `80419d7d`.

## Problem

The user reports that the orchard trucks appear to wake only on approach, with
gravity apparently paused beforehand. After visiting and leaving both trucks,
performance also seems worse than before approaching either truck. The latter
is an observation to reproduce and measure, not yet a confirmed callback leak.

The current proximity optimization mixes script activity with physics parking.
We need an explicit truck on/off switch that gates MMS vehicle control/update
work while native velocity, gravity and collision continue operating normally.
Being distant or switched off must not suspend a falling or moving truck.

## Historical evidence before the power-button change

- `assets/components/vehicles/pickup_activity.mms::park_if_supported()` waits for
  support, then calls `gravity.set_enabled(false)` and `motion.reset()`. This
  intentionally freezes the parked truck's native motion; it is not solely an
  MMS control-loop optimization. `wake()` restores configured gravity.
- The initial state settles before parking; the code does not intentionally
  disable gravity for the first unsupported fall. Investigate the reported
  apparent spawn freeze separately from this confirmed parked-state behavior.
- Returning from active to settling installs `pickup_parking_check`, a looping
  0.5-second Animation. It removes itself only when the support check succeeds.
  Measure whether any truck stays in settling indefinitely after the player exits.
- `ship_input_controller.mms` creates driving input/FrameTick bindings lazily on
  mounting and bypasses them when no driver remains. Mere proximity does not
  currently create those driving bindings. Retained bypassed handles are
  expected; active callbacks/FrameTick generation while idle need checking.
- `pickup_driving.mms` also resets horizontal/angular velocity when publishing
  drive commands or mounting/dismounting. A power-off transition must not reuse
  that reset as a shortcut if it would erase existing physical motion.
- Existing `set_active(false)` is not the desired control-power switch: mounted
  or commanded state wins over it in `refresh()`, and its off path eventually
  parks native physics. Keep manual control power distinct from proximity/mount
  availability and preserve physics independently.

## Desired responsibility split

| State / responsibility | Expected behavior |
| --- | --- |
| MMS control power off | No driving update/polling callback dispatch; input cannot apply powered acceleration/steering |
| Native velocity | Continues integrating existing linear/angular velocity according to native physics |
| Native gravity/contact/collision | Continues independently of proximity and script control power |
| Native physics sleeping | If supported, belongs to physics with contact/impulse/support-change wake rules; not a proximity-driven gravity toggle |
| Mounted attachment | A seated player may switch off without dismounting; mounting does not enable power |
| Proximity sensor | Cheap availability/interaction hint or optional script wake policy; never authority over physics motion |
| NPC driver | Uses the same explicit control-power state and command interface as a player |

The switch changes whether new powered control commands/updates run. It does
not mean braking, resetting Velocity or disabling gravity. Existing momentum is
allowed to change through native friction, damping, contact or an explicitly
separate braking action. Do not automatically re-enable controls merely because
someone stands near a truck that was manually switched off.

## In-cab power button interaction

The explicit on/off button is implemented in `pickup_power_panel.mms`, inside the cab,
with ordinary interaction that does not require mounting or occupying the seat:
a player may reach in and press it from outside. Mounting remains independent
of control power and must not override a manually selected off state.

A truck starts with control power off while native gravity and collision remain
active. It falls and settles on the ground before anyone approaches or enters.
Pressing the button enables/disables the MMS driving logic; it does not toggle
gravity, clear momentum or freeze collision/force response. The previous proximity
parking implementation has been removed, along with its zones and polling timer.

## Slices

- [ ] Reproduce cold distant state -> approach truck A -> leave -> approach truck
  B -> leave, with the same camera/viewpoint and enough settling time.
- [ ] Capture per-truck power/proximity/mounted/commanded state, gravity enabled,
  grounded/contact state, velocity, live parking Animation count and active MMS
  frame/input bindings. Separate native physics work from script dispatch cost.
- [x] Introduce explicit control power with a small persistent state object and
  an in-world on/off control. Document default state, player/NPC command behavior,
  and precedence over proximity. Reuse binding bypass/removal; no new MMS API.
- [x] Remove proximity parking's gravity-disable/velocity-reset coupling. Keep
  physics enabled according to the prefab's authored configuration, independently
  of switch/proximity. Native engine physics optimizations remain separate work.
- [ ] Audit controller cleanup, settling timers, transitions/suspension callbacks,
  streamed-out vehicles and stale mounted drivers. Establish whether the observed
  post-visit slowdown is lingering script work or legitimate active physics.
- [x] Replace existing parking-specific assertions in `tests/pickup_activity.rs`
  with the intended script-power/continuous-physics behavior. Preserve unrelated
  mounting, driving, damage and unloading coverage.
- [ ] Compare cold, approached and post-exit steady-state CPU timings and record
  any intentional native-physics cost increase versus the current freeze prototype.

## Acceptance cases

1. A truck spawned above a floor falls while control power is off and the player
   is distant. Removing its support also permits it to fall without approach.
2. A truck with native linear/angular velocity continues physical motion after
   turning controls off, subject to supported native collision/friction behavior.
3. An off truck's control FrameTick/input callbacks are bypassed natively; they
   do not merely return early after entering MMS each frame.
4. Repeated on/off, approach/leave, mount/unmount and unload cycles do not grow
   active callback/timer counts or leave stale handlers producing CPU work.
5. After leaving both orchard trucks, script activity returns to its appropriate
   idle baseline. Report native-physics time separately; do not regain the old
   benchmark by freezing physical motion again.
6. Current native collision/suspension limitations are reported accurately; this
   task does not silently promise a full rigid-body vehicle simulation.

Related: [ship performance report](ship-interior-performance-profile.md),
[Vulkan optimization epic](epic/vulkan-rendering-optimization/README.md),
[vehicle schema](../how_to/pickup-model-schema.md).

## Implemented behavior and native-physics audit

`pickup_driving.mms` owns a small `powered` state, initially false.
`pickup_power_panel.mms` provides a reachable in-cab toggle and ON/OFF readout;
`truck.set_powered(bool)` and `truck.powered()` share that state for NPCs/tests.
Only a mounted, powered driver enables the shared controller's input/heading
bindings. Powering off and mounting/dismounting clear input latches without
resetting native velocity; NPC driving commands are ignored while off. Powering
on an empty truck does not install unnecessary input/FrameTick work.

The former proximity flag has been removed from prefab callers and the parking
asset deleted. Authored Gravity configuration remains independent of control
power. Tests cover unmounted button use, approach/leave, off-state falling,
support removal, motion after power off, external linear/angular velocity,
mounted power cycles, driver bindings, driving/collision and unload cleanup.

Native rest is not optimized yet: the engine has support detection and contact
friction, but no sleeping Velocity state or awake-body/contact registry. The
[physics audit and implementation slices](native-velocity-sleeping-and-wake.md)
record exact current behavior, required wake paths and the distinction between
retaining collision targets and skipping resting-body work. Existing blast damage
also does not currently apply physical impulse/torque to trucks.

The CPU audit also exposed an avoidable dashboard CombineMesh rebake on whole
truck motion. The engine fix preserves retained geometry for ordinary structural
parent movement and keeps invalidation for internal edits/transform operators.
Sampled mesh uploads fall from 960 to zero, but active physics remains much more
expensive than frozen physics; see the measurements in the native-sleeping audit.
This change intentionally does not preserve historical FPS by disabling gravity.

Validation: 11 integration tests pass across pickup power, sliding collision,
imported destruction, suspension, ship assets, mounting/unloading, streaming and
weapon input/ammunition. Nine CombineMesh unit tests pass. Integration fixtures
now explicitly power controls, use a stable raycast target instead of replaceable
text glyphs, and instantiate the lightweight weapons table for streaming checks
without restoring the disabled grocery inventory. The short destruction fixture
waits for asynchronous shader compilation before test-process teardown.
