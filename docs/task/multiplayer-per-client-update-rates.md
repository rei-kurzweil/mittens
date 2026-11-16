# Per-client multiplayer update rates

Date: 2026-11-15

Status: proposed; implementation not started.

## Goal

Allow the host to send current state to different connected clients at different
configured rates, for example 60 Hz, 20 Hz, and 10 Hz, without changing simulation
speed or delaying connection control and gameplay commands.

## Current behavior

- `MultiplayerSession.tick_hz` schedules roster/heartbeat control for the whole
  session. The host publishes to all clients at the same cadence; this setting
  does not control typed spatial telemetry.
- Typed UDP publications have individual destination addresses. MMS chooses
  when to call `.write()`; pending writes coalesce before socket I/O. Per-peer
  scheduling can therefore use the existing wire format.
- `examples/multiplayer-basic.mms` uses one hardcoded 20 Hz telemetry timer and
  one remote avatar/connection/channel set. A new peer replaces the previous
  connection's channels. It is currently a two-peer scene.
- `assets/components/network/shared_grabbable.mms` also has one remote connection
  and its own 20 Hz timer. Session control does not automatically relay typed
  telemetry between clients.

## Proposed first slice

- Store outgoing channel groups and scheduling state per peer, with a
  server-assigned rate and next deadline. Retain 20 Hz as the default. Define
  finite positive rate bounds and reject invalid configuration explicitly.
- Allow a connected peer's rate to change at runtime. Keep scheduling policy
  accessible from MMS; start with a focused MMS helper unless native scheduling
  is justified by measured cost or reuse. Do not imply a new API already exists.
- Send the latest state when a peer is due. Skip missed deadlines after a late
  frame instead of replaying old samples or bursting to catch up. Preserve
  fractional timing so frame rounding does not systematically lower the rate.
  Actual publication frequency is bounded by the socket-I/O/frame cadence.
- Sample common source state once for the peers due in a frame and reuse it
  across destinations. Keep receiving active every frame, independent of the
  outgoing rate. Do not slow physics, local tracking, or rendering.
- Keep session heartbeats/roster control on the existing independent schedule.
  Gameplay commands and ownership transitions must not wait for a telemetry
  deadline; retain the existing delivery semantics without claiming reliability.
- Clean up per-peer channels/timers on departure and recreate them on reconnect.
  Ignore callbacks belonging to an obsolete connection generation.

## Multi-client integration

The scheduler can be validated with host-owned state and two receiving clients
before expanding the full avatar/grab demo.

- Replace the demo's single connection and remote avatar with peer-keyed
  connection groups and avatar instances when integrating multi-client avatars.
- Specify channel identity, telemetry-port discovery/configuration, and source
  attribution for multiple peers. Choose explicitly between direct peer
  publication and host relay for client-owned avatars; a host fan-out of its own
  state alone does not replicate every client's avatar to every other client.
- Update shared-grab connection storage and scheduling before presenting those
  props as multi-client capable. The existing two-rank ownership policy needs a
  separately specified multi-client policy; coordinate with
  [shared multiplayer grabbing](multiplayer-shared-grabbing.md).
- Preserve the existing two-window and numeric-address LAN/Tailscale workflows.

## Performance constraints

- Nonblocking I/O and bounded per-frame work; no per-client worker threads or
  unbounded telemetry queues. Reuse resolved channels and source bindings.
- Avoid repeating scene traversal, source sampling, or resource construction
  for each destination. Distinct destination sends remain necessary.
- Measure observed send rates, datagrams/bytes per peer, and scheduling/sampling
  CPU cost with multiple clients. Stagger deadlines if synchronized fan-out
  creates meaningful frame spikes.

## Validation and acceptance

- One host sends changing state to at least two clients at distinct configured
  rates. Count received samples over a defined interval with tolerances for
  frame cadence and UDP loss; demonstrate independent progression.
- Deterministic scheduler coverage verifies fractional timing, invalid rates,
  runtime rate changes, and a long frame producing no catch-up burst.
- Disconnect/reconnect removes stale scheduling state and channel callbacks.
  A slow telemetry recipient remains connected through independent control
  heartbeats, and incoming data continues to be polled each frame.
- Loss/reordering does not build a backlog; the next publication contains the
  latest state. Preserve existing stale-sample rejection.
- Verify the existing default 20 Hz two-peer workflow and document the selected
  configuration surface and observed performance.
- Full multi-client demo acceptance additionally verifies separate avatars,
  source routing, join/leave cleanup, and the chosen shared-object policy.

## Separate follow-ups

Client-requested rate negotiation, automatic bandwidth/congestion adaptation,
per-object relevance/rates, reliable commands, and interpolation are separate
changes. Received poses currently apply directly, so lower rates visibly step;
this task does not promise smooth low-rate motion. See the
[multiplayer roadmap](multiplayer-roadmap.md).
