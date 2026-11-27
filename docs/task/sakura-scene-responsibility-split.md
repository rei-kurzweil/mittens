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

Keep vehicle reset controls and weapon restocking separate from weapon rack
placement. Reuse the fleet's rider and cargo cleanup for manual resets and
preserve stable weapon handles when returning to their original stands. Use
one-shot animation timers and existing shared item channels for restocking.
Validate a reset with riders or cargo, a return while held or mounted, and
matching ammo and poses on both peers.

Keep shield pickup placement and peer ownership with arena equipment rules.
The reusable shield owns integrity, burst visuals and recovery. Use existing
ray indices and event or animation callbacks, with no script frame checks for
shield visibility or recharge. Validate equipped and dropped shields, bullet
pass-through during cooldown, rocket splash and the avatar picker reset.

Keep arena sound clips and their playback callbacks together. Classify each
damage update by whether it affects health, shield integrity, or both. Carry
the chosen cue in the existing health snapshot and verify retransmits do not
replay it.

Bake the shield artwork offline and keep death burst scheduling in a separate
animation component. Verify the texture wraps at the capsule seam, death
effects retire after their last burst, and both peers hear the same damage cue.
