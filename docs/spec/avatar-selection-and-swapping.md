# Avatar selection and runtime swapping

Date: 2026-11-15
Status: proposed design; nothing here is implemented. The current baseline is
[twilight_rooftop.mms](../../examples/twilight_rooftop.mms), where each side
derives the host's and client's avatars from the network role.

## Goal

Let a player pick their avatar from a roster panel, on desktop with the mouse
pointer or in XR with either hand's laser, and see the change applied locally and
on every peer. A peer that joins later sees the current choice. Each avatar is
authored as a **factory function** that builds its own body. A generic builder
that reads per-model data tables is explicitly not the design.

Out of scope: user-imported models, avatar file transfer between peers,
per-avatar animation sets, and multi-peer sessions beyond the current one-peer
session.

## Current engine facts this design depends on

- **AVC binds once.** Avatar control initialization stores the model root,
  resolves head/hand/hip bone ids and arm IK chains, builds its capsule and
  transform fork under the model root from the model's render bounds, and
  calibrates the model's height. Nothing clears or re-resolves these. Only the
  camera anchor follows a new humanoid bone-map generation
  (`refresh_map_camera_anchor` in `avatar_control_system.rs`). **Swapping the GLTF
  under a live AVC is therefore unsupported**; a swap replaces the whole AVC.
- **AVC owns its camera and hands by topology.** Direct `Camera3D`/`CameraXR`
  children (optionally in one offset wrapper) are re-parented under the head
  anchor during initialization ([avatar camera](avatar-camera.md)). Direct
  `ControllerXR` (`XRHand`) children are discovered as the hand controllers.
  Removing the AVC subtree removes whatever it currently holds.
