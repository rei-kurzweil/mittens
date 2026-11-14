# Typed UDP channels and MMS bindings

Status: planned; implementation has not started. Multiplayer naming is already
updated. Design and phase 2 scope: [discussion draft](../draft/typed-telemetry-mms-bindings.md).

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
