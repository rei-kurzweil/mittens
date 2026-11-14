# Typed UDP channels and MMS bindings

Status: native binary codec and channel endpoint implemented; MMS socket
values, channel builders and transform bindings are not implemented yet.
Multiplayer naming is already updated. Design and phase 2 scope: [discussion draft](../draft/typed-telemetry-mms-bindings.md).

## Native foundation completed

`src/engine/udp_channels/` provides the binary codec, TypeTag/RecordType schema
adapter, bounded nonblocking socket, publication/subscription arenas, declaration
retry/acknowledgment, latest-value writes/reads, mismatch events and lifecycle
history. Protocol and limits: [native channel reference](../spec/typed-udp-channel-wire.md).

Validation: 12 functional tests pass with UDP loopback access; the ignored
measurement is run through `scripts/benchmark_udp_channel_codec.sh`, which
optimizes the channel modules and dependencies without rebuilding the renderer.
These tests do not yet exercise MMS socket values or a channel-based scene.
Next: host ownership and builder integration for the proposed MMS API, then
adapt the existing demo and end-to-end tests.

## Goal

Expose first-class socket values and typed channels in MMS. Publish explicitly
with `.write(value)`; receive through Sample handlers or subscription-owned
`.bind_transform(target)`. Use type tags imported from `"types"` and structural
record types, compact binary payloads and channel identities. Keep local scene
and physics policy in MMS. Do not introduce a global Bind component/table.

## Testable slices

1. Settle socket ownership/routing, registration and portable schema semantics;
   select a binary codec using vectors, matrices, TRS records and occasional
   key-value data. Measure packet size and encode/decode cost against JSON.
   Test round trips, malformed data and packet limits. Matrix tag support needs
   a language type-system extension; do not invent a separate network tag system.
2. Implement socket handles and publish/subscribe registration. Test several
   channels sharing one socket, schema mismatch, subscriber-before-publisher,
   generation/session reuse protection, readiness and cleanup over loopback.
3. Add imperative channel writes and Sample delivery. Test typed MMS exchange,
   multiple subscribers, stale-sequence handling and bounded receive work,
   without implicit scene writes.
4. Add subscription-owned whole-TRS receive binding with explicit space,
   lifetime, timeout and write ownership. Test target deletion and channel loss.
5. Add execution-time partial transform writes and independent imperative
   translation/rotation mappings, proving queued changes do not clobber each
   other. Follow with declarative individual-property receive bindings. Partial
   writes must be available before those imperative mappings are exposed.

## Reuse the two-window desktop demo

Use [examples/multiplayer-basic.mms](../../examples/multiplayer-basic.mms) as the
visible before/after demonstration, rather than creating an unrelated cube-only
scene. It already shows remote players, a host-simulated bouncing ball and an
E-to-kick interaction in two desktop processes. Existing automated coverage is
in [multiplayer_tests.rs](../../src/scripting/multiplayer_tests.rs), including
`multiplayer_basic_example_runs_as_host_and_client` and a two-Universe test.

- Preserve the current working demo until the channel path is functional.
- Migrate pose publication to explicit MMS channel writes and reception to
  `.subscribe(...).type(pose_type).bind_transform(target).space("world") {}`.
- Make simulation ownership explicit in MMS rather than implicitly disabling
  Velocity when telemetry arrives. Preserve host authority and kick behavior;
  a telemetry channel does not supply reliable gameplay-command delivery.
- Add a visible independent translation/rotation channel demonstration when
  partial writes are available. Whole-transform and property-level examples
  must not compete for the same target properties.
- Adapt the existing host/client automated tests to exercise the new channel
  path, keeping assertions for remote poses, host-owned physics and messages.

Manual validation uses two terminals/windows:

```sh
MITTENS_MULTIPLAYER_ROLE=host MITTENS_MULTIPLAYER_NAME=rei cargo run --release -- load examples/multiplayer-basic.mms
MITTENS_MULTIPLAYER_ROLE=client MITTENS_MULTIPLAYER_NAME=dan cargo run --release -- load examples/multiplayer-basic.mms
```

These are the current launch commands. Preserve this workflow or update the
example, tests and how-to together if launcher configuration changes. Verify
remote motion in both windows, ball authority and E-to-kick, plus restart/loss
behavior. Revisit or replace MultiplayerSync after the replacement path passes.

Also validate the same scene on two computers over Tailscale: host binds
`0.0.0.0:7010`, client sets `MITTENS_MULTIPLAYER_ADDR=<host-tailscale-ip>:7010`.
Keep numeric remote addresses configurable; never hardcode loopback into the
replacement. See [launch instructions](../how_to/multiplayer.md#two-computers-over-tailscale).
Local loopback tests do not constitute verification of the remote network path.

## Avatar acceptance target: Rei and Dan as Mitten

The migrated scene must render the full Mitten avatar for both peers on each
computer, rather than the current sphere/cube remote proxies. Reuse
`assets/models/mitten.glb` and the existing Mitten humanoid map, anime material,
blink morph mapping and appropriate local avatar/secondary-motion setup from
`examples/mitten.mms`, `examples/mittens-corp.mms` and its desktop counterpart.
Do not copy unrelated vehicle/editor systems into the multiplayer example.

Each computer runs AVC/IK locally for both the locally owned avatar and received
remote avatars. Transmit solver inputs, not solved bone transforms:

- Authoritative locomotion-root transform, plus movement state needed by local
  animation. Gamepad input drives locomotion on the owning peer. Raw stick values
  alone are not enough to keep remote position aligned; do not independently
  integrate the same locomotion on receivers and introduce drift.
- Headset/head target and left/right hand target poses, with explicit tracking
  validity, coordinate space, root relationship and per-peer calibration.
- Blink/face morph weights, starting with independent left/right eye closure.
  Existing semantic slots map to Fcl_EYE_Close_L / Fcl_EYE_Close_R. Additional
  supported hand/finger inputs or expression weights can follow as required;
  actual voice streaming remains phase 2.

Add subscription-owned scalar morph-target receive binding alongside transform
binding. Resolve the target within the correct avatar's MorphTargetMap/GLTF
instance after initialization; never globally query a shared bone/morph name.
Define finite/range validation, target-load/loss behavior and write ownership.
The remote avatar's automatic blink/tracking writers must not overwrite received
blink values. Local camera, InputXR, controller input and microphone providers
must not take control of a remote avatar, and received channels must not be
republished as local input.

The desktop two-window version must remain testable without XR hardware: use
local desktop controls and deterministic head/hand/blink driver inputs to prove
remote motion and left/right blinking. Add assertions for avatar initialization,
per-peer target isolation, received target/morph values and local IK consumption.
Then manually validate headset/controller motion and blinking from Rei to Dan
and back over Tailscale. Local tests do not prove headset or remote-network
behavior. Keep this target explicit while the MMS/channel adapters are being
implemented; the current example still uses simple proxies.

## Constraints and later scope

Keep nonblocking bounded I/O, reusable buffers and direct/batched application;
no per-channel worker threads, repeated name resolution per sample or accidental
per-target per-frame allocations. Wire IDs resolve through a dedicated channel
arena to local handles; never transmit ComponentId as cross-process identity.
Before adding to oversized source files, update the relevant responsibility-split
tickets under docs/task; scoped implementation does not authorize those splits.

Automatic source publication is deferred. AudioStream, writable Rust producer
handles, subscription audio binding, nested AudioSourceKind and voice chat are
phase 2, after basic channels and whole/property transform bindings. See the
design draft for that scope.
