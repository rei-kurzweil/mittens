# Multiplayer roadmap

Date: 2026-11-13

Status: first slice implemented ([how-to](../how_to/multiplayer.md)). Items
below are scoped follow-ups; each is a separate change.

## Original first slice (historical)

- `MultiplayerSession` (host/join/from_env) and `MultiplayerSync` (host / local_player /
  remote_player) components with MMS bindings and `Multiplayer*` events.
- Host-authoritative roster and physics entities; peer-owned player poses;
  unreliable app messages for client → host intents.
- Versioned JSON envelope, 1,200-byte datagrams, chunked snapshots, stale-tick
  rejection, handshake retry, 3 s timeouts, best-effort `Bye`.
- Tests: protocol/endpoint over loopback, two `Universe`s end to end, and the
  `multiplayer-basic.mms` example run as host + client.

## Current priority: typed channels and MMS-controlled application

The [typed UDP channels and MMS bindings](typed-udp-channels-and-mms-bindings.md)
task records the channel foundation, receiver-controlled transform/morph bindings,
and migration of `multiplayer-basic.mms`. It also retains the detailed audio
milestones. The list below keeps longer-term follow-ups; it is not an instruction
to implement them all now.

The typed-channel foundation and full-avatar demo migration are now implemented.
Use [branch acceptance and follow-ups](multiplayer-branch-acceptance-and-followups.md)
for the current test checklist and remaining interaction/audio work. The historical
first-slice notes describe that earlier protocol;
root/head/hand/blink telemetry in the demo uses binary typed channels.
MultiplayerSync and the automatic replication runtime are now removed; session
control only handles connections, roster, heartbeats and app messages.
[Posture/tracking inventory](avatar-posture-tracking-inventory.md) is a separate,
docs-only design track, not more implementation scope for this branch.

## Longer-term follow-ups

1. **Interpolation buffer.** Keep ~3 snapshots per remote pose and render
   ~100 ms behind using typed-channel sample timestamps; extrapolate briefly on loss.
   Hook: typed subscription receive bindings. Avoid per-avatar per-frame allocation.
2. **Reliable ordered channel** for spawn/despawn, gameplay intents, and chat:
   sequence + ACK + resend, reusing the terminal UDP prototype's approach
   (`src/engine/user_input/udp_text`). `protocol::Channel` is the stub.
3. **Replicated spawn/despawn.** Today both peers must author the same
   shared objects in MMS. Add host-issued spawn messages naming a prefab
   (MMS module + export) plus initial state, and late-join snapshots of them.
4. **Player animation state.** Root/head/hands and blinks are implemented through
   typed channels. Crouch/mouth and further animation inputs remain follow-ups.
5. **Client prediction for host-owned interactions** (grab/kick) with host
   reconciliation. Only after (1)–(3). First establish shared grab ownership,
   release, and physics authority as described in
   [shared multiplayer grabbing](multiplayer-shared-grabbing.md); prediction is
   a later layer.
6. **Compression and relevance.** Binary encoding, quantized poses, deltas
   against acked snapshots, and per-client interest management.
7. **Rendering-server mode.** The [laptop rendering server](../desktop/rendering-server.md)
   is a spectator client with no player; it should reuse this session layer.
8. **Shared native UDP transport.** Session, typed channels, eye adapters and
   terminal use `UdpSystem`; protocol systems remain separate. MMS `UdpSocket`
   is the typed-channel authoring surface.
9. **Security/NAT.** Session tokens, optional encryption, and relay/NAT
   traversal before anything leaves a trusted LAN.

## Constraints

- Peers exchange stable keys and world poses, never `ComponentId`s.
- No worker thread per datagram; bounded per-frame receive work; keep blocking
  operations off the XR update path.
- One active `MultiplayerSession` per process until multi-session needs are concrete.
