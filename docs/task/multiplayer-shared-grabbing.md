# Shared multiplayer grabbing

Status: planned. Follow up after real desktop/XR two-peer testing.

## Current gap

`GrabbableSystem` handles grabs locally. The multiplayer example sends avatar
root/head/hand poses and blink weights, plus the host-owned ball pose; it does
not replicate grab ownership, attachment, release, or arbitrary grabbed objects.
The ball is not currently authored as grabbable. A host-published object's pose
can be visible remotely, but that alone does not implement shared grabbing;
client-local edits can be overwritten by its receive binding.

Desktop grabbing currently holds an object in front of the camera rather than
driving emulated avatar hands. Support that existing path in the first slice;
desktop hand-pose emulation is a separate follow-up, not a prerequisite.

## Responsibility and proposed first slice

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
