# Sakura scene responsibility split

Status: proposed. The scene has grown to include environment construction,
avatar setup, target placement, and weapon input in one example file.

Extract cohesive scene families into reusable components as future work: keep
roof/building assembly, avatar and input setup, and weapon-table interactions
at separate boundaries. Preserve shared materials and existing batched geometry;
do not introduce per-frame queries or per-object resource creation. Validate
both XR and desktop variants, held input, target interactions, and scene startup;
compare startup and frame performance if the extraction changes batching or
runtime subscriptions. This is not a prerequisite for scoped scene changes.

Sakura Twilight's two pistol racks and their shared impact channel are
candidates for a cohesive scene asset. Keep rack placement separate from the
peer lifecycle, and retain event-driven retries and one impact stream for the
roster. Validate both racks, the rooftop drive lane, the per-peer channel
budget, and final grab and shot delivery after release.

Keep player presentation separate from avatar construction and replacement in
`assets/components/characters/rooftop_player.mms`. Presentation owns the
overhead status display and local HUD, while the player factory owns the
stable handles and reattachment during a swap. Preserve camera calibration and
avoid scripted frame subscriptions for presentation. Validate placement and
health changes in the simpler scene with both desktop and XR tracking, after
avatar replacement, and on both sides of a peer session.

Keep arena rules and player-health replication in their own component. The
scene connects the weapon roster, player hit zones, peer channels, and picker
callback. Reuse native ray and projectile collision, use one typed health
channel per direction, and check firing eligibility at trigger and shot time.
Validate pistol and rocket hits, packet repetition, zero-health weapon gates,
and picker resets while old hit reports are in flight.
