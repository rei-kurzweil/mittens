# Rooftop example module boundaries

`target_practice.mms` is oversized and combines health, targeting, combat,
navigation and vehicle behavior. Split these along those responsibilities as
the existing items below describe. Preserve its single gated frame handler,
avoid per-frame allocation and repeated route or resource creation, and
validate combat, navigation, vehicle and multiplayer behavior before and after.

- [x] Share the target's firing controller across Interceptor and Truck Bed Demo. Keep vehicle boarding and player damage with the target fixture.
- [ ] Consider moving the target's wrist posing and grip calibration beside the shared equipment code. Compare the solved wrist and muzzle in both demos, and keep pose updates limited to active tracking.
- [ ] Consider moving retreat and rearm movement into a shared navigation controller if more scenes adopt them. Compare pickup routing, cover selection, and foot placement in both demos, and plan routes only when a target changes.
- [ ] Consider moving activity scheduling out of `target_practice.mms` if another NPC adopts it. Keep a single gated frame handler per NPC and verify idle, movement, combat, and disposal with a simulated clock.
- [x] Keep Interceptor's equipment and encounter together in `interceptor_range.mms`.
- [ ] Keep body-mount setup with the avatar swap and equipment lifecycle. Preserve mount identity across avatar swaps and use the existing torso transform chain so worn gear follows solved poses without a frame callback.
- [ ] Consider moving the hangar shell and skyline into scene builders so the example can focus on composing the scene and wiring sessions. Keep each skyline batch intact and keep CSG colliders scoped to the surfaces that own them.
- [ ] Consider moving Truck Bed Demo's truck and mech respawn schedules beside their fixtures. Keep the existing vehicle input and native animation paths. Check seat reattachment, destruction and recovery in both desktop and XR modes before a split.
- [ ] Consider moving NPC seating and ejection into a vehicle-rider component if more vehicles use it. Keep the seat pose apply event-driven and validate the lower-body pose after repeated boarding.
- [ ] Consider moving avatar swapping and its load timeout out of `rooftop_player.mms`. Preserve rig and pointer identity across swaps, remove the timeout binding when disposing a rig, and verify local and remote avatar changes.
- [ ] Consider moving `sakura-twilight-simpler.mms`'s per-peer avatar links (pose, blink, hand validity, avatar/height/flight bursts and their join/leave wiring) into a network module shared with Interceptor's single-sample pose, so the scene only composes connect/disconnect calls. Keep one 20 Hz FrameTick sender and native `bind_transform` receivers (no per-sample script on the receive side), and verify join, rejoin without a leave, leave, avatar change and remote height.
- [ ] Consider grouping `target_practice.mms`'s damage entry points (pistol, rocket splash, remote `take_damage`) and replication hooks (`on_shot`, `on_lie_down`, `on_respawn`) with `practice_health` in their own module if another NPC adopts `npc_replication.mms`. Keep the single gated frame handler and verify host and puppet health, shield and lying state stay in step.
- [ ] Split `sakura_combat_tests.rs` by encounter only when needed. Keep shield wear, flash, and bullet proxy assertions together and reuse the same fixture. Preserve a shared transform for the worn geometry and validate tilt, scaling, expiry, and peer replication without adding per-frame script work.