- **Handler lifetime.** Removing a subtree removes every handler scoped to a
  component inside it (`system_world.rs`, `remove_all_scoped_handlers_for_scopes`).
  Handlers scoped to `world` (`on(world, …)`, `on_global`) or to components
  outside the subtree survive. `on(...)` returns a binding with `.remove()`;
  removal also discards that binding's queued callbacks
  ([language guide](../../crates/meow-meow-script/docs/guide/language.md#signals)).
  A surviving handler that calls a method on a removed component fails with
  `component does not exist`, every time it runs.
- **MMS has tables, not structs.** Tables (`{ eye_height = 1.58 }`) hold values,
  including function values; type-only records (`{ translation: vec3 }`) describe
  material inputs and UDP channel payloads. Records carrying both types and values
  are only a [draft](../../crates/meow-meow-script/docs/draft/record-types-and-values.md).
  The body contract below is therefore a documented table shape, not a checked type.
- **GLTF resources are cached by URI** (`GLTFSystem::resources_by_uri`). The first
  load of a model reads and parses the file; later instances of the same URI reuse it.
- **Thumbnails cannot be live-rendered from MMS yet.** `Texture.render_image(...)`
  consumes renderer-produced images, but the `SceneCapture` producer kind has no
  MMS path. Static images work today.
- **Session app messages are unreliable.** `net.send` has no ack or resend
  ([multiplayer roadmap](../task/multiplayer-roadmap.md), item 2).

## Design: a persistent shell and a swappable body

Split each player into two parts:

- **Shell** (persistent, created once per player): the locomotion root with
  `Velocity`/`Gravity`, the head target, the two hand targets with their pointers,
  the pose driver, the camera (local player only), and the `XRHand` drivers. Every
  network *transform* channel binds to shell nodes, which is what keeps those
  channels valid across swaps.
- **Body** (replaced on swap): the AVC, the GLTF and everything per-model inside
  them — colliders, secondary motion, morph map, poses, eye animation and accessories.

### Why the factory takes a camera and hands, but not the pose driver

The pose driver writes the shell's head and hand targets: `InputXR` for local XR,
`PoseInput` for local desktop, and `bind_transform` channels for a remote player.
Those targets must outlive every body, so the driver stays in the shell. It is
never handed to the factory, and the body mounts *under* the head target it drives.

The camera and the `XRHand` drivers are different: AVC only finds them as its own
direct children. The shell therefore *lends* them to the factory, the AVC adopts
them during initialization, and the shell takes them back before the body is
disposed.

### Body factory contract

Each avatar is one exported function, for example `mitten_body(shell)` in
`assets/components/characters/mitten/player_body.mms` and `lumae_body(shell)` in
`assets/components/characters/lumae/player_body.mms`. Model-specific calibration
(eye height, sole offsets, colliders, secondary motion, accessories) lives inside
that function as code.

```mms
// Input: the shell (read-only from the factory's point of view)
//   shell.prefix          stable name prefix, e.g. "local" / "remote"
//   shell.root_name       movement target name for AVC.movement_target
//   shell.camera          camera component to adopt, or null (remote players)
//   shell.left_hand       XRHand driver to adopt (the shell keeps a reference)
//   shell.right_hand
//   shell.local           true for the local player
//
// Output: a body table
//   body.avatar           the AVC; mount under shell.head
//   body.model            the GLTF; morph channels bind here
//   body.eye_height       desktop eye-to-sole height for shell.head
//   body.bindings         every binding the factory created outside its own subtree
//   body.disposed         false until dispose() runs
//   body.dispose()        removes body.bindings, then the body subtree
export fn mitten_body(shell) { ... }
```

Rules for factories:

1. Adopt `shell.camera`, `shell.left_hand` and `shell.right_hand` as direct AVC
   children (camera inside its usual eye-offset wrapper). Never create cameras
   or hand drivers.
2. Anything registered on `world` or on a component outside the body, such as a
   `FrameTick` loop or a handler on the session, is pushed onto `body.bindings`.
   Handlers on the body's own components need no bookkeeping.
3. Delayed work, like the `GLTFInitialized` handler that attaches eye gaze or a
   halo, checks `body.disposed` first. The handler itself is removed with the
   subtree, but a callback that is already running can outlive the check.
4. Use `shell.prefix` for every name so two bodies never collide in queries.

### Roster

The roster is the one piece of plain data: a list of `{ id, label, thumbnail,
factory }`, where `factory` is the function value. `id` is the stable string sent
over the network ("mitten", "lumae"); an unknown id falls back to the session
default. This is UI and lookup data, not component recipe data.

## Swap sequence (local player)

The order matters; each step names the failure it prevents.

1. **Lock.** Ignore further selections until this swap finishes, and disable the
   roster buttons. Rapid clicks would otherwise interleave two swaps.
2. **Release held items.** Release every shared grab held by this player's pointers
   and end any active tool use. Held items attach to hand bones and would vanish
   with the old body; shared-grab ownership would stay claimed.
3. **Pause physics.** Disable the shell's `Gravity` and zero its velocity. During
   the swap there is no capsule, and the player would fall through the roof.
4. **Reclaim lent parts.** Re-parent the camera and both hand drivers from the old
   AVC back to the shell. If they are still inside the AVC when it is removed, they
   are destroyed: the local player's view goes black and the pointers disappear.
5. **Dispose.** `body.dispose()` removes `body.bindings`, then the body subtree;
   scoped handlers inside it go with it.
6. **Build.** `let next = factory(shell)`; set `shell.body = next`; mount
   `next.avatar` under `shell.head`; set the desktop head height from
   `next.eye_height`. AVC initialization adopts the lent camera and hands.
7. **Wait for readiness.** After `GLTFInitialized` (per-model wiring) and
   `CapsuleReady`, re-enable `Gravity`. Until then, hold the root still.
8. **Announce.** Send the avatar id to the peer (see Networking), then unlock the
   roster.

### Remote player swap

The same as the local sequence without the camera, input or physics steps. It is
triggered by the peer's avatar message. Transform channels keep writing into the
shell throughout; only the morph bindings (blink) are removed and re-created
against `next.model`.

## Event handlers

What happens to each kind of handler on dispose, and what code must do about it:

| Handler | Example in today's scenes | Survives dispose? | Required |
| --- | --- | --- | --- |
| Scoped to a component inside the body | `on(model, "GLTFInitialized", …)` | No, removed with the subtree | Nothing, apart from rule 3's `disposed` check |
| Scoped to `world` and created by the factory | a per-avatar `FrameTick` animation | Yes | Factory records it in `body.bindings`; dispose removes it |
| Scoped to a persistent component, referencing body parts | `on(left_valid, "UdpSample", …)` calling a hand driver | Yes | Reference shell parts only (the hand drivers are shell-owned), never the body |
| Caller-owned loops that read the body | the multiplayer `FrameTick` writing `local.model.morph_weight(...)` | Yes | Read through `shell.body` on each call, never a captured body; skip when `shell.body` is null or disposed |
| Morph receive bindings | `blink_left.bind_morph(remote.model, …)` | The binding survives, but its target is gone | Remove before dispose; re-create on the new model |
| Animation clips attached to the AVC | ambient eye saccades, desktop blink | No, removed with the subtree | Factory re-creates them for the new model |
| Queued callbacks for removed bindings | any of the above with pending events | Discarded on `.remove()` | Nothing |

The general rule: **code outside the factory only ever reaches the body through
`shell.body`**, and the factory owns cleanup of everything it registers outside
its own subtree. A captured `body` in a long-lived closure is the bug pattern
that produces repeated `component does not exist` errors after a swap.

## Per-frame access: the avatar slot

Today's multiplayer scenes need a `FrameTick` loop to send their avatar's pose,
because typed UDP bindings only work in the receive direction. Subscriptions
apply samples directly (`bind_transform`, `bind_morph`); publications only have
`.write(value)`, and automatic source publication is deferred
([typed channels task](../task/typed-udp-channels-and-mms-bindings.md)). The loop
reads `local.model`, `local.left_driver` and similar through a table captured at
setup. That is the one pattern guaranteed to break on a swap. Changing the
handler's scope does not help; the captured reference is the problem.

Anything that runs every frame against an avatar should therefore go through an
**avatar slot**: a stable place that may or may not hold a body. Per-frame code
asks the slot for its current occupant and skips work when it is empty or not
ready.

What to read from where:

| Data | Source | Survives a swap? |
| --- | --- | --- |
| Root, head and hand poses | Shell transforms | Yes, always valid |
| Hand-tracking validity | Shell `XRHand` drivers | Yes |
| Blink and mouth morph weights | The current body's model, by semantic slot name | Only through the slot |
| Capsule readiness, grounded state | The current body's AVC | Only through the slot |

Semantic indirection for morph *names* already exists: `MorphTargetMap` maps
`left_eye_blink` to each model's own target, and `bind_morph` takes the semantic
name. What is missing is indirection for *which model*.

### Stage 1: MMS slot (no engine change)

The shell table carries `body` (null between bodies) and the swap sequence sets
it. The one publish loop per scene reads through it:

```mms
on(world, "FrameTick", fn(event) {
    // ...rate limit...
    links.root.write(shell.root.world_trs())          // shell: always valid
    links.head.write(shell.head.trs())
    let body = shell.body
    if body == null || body.disposed || !body.ready { return }
    links.blink_left.write(body.model.morph_weight("left_eye_blink"))
})
```

Keep exactly one `FrameTick` publisher per scene, iterating the shells it sends
for, rather than one handler per avatar or per channel. Receive-side morph
bindings are removed and re-created on swap, as described above.

### Stage 2: engine slot and send-side bindings

Two engine pieces would remove the loop entirely:

- **`AvatarSlot` component** on the shell, holding the current body's AVC and
  GLTF and a generation number that increments on each swap. Code and bindings
  target the slot instead of the model. On a new generation, bindings re-resolve
  once, with no per-frame lookup. This mirrors how AVC already follows humanoid
  bone-map generations for its camera anchor.
- **Send-side bindings** on publications, completing the deferred source
  publication: `publication.bind_transform(source, { space, rate_hz })` and
  `publication.bind_morph(slot, "left_eye_blink", { rate_hz })`. Sampling then
  happens in the UDP system's batched I/O phase, not in a script callback.

Bound to an `AvatarSlot`, morph bindings in both directions survive swaps
without any MMS bookkeeping. Transform bindings target shell nodes and need no
slot. Validity flags could become a binding option, which would remove the
remaining imperative `UdpSample` handlers.

Stage 1 is enough for slices 1–3 below. Stage 2 is the point at which avatar
swapping stops depending on every scene author following the rules.

## Networking

- The avatar id travels with the player: in the join handshake / roster entry, so
  late joiners get it without a separate message, and in an `avatar` message on
  each change.
- Until a reliable control channel exists, re-announce the current id every few
  seconds and treat a repeated id as a no-op. A dropped change message otherwise
  leaves the peer showing the wrong body indefinitely.
- Transform channels need no change because they target the shell. The sender
  keeps publishing during a swap, but the receiver ignores morph samples until it
  has re-created those bindings.

## Known glitches and mitigations

| Glitch | Cause | Mitigation |
| --- | --- | --- |
| Avatar vanishes for a few frames | First GLTF load reads and parses the file | Preload every roster model once at startup; later loads hit the URI cache |
| Bind-pose pop-in | The model renders before IK runs | Keep the body hidden until `CapsuleReady`, or fade it in |
| Black view or lost pointers | The camera or hands were removed with the old AVC | Swap step 4 |
| Falling through the floor | No capsule during the swap | Swap steps 3 and 7 |
| Eye-height jump on desktop; different leg contact in XR | Per-model proportions | Apply `eye_height` in step 6; in XR the tracked head stays put and only the body changes |
| Hair or cloth flails briefly | New spring bones start at rest | Accept for now; later, settle secondary motion for a few frames while hidden |
| Wrong body on the peer, or none for a late joiner | Unreliable messages | Avatar id in the roster plus periodic re-announce |
| Error spam after a swap | A world handler calling into the old body | Event-handler rules above |
| Mirror shows a stale body | Renderer bug surface | Headset check; no known cause |

## Roster UI

A panel with one button per roster entry: thumbnail, label, and an outline for the
current selection. It uses the existing raycast/pointer path, so it works with
the desktop pointer and both XR lasers. Thumbnails are static PNGs under each
character's folder; live portraits wait for an MMS `SceneCapture` producer and add
render cost per thumbnail. Buttons are disabled while a swap is in progress.

## Implementation slices

1. **Shell/body split, no UI.** Refactor `twilight_rooftop.mms`'s `make_player`
   into a shell plus `mitten_body`/`lumae_body` factories, still choosing by role.
   Remote bodies spawn on peer join and are disposed on leave, which also resolves
   the frozen-remote-avatar follow-up in
   [multiplayer acceptance](../task/multiplayer-branch-acceptance-and-followups.md).
2. **Local swap.** A debug key swaps the local body through the full sequence.
3. **Networked swap.** Avatar id in the session roster, the `avatar` message,
   periodic re-announce, and the remote swap path.
4. **Roster panel** with static thumbnails.

Slices 1–4 are MMS work plus the roster field in `MultiplayerSession`. No AVC
teardown support is needed, because each swap replaces the whole AVC.

## Validation

- Two-`Universe` host+client test, in the style of
  `twilight_rooftop_runs_as_host_and_client_on_the_roof`, that swaps each side
  twice and asserts:
  - a camera exists every frame on the local side;
  - there are no `component does not exist` errors;
  - the handler and binding counts return to their pre-swap baseline;
  - both capsules become ready and the root does not drop below the roof;
  - blink samples reach the new remote model.
- A late-join test: the client connects after the host has swapped and sees the
  host's current avatar.
- Headset check: swap mid-walk and while holding an item; watch for view loss,
  pop-in and the mirror.

## Open questions

- Should the swap cross-fade both bodies, or is hide-until-ready enough?
- Does the session roster gain a general per-player metadata table (avatar,
  colour, display name) rather than a single avatar field?
- Where does the camera's eye-offset wrapper live while lent: shell or body?
  It is per-model only if eye offsets differ between models.
- Is `AvatarSlot` its own component, or a role of the existing AVC's parent
  transform? A separate component keeps the slot meaningful while it is empty.
- Should a send-side binding sample on its own `rate_hz`, or on a shared
  per-socket tick so one peer's channels stay sample-aligned?
