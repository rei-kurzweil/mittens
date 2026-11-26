# Rooftop example module boundaries

- [x] Share the target's firing controller across Interceptor and Truck Bed Demo. Keep vehicle boarding and player damage with the target fixture.
- [ ] Consider moving the target's wrist posing and grip calibration beside the shared equipment code. Compare the solved wrist and muzzle in both demos, and keep pose updates limited to active tracking.
- [ ] Consider moving retreat and rearm movement into a shared navigation controller if more scenes adopt them. Compare pickup routing, cover selection, and foot placement in both demos, and plan routes only when a target changes.
- [ ] Consider moving activity scheduling out of `target_practice.mms` if another NPC adopts it. Keep a single gated frame handler per NPC and verify idle, movement, combat, and disposal with a simulated clock.
- [x] Keep Interceptor's equipment and encounter together in `interceptor_range.mms`.
- [ ] Consider moving the hangar shell and skyline into scene builders so the example can focus on composing the scene and wiring sessions. Keep each skyline batch intact and keep CSG colliders scoped to the surfaces that own them.
- [ ] Consider moving Truck Bed Demo's truck and mech respawn schedules beside their fixtures. Keep the existing vehicle input and native animation paths. Check seat reattachment, destruction and recovery in both desktop and XR modes before a split.
- [ ] Consider moving avatar swapping and its load timeout out of `rooftop_player.mms`. Preserve rig and pointer identity across swaps, remove the timeout binding when disposing a rig, and verify local and remote avatar changes.
