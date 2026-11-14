# Multiplayer roadmap

Date: 2026-11-13

Status: first slice implemented ([how-to](../how_to/multiplayer.md)). Items
below are scoped follow-ups; each is a separate change.

## First slice (done)

- `NetSession` (host/join/from_env) and `NetSync` (host / local_player /
  remote_player) components with MMS bindings and `Net*` events.
- Host-authoritative roster and physics entities; peer-owned player poses;
  unreliable app messages for client → host intents.
- Versioned JSON envelope, 1,200-byte datagrams, chunked snapshots, stale-tick
  rejection, handshake retry, 3 s timeouts, best-effort `Bye`.
- Tests: protocol/endpoint over loopback, two `Universe`s end to end, and the
  `multiplayer-basic.mms` example run as host + client.

## Next, in rough order

1. **Interpolation buffer.** Keep ~3 snapshots per remote pose and render
   ~100 ms behind using host tick timestamps; extrapolate briefly on loss.
   Hook: the apply loop in `NetSystem::tick` (marked `TODO(net)`). Must not
   allocate per sync per frame.
2. **Reliable ordered channel** for spawn/despawn, gameplay intents, and chat:
   sequence + ACK + resend, reusing the terminal UDP prototype's approach
   (`src/engine/user_input/udp_text`). `protocol::Channel` is the stub.
3. **Replicated spawn/despawn.** Today both peers must author the same
   `NetSync.host(key)` entities. Add host-issued spawn messages naming a prefab
   (MMS module + export) plus initial state, and late-join snapshots of them.
4. **Player animation state.** Replicate AVC inputs (head/hand poses, crouch,
   mouth) instead of only the root, so remote players use the real avatar rig.
   XR players need head + hands; the root alone is insufficient.
5. **Client prediction for host-owned interactions** (grab/kick) with host
   reconciliation. Only after (1)–(3).
6. **Compression and relevance.** Binary encoding, quantized poses, deltas
   against acked snapshots, and per-client interest management.
7. **Rendering-server mode.** The [laptop rendering server](../desktop/rendering-server.md)
   is a spectator client with no player; it should reuse this session layer.
8. **Generic MMS `UdpSocket`** (see `android/docs/udp-lan-keyboard.md`) stays
   separate: it is a raw transport, while `NetSession` owns session semantics.
   `NetSession` may later sit on top of it.
9. **Security/NAT.** Session tokens, optional encryption, and relay/NAT
   traversal before anything leaves a trusted LAN.

## Constraints

- Peers exchange stable keys and world poses, never `ComponentId`s.
- No worker thread per datagram; bounded per-frame receive work; keep blocking
  operations off the XR update path.
- One active `NetSession` per process until multi-session needs are concrete.
