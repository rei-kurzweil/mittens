# Player grabbing and tiny avatar equipment

Status: planned. Reported during Sakura Twilight Simpler playtesting on
2026-11-26.

## Avatar collision and pickup reach

A tiny player still gets blocked between a regular or giant avatar's legs and
can be stopped before the other player gets close enough to pick them up.

Commit `93ba4c98` added avatar-to-avatar shape queries for detailed rigs.
Nearby movers query the other avatar's bone-following thigh, shin, and ankle
spheres in an overlay collision context. Successful limb binding marks the
main capsule `mover_only`, so it still drives its owner through the world.
The torso spheres can still block at hip and chest height. This is a mover
querying another rig's obstacle shapes, with the existing static-world
collision path alongside it.

- Reproduce on flat ground with two players in Sakura Twilight Simpler. Test
  a normal-size and a giant target, and swap which peer is tiny. Compare
  with a local fixture using the same two rigs and stance.
- Capture the blocking zone and contact normal. Check limb binding on both
  peers, the remote capsule's `mover_only` flag, and world-space bounds for
  the limbs, torso, and tiny mover. Check for stale bounds or fallback after
  a height change or avatar replacement. Verify the available clearance in
  the test stance before expecting a pass between the legs.
- Check hand interaction separately once the tiny player can get close.
  Verify the pointer can acquire the target from a grab pose and that the
  held placement can clear nearby geometry. Distinguish movement blocked by
  a collision shape from acquisition or placement blocked by the grab path.
- Keep the retained overlay queries and the static-world collision behavior.
  Preserve scale-relative tolerances for tiny avatars and avoid a new
  per-frame scan of avatar bones.

## Height and attachment eligibility

- Use the effective avatar height in metres from the Humanoid settings path
  after the height change commits. Below `0.5 m`, expose an incoming grab
  and equipment role at the same attachment index as handheld items. Exactly
  `0.5 m` stays in the regular avatar tier. The rooftop player already uses
  this boundary to switch to scaling the whole rig.
- Preserve the tiny player's ability to grab weapons and props. Incoming
  eligibility for being held or worn and outgoing hand interaction are
  separate capabilities. Use the same applied height and eligibility on both
  peers after avatar or height changes.
- A higher-index player can lift, rotate, translate, and equip a lower-index
  player. Same-size players can grip and pull each other over the network.
  Start with a pull that keeps the target in its ordinary movement and
  collision path. Tune pull strength, movement limits, and contention in a
  two-peer prototype before sharing the full carry behavior.
- Reuse the equipment drop policy. `Grabbable.attachment_index` currently
  gates `Zone.receive_drops`, and player equipment receivers accept through
  index `0`. Verify item defaults before choosing the shared lower index.
  Keep attachment eligibility independent of the peer tie-break rank in
  `shared_grabbable.mms`.

## Carry and wear in v1

- Keep the carried player's pose at the moment the grip is accepted, whether
  they are standing, crouching, or crawling. The holder's hand drives the
  carried root's position and orientation, with the carried player's scale
  preserved. Suspend the carried player's locomotion, jump, and flight for
  that attachment. The carried player can still look around, with their
  own view rotation composed relative to the moving holder or equipment
  frame. Keep reachable item grabs available and reject attachment cycles.
- Let the holder release a tiny player onto the existing compatible
  equipment slots, including the thigh, back, and head placements used by
  wearable gear. Follow the slot after equipping and keep the same pose and
  look rules. Support taking them off or grabbing them again. A player on a
  thigh slot does not count as a rocket for flight permission.
- On release, restore the movement state owned by this attachment and resume
  the pose solver from a valid release pose. End conflicting movement writers
  while held and avoid a collision correction against the holder fighting
  the attachment. Define the release behavior when an avatar resizes out of
  eligibility, swaps rigs, disconnects, or is removed.

## Multiplayer prototype and acceptance

Reuse the existing session and typed-channel patterns from shared grabbing.
Identify participants, hand or slot, and attachment generation with stable
network identities. Prototype an accepted grab, pull, carry, equip, or
release transition with one owner for root movement in each state. While
carried, the holder supplies the hand or mount frame and the carried peer
continues supplying look relative to that frame. Prevent the ordinary remote
root receive binding from overwriting the attached pose. Specify arbitration
for simultaneous grabs and recovery for stale pose samples, reordered
transitions, lost releases, and disconnects before enabling carry by default.

- [ ] A tiny avatar can cross a physically open gap between larger legs,
  still collides with a leg and the floor, and is reachable by the holder's
  hand on either peer.
- [ ] Below, at, and above `0.5 m`, both peers agree on incoming eligibility.
  A tiny player can still grab a weapon or prop.
- [ ] Two same-size players can grip and pull in both directions over the
  network. A higher-index player can pick up and rotate a lower-index player.
- [ ] Both peers see the captured body posture through lifting and equipping.
  The carried player can look relative to the holder or slot while movement
  input leaves their root attached.
- [ ] Equipping and removing a tiny player use an existing compatible slot.
  Release, resize, avatar replacement, and peer departure restore control
  without a snap back to a stale root pose or a stranded attachment.

## Related tasks

- [Collision contexts and vehicle carry](collision-contexts-and-vehicle-carry.md)
- [Shared multiplayer grabbing](multiplayer-shared-grabbing.md)
- [Release a held item onto a body mount point](held-item-body-attachment.md)
- [Attachment valence and Grabbable unification](attachment-valence-and-grabbable-unification.md)
- [Attachment movement authority and input routing](attachment-movement-authority-and-input-routing.md)
