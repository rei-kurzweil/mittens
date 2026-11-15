# Shared multiplayer grabbing

Status: MMS prototype implemented; physical desktop/XR and Tailscale acceptance pending.
Host-approved ownership, weapon replication, and throw physics remain planned.

## Current prototype

`PointerGrabbed` / `PointerReleased` are pointer-scoped notifications after local
Grabbable acquisition/release. They expose `pointer` and `target`; rejected grabs
do not announce success. The receiving machine does not invoke GrabbableSystem
for a remote hold. The holder uses its ordinary local grab, while the receiver
applies the resulting world pose. These roles are independent of host/client.

`assets/components/network/shared_grabbable.mms` owns replication policy. The
multiplayer example passes its existing socket, stable channel/object name, local
Transform, and peer rank. The cyan cube and black spring gohei have no root-body
physics; gohei paper secondary motion remains local. A typed snapshot at 20 Hz
contains a transition revision, owner, held flag, translation, and quaternion.
The owner repeats the complete latest state after release as well as while held,
so losing a transition packet does not permanently lose that transition.

Newest transition wins; equal revisions favor rank 2 (client). A losing local
hold is reparented to its original identity scene mount through MMS; the existing
native invalidation path then ends following and emits PointerReleased. This
cleanup notification does not reclaim network ownership. Old revisions are
ignored. The incoming channel is pinned to the session peer's address and accepts
only that peer's rank, but there is no host approval/security authority in this
prototype. Revision/owner are integral values carried by existing f32 type tags;
revision precision is limited to 24 bits. This is a two-peer demonstration, not a
final multiplayer ownership protocol.

On peer departure, the remaining peer retains the last pose and takes ownership
of a remotely owned prop. Connection generation guards ignore obsolete callbacks.
The ball is still separately host-authoritative and is not grabbable. Weapons,
throw velocity, and damage are not connected to this prototype.

Desktop grabbing currently holds an object in front of the camera rather than
driving emulated avatar hands. Support that existing path in the first slice;
desktop hand-pose emulation is a separate follow-up, not a prerequisite.

## Follow-up: host-approved interaction policy

- Keep local pointer/grip placement in `GrabbableSystem`. Add multiplayer
  interaction policy, initially in MMS using the session and typed-channel APIs.
  If shared Rust coordination is needed, consider a focused
  `MultiplayerInteractionSystem`; this is a proposal, not an existing system.
  Generic UDP transport must not acquire grab or physics policy.
- Author stable network object keys and resolve them to local component IDs on
  each machine. Never transmit local slotmap/component IDs.
- A peer requests a grab; the host grants or rejects ownership. Handle simultaneous
  grabs deterministically. Include object identity, peer, interaction generation,
  and hand/pointer identity. Reject stale or unauthorized updates.
- First use host-authoritative objects: the granted holder supplies a desired
  world pose, the host validates/applies it and publishes the accepted pose.
  Do not assume a tracked hand exists: desktop camera-held targets must work.
  Avoid competing writes from local grab following, receive bindings, and physics.
- Replicate accepted world poses through typed channels. Carry grab/release and
  ownership transitions with acknowledged, retried, idempotent control messages;
  the current best-effort app-message path alone is insufficient.
- On release, restore host physics with an explicit policy for final pose and
  throw velocity. Resolve disconnects/timeouts, removed objects, and late joins
  without leaving objects permanently held or physics disabled.
- Defer prediction, smoothing, hand-relative remote attachments, and ownership
  transfer for physics simulation until the basic authority path works. Keep
  gameplay policy and receive binding choices accessible from MMS.

## Acceptance

- Add one explicitly shared grabbable prop to `multiplayer-basic.mms`.
- Test desktop-to-desktop and XR-to-desktop grabbing in both directions: the
  other peer sees hold, movement, release, and resumed host physics.
- Test simultaneous requests, duplicate/lost/reordered control messages, stale
  pose packets after release/re-grab, disconnect while held, and late join.
- No per-object worker threads, unbounded queues, blocking network work on the
  XR update path, or per-frame scene queries for already resolved bindings.

## Weapons and firing channels

Include the machine pistol and STA launcher as motivating shared objects. Grab
ownership and object motion alone do not replicate trigger actions or shots.

- Both `laser_machine_pistol(options)` and `sta_rocket_launcher(options)` already
  accept an options table. Extend that contract with an optional `socket` and a
  stable `weapon_id`; pass these through scene/assembly factories (including
  `ship_machine_pistol`, which currently takes only a label). Omitting the socket
  must preserve offline behavior. This API extension is planned, not wired up.
- Register typed firing channels on the supplied socket in the multiplayer
  example. Define an explicit direction/authority contract: holder requests a
  shot, host validates ownership/ammo/cadence and publishes accepted shots.
  Use type tags imported from `types`, not JSON payloads or a hidden global socket.
- Identify shots with peer/session identity and sequence numbers; include weapon
  identity, muzzle world pose, and any deterministic spread seed or timing fields
  needed for playback. Resolve local tool/user components locally, never send
  `source_tool` or `source_user` component IDs across the wire.
- Publish actual accepted shots, not only trigger-down/up. A held auto trigger
  produces multiple shots; the STA launches after its 100 ms sequence delay.
  Deduplicate retries and prevent replayed shots from publishing again.
- Remote playback owns visual/audio effects; the host owns damage and authoritative
  projectile outcomes. Do not invoke the current damage-producing fire path on
  both peers. Define recovery for dropped firing messages and lost trigger release.
- Validate pistol semi/burst/auto and delayed STA launches, including firing while
  moving/held, ownership changes during the delay, disconnect, duplicate packets,
  ammo agreement, and exactly-once damage application.

Illustrative constructor shape (future API):

```mms
let launcher = sta_rocket_launcher({
    name = "shared_sta_launcher"
    weapon_id = "arena.launcher.1"
    socket = socket
})
```

Desktop camera-held weapons must fire and replicate without requiring emulated
hand drivers. Correcting their avatar hand presentation remains separate.

See [multiplayer roadmap](multiplayer-roadmap.md) and
[local hand placement](grabbable-to-hand.md).

## Prototype validation

- Native tests cover failed/duplicate acquisition, pointer scope, one notification
  per actual transition, and release after external reparenting.
- MMS tests cover component payloads and scope, two-peer translation/quaternion
  replication, release, handoff, simultaneous grabs, disconnect generation guards,
  and a receiver that missed the grab/release receiving the final snapshot.
- The full multiplayer scene loads the cube/gohei, establishes 21 channels per
  peer, and retains desktop/XR mode checks. Physical grip feel and Tailscale
  latency/packet-loss testing remain manual acceptance.
