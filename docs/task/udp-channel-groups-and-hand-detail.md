# UDP channel groups and hand detail

Status: proposed. Documentation only; nothing here is implemented.

Related: [typed UDP channels](typed-udp-channels-and-mms-bindings.md),
[channel wire format](../spec/typed-udp-channel-wire.md),
[hand tracking and finger retargeting](hand-tracking-system-and-finger-retargeting.md),
[multiplayer roadmap](multiplayer-roadmap.md),
[body solver collision detail](body-solver-horizontal-mode.md#collision-detail).

## Goal

Let a multiplayer scene choose how much of each hand it sends:

- **Wrist:** today's behavior. One hand-target TRS plus a pose-valid flag.
- **Fingers:** the wrist plus every mapped finger joint (15 per hand on the
  VRoid map).

Finger data should only go out when it carries information. With controllers
held, the fingers barely change; they should be sent when a grip or trigger
changes the hand pose, not every tick.

## How hands are sent today

`examples/twilight_rooftop.mms` (and `multiplayer-basic.mms`) publish one typed
channel per transform: `root`, `head`, `left-hand`, `right-hand`, plus `f32`
channels for blinks and hand validity. A `FrameTick` handler writes each one at
20 Hz with `.write(...)`. The receiver binds each channel to one transform with
`.bind_transform(target, { space = ... })`, and each machine solves its own IK.

Per the [wire spec](../spec/typed-udp-channel-wire.md), a datagram is at most
1,200 bytes, including a 38-byte header, and a TRS sample is 40 payload bytes
(78 bytes on the wire).

Extending that pattern to fingers would mean, per hand, per tick:

- 15 more channels to declare and bind,
- 15 `.write(...)` calls through the interpreted MMS evaluator, inside a
  `FrameTick` handler that already does the existing writes,
- 15 datagrams, each paying the 38-byte header for 16 bytes of rotation.

That works as a prototype, but the per-joint MMS loop is the expensive part,
and it grows with every player and every hand.

## Proposal: a UDP channel group

A **channel group** is a named bundle of channels that travel together, for
example `"player_1_left_hand"` holding the wrist and 15 finger joints. MMS
declares it and binds it to part of an armature once; the engine samples,
packs, sends, receives and writes the joints natively.

```mms
// Sender: one group per hand, bound to the mapped finger joints.
let left_hand = socket.publish_group("player_1_left_hand")
    .to(address)
    .bind_tree(local.model, { slots = "left_hand_and_fingers", fields = "rotation" })
    .send("changed", { rate = 20.0, keepalive = 1.0, threshold_deg = 1.0 })

// Receiver: the same group written onto the remote model's joints.
let remote_left = socket.subscribe_group("player_1_left_hand")
    .from(address)
    .bind_tree(remote.model, { slots = "left_hand_and_fingers", fields = "rotation" })

// Policy stays in MMS: detail is binding or unbinding.
left_hand.enabled(false)   // wrist only: stop sending fingers
left_hand.enabled(true)    // fingers again
```

Names and option spellings are placeholders.

### What the engine does natively

- **Binding:** resolves the bone-map slots once and caches the joint ids; MMS
  never walks the tree per frame. It rebinds when the map generation changes.
- **Sending:** samples the bound fields of all members, and packs them into
  one datagram: one header, a member mask, then the values. 16 joints × 16-byte
  quaternions is 256 bytes, well under the 1,200-byte limit.
- **Change gating:** with `send("changed")`, a sample goes out only when some
  member moved more than the threshold since the last send, capped at `rate`,
  plus a `keepalive` so late joiners and lost packets recover (channels are
  unreliable). Controllers held still produce no finger traffic; pressing a
  grip or trigger moves the fingers past the threshold, and those changes go out.
- **Receiving:** writes all members in one pass under the subscription's
  ownership. On timeout or unbind, members return to rest (or the local
  animation), the way morph and transform bindings already release.

### Why the group, not a bigger record channel

Today's channels already carry record types, so one channel with 16 `vec4`
fields would fit in a single datagram. What that lacks is the tree binding:
MMS would still read and write 16 transforms per tick. The group is the
MMS-facing concept (named members, one binding to a subtree or slot list, one
enable switch). On the wire it can be exactly that one record-shaped sample.

## Where the detail setting lives

Not on `AVC`. AVC owns the local body; it should not know about networking.
Collision detail sits on AVC because it shapes the local body. Network detail
is a sending policy, so it belongs with the channels:

- The **group** is generic: any named set of transforms, not just hands.
- The **player factory in MMS** chooses the detail level and toggles groups,
  as `twilight_rooftop.mms` already owns which channels exist.

A small MMS helper (for example `assets/components/network/hand_channels.mms`)
can package "wrist" vs "fingers" for reuse without new engine concepts.

## When fingers carry information

- **Articulated hand tracking** (`HandTracking` with an unobstructed data
  source): fingers change continuously; send while they move.
- **Controllers held:** fingers come from controller-driven poses (or rest)
  and only change when grip, trigger or touch inputs change. Change gating
  covers this without the sender needing to inspect controller state.
- **No tracking or animation:** nothing moves; only keepalives go out.

If change gating alone proves too chatty (tracker noise near the threshold),
add an explicit gate from MMS: enable the finger group only while
`HandTracking` reports an articulated source, or for a short window after
`XrButtonChanged`/`XrAxisChanged` on grip or trigger.

## Slices

1. **MMS-only prototype.** Per-joint channels for one hand in the twilight
   example, behind a `hand_detail = "fingers"` flag at the top. Measure frame
   cost and packet counts against `"wrist"`. This validates the policy and
   gives the numbers that justify step 2.
2. **Native group, send side.** `publish_group` with slot-list tree binding
   and one packed datagram per sample. Codec tests: round trip, member mask,
   size limit, schema mismatch.
3. **Native group, receive side.** `subscribe_group(...).bind_tree(...)` with
   ownership, timeout and release-to-rest. Two-Universe test with real rigs:
   curled fingers on one side appear on the other.
4. **Change gating and keepalive.** Threshold, rate cap and keepalive; tests
   prove a still hand sends only keepalives and a grip change sends promptly.
5. **Detail switching.** `enabled(...)` from MMS, with the receiver releasing
   cleanly when fingers stop; switch the twilight example over.

## Open questions

1. Quaternion encoding: full `vec4` (16 bytes) first; smallest-three
   quantization (about 6–7 bytes) only if bandwidth matters.
2. Slot lists vs raw subtree: slot lists (`left_hand_and_fingers`) avoid
   sending helper and cloth bones; a raw subtree may suit non-humanoid props.
3. Whether the receiver writes local rotations directly or hands them to
   `HandTrackingSystem` as a remote articulation source, so remote and local
   finger posing share one retargeting path.
4. Interaction with the body solver: when a horizontal avatar plants untracked
   hands, the sender should not override them with stale finger poses.
