# General UDP send/receive semantics for MMS

Date: 2026-11-13

Status: documentation survey. The general MMS-facing UDP API is specified as a
proposal but is not implemented. Current engine UDP use is limited to the
terminal text/edit prototype and fixed-purpose eye-tracking sockets.

## Document inventory

| Document | Role | Status |
| --- | --- | --- |
| `android/docs/udp-lan-keyboard.md` | Primary source of truth for general UDP component/API semantics, signals, limits, lifecycle, and the keyboard relay | Proposal |
| `android/docs/mms-lan-development.md` | LAN development workflow context; separates the socket from browsing, evaluation, and scene launch | Workflow design; UDP API proposed |
| `docs/desktop/rendering-server.md` | Future versioned UDP transport for scene replication, with recovery/ordering requirements | Blocked on hardware for full validation; design only |
| `docs/how_to/mitten-droid-shell.md` | Shipped narrow UDP terminal text/edit prototype on port 7001 | Implemented prototype |
| `docs/task/optional-mediapipe-tracking-bridge.md` | MediaPipe tracking transport proposal; cites existing eye-tracking UDP as a local pattern | Proposal |
| `docs/task/mitten-droid-mediapipe-tracking-stream.md` | LAN MediaPipe datagram plan to mitten-droid | Proposal |

## Proposed component and API surface

Source: `android/docs/udp-lan-keyboard.md`.

The `with_*` builder names are Rust-only. MMS uses the unprefixed builder
methods, as shown in the MMS column.

| Purpose | Rust surface | MMS surface |
| --- | --- | --- |
| Bind | `UdpSocketComponent::bind(address)` | `UdpSocket.bind(address) {}` |
| Enable/disable at construction | `.with_enabled(bool)` | `.enabled(bool)` |
| Datagram size limit at construction | `.with_max_datagram_bytes(limit)` | `.max_datagram_bytes(limit)` |
| Register runtime | `IntentValue::RegisterUdpSocket { component_id }` | Component materialization |
| Queue a binary send | `IntentValue::UdpSendTo { component_id, send_id, remote_addr, payload }` | `socket.send_to(remote_addr, payload)` |
| Queue a text send | — | `socket.send_text(remote_addr, text)` |
| Change enabled state | `IntentValue::UdpSetEnabled { component_id, enabled }` | `socket.set_enabled(bool)` |
| Remove runtime | `UdpSystem::remove_component(component_id)` | Component removal |
| Diagnostics | `socket.stats()` | `socket.stats()` |
| Subscribe | Handler registration in Rust | `on(socket, "UdpReady" / "UdpDatagram" / "UdpSent" / "UdpError", callback)` |

Proposed semantics:

- `bind(address)` takes a numeric IPv4/IPv6 address; port `0` selects an
  ephemeral port; binding happens asynchronously on initialization.
- Enabled defaults to true; a disabled component owns no live socket.
- `max_datagram_bytes` defaults to 1,200 bytes for the complete UDP payload
  including application envelopes; the initial hard cap is 65,507 bytes for both
  address families.
- `RegisterUdpSocket` creates the runtime or reports a bind/configuration
  failure; registration is idempotent for unchanged configuration.
- `UdpSendTo` queues one owned binary datagram; validate size and destination
  before accepting it.
- `UdpSetEnabled` with `false` closes the socket and invalidates pending work;
  enabling creates a new runtime generation.
- `remove_component` closes the socket, stops or wakes the worker, and discards
  pending deliveries for that runtime.
- `send_text` UTF-8 encodes, queues one datagram, and returns its send ID; count
  encoded bytes.
- `send_to` queues one binary datagram and returns its send ID.
- `set_enabled(bool)` queues a lifecycle change.
- `stats()` reads a bounded diagnostics snapshot.

Send IDs are exposed to MMS as decimal strings to avoid integer precision loss.
A missing, disabled, removed, or failed socket must not silently consume a send
request.

## Proposed event signals

| Signal | Rust payload / MMS fields |
| --- | --- |
| `UdpReady` | `{ local_addr }`: actual bound address, including the chosen port |
| `UdpDatagram` | `{ remote_addr, payload, size_bytes }`: sender address and complete bytes; MMS additionally exposes `body_text`, null for invalid UTF-8 |
| `UdpSent` | `{ send_id, remote_addr, size_bytes }`: the local OS accepted this datagram; it does not acknowledge peer receipt |
| `UdpError` | `{ send_id?, phase, code, message, remote_addr? }`: phases include configuration, bind, send, receive, and queue admission; stable codes identify size, address, and queue errors |

## Semantic rules

- No ordering, retry, evaluation, JSON parsing, or keyboard injection belongs in
  the general socket component.
- Receiving a datagram never implicitly evaluates MMS; interpreting a payload as
  code is an explicit application decision.
- Zero-length datagrams are valid.
- Bounds: at most eight live sockets; raw send/receive queues capped at 256
  datagrams or 256 KiB per socket, whichever is reached first; aggregate worker
  and queue memory is also bounded.
- Oversized received datagrams are discarded, never truncated into a valid
  `UdpDatagram`; use reusable receive storage large enough to detect oversize
  packets (for example 65,536 bytes).
- No thread per datagram; use persistent bounded workers with wakeups or bounded
  waits, and keep blocking joins off the XR update path.
- Send failures are observable through `UdpError` while the component exists;
  removing a component is an explicit cancellation boundary, and callbacks are
  not promised after deletion.
- `stats()` exposes bounded counters for received/sent packets and bytes, oversize
  drops, queue drops, and coalesced errors; do not emit an error per packet in an
  overload flood.
- Numeric unicast addresses are sufficient initially; hostname resolution,
  discovery, broadcast, and multicast are separate extensions.

Example shape:

```mms
let socket = UdpSocket.bind("0.0.0.0:7001").max_datagram_bytes(1200) {}
socket

on(socket, "UdpDatagram", fn(packet) {
    socket.send_to(packet.remote_addr, packet.payload)
})
```

## Supporting design constraints

| Topic | Documented position |
| --- | --- |
| Socket vs evaluation | Keep transport separate from browsing, input, scene launch, and evaluation; both transports may eventually feed the same evaluation queue |
| Reliable operations | Use HTTP initially for listings, launch/control, terminal submissions, and relay setup |
| UDP suitability | Telemetry and frequently updated state where a newer sample supersedes a lost one; unordered and unreliable otherwise |
| Keyboard relay | Separate `KeyboardRelay` component with ordering, ACKs, retries, leases, focus generations, and held-key cleanup; generic UDP alone does not satisfy the input contract |
| Scene replication | Needs protocol version, session identity, sequence numbers, timestamps, snapshot plus ordered changes, loss/reorder/reconnect handling, periodic recovery, bounded queues, and a trusted-LAN boundary |
| Payload budget | 1,200 bytes is an application starting point, not a guarantee for every network path; avoid fragmentation and measure the real path |

## Implementation status

| Item | Status |
| --- | --- |
| `UdpSocketComponent` / `UdpSystem` / `UdpDatagram` / `UdpSendTo` | Not present in `src/` |
| General MMS UDP socket API | Not implemented; proposal only |
| Gameplay-key relay | Not implemented |
| Terminal text/edit UDP prototype | Implemented in `src/engine/user_input/udp_text/`; version-1 JSON envelope, sequence/ACK, duplicate suppression, 1,200-byte datagrams |
| Eye-tracking UDP | Implemented in `src/engine/ecs/system/xr_eye_tracking_system.rs`; fixed-purpose OSC/HTC sockets, not reusable for general MMS traffic |
