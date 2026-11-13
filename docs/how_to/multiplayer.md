# Multiplayer (first slice)

Status: implemented first slice. Desktop/loopback/LAN verified by tests; no
interpolation, prediction, or reliable delivery yet. See the
[roadmap](../task/multiplayer-roadmap.md) for what comes next.

## Run the example

Two processes on one machine:

```sh
MITTENS_NET_ROLE=host   MITTENS_NET_NAME=rei cargo run --release -- load examples/multiplayer-basic.mms
MITTENS_NET_ROLE=client MITTENS_NET_NAME=dan cargo run --release -- load examples/multiplayer-basic.mms
```

Across a LAN, the host binds `0.0.0.0:7010` by default; set
`MITTENS_NET_ADDR=<host-ip>:7010` on the client and allow UDP 7010 on the host.
Addresses are numeric; hostnames are not resolved.

Each window shows the other player as a head/body proxy with a name tag. The
orange ball is simulated only by the host. Press **E** to kick it: the host
kicks directly, a client sends a `kick` message that the host acts on.

## Authority model

| State | Owner | Path |
| --- | --- | --- |
| Roster (who is connected) | Host | Carried in every snapshot; clients diff it into `NetPeerJoined`/`NetPeerLeft` |
| Player pose | The peer that owns the player | Client → host (`PlayerPose`), host → all (`Snapshot.players`) |
| World/physics entities | Host | Host → clients (`Snapshot.entities`); clients disable the driving `Velocity` |
| Gameplay intents | Requested by anyone, decided by host | `NetSession.send(name, text)` → `NetMessage` on the host |

Clients never talk to each other directly; the host relays.

## MMS surface

```mms parse-only
let net = NetSession.from_env() {}   // or .host("0.0.0.0:7010") / .join("192.168.1.20:7010")
net

// This peer's player: publish the pose of the parent transform.
T { NetSync.local_player() C3D {} }

// Host-owned body: the host simulates and publishes; clients only apply.
Gravity.enabled(true) { Velocity { T { NetSync.host("ball") R.sphere() {} } } }

// Another peer's player, typically spawned from NetPeerJoined.
fn proxy(event) { return T { NetSync.remote_player(event.peer_id) R.cube() {} } }
on(net, "NetPeerJoined", fn(event) { net_proxies.attach(proxy(event)) })
```

`NetSession` constructors: `host(addr?)`, `join(host_addr)`, `from_env()`
(`MITTENS_NET_ROLE` = `host`|`client`, `MITTENS_NET_ADDR`, `MITTENS_NET_NAME`).
Builders: `player_name(name)`, `tick_hz(hz)` (default 20), `enabled(bool)`.
Methods: `send(name, text)`, `is_host()`, `peer_id()`, `local_name()`.

Events on the session component:

| Event | Payload |
| --- | --- |
| `NetConnected` | `peer_id`, `is_host` |
| `NetPeerJoined` / `NetPeerLeft` | `peer_id`, `name` |
| `NetMessage` | `sender`, `name`, `text` (`from` is an MMS keyword) |
| `NetError` | `phase`, `message` |

`NetSync` replicates the **world** pose of its parent `Transform`, applied with
`SetTransformTrs` in world space. Only one `NetSession` may be active per
process. The host is always peer `1`; clients are assigned `2, 3, …`.

## Wire and runtime behavior

- UDP, JSON envelope `{v, session, seq, msg}`; version mismatches are dropped.
  Every datagram stays within 1,200 bytes; large snapshots are split into
  independently applicable chunks (only the first carries the roster).
- Nonblocking socket polled once per frame, at most 256 datagrams per poll; no
  worker thread and no blocking on the update path. Oversize datagrams are
  discarded, not truncated.
- The net tick runs after fixed-step physics and before camera/XR publication,
  so the host publishes this frame's simulation and received poses are visible
  in the same frame.
- Sends happen at `tick_hz`; a late frame does not burst to catch up.
- Clients resend `Hello` every 0.5 s until welcomed. Either side treats 3 s of
  silence as a disconnect; a client then re-handshakes automatically. Dropping
  the session sends a best-effort `Bye`.
- Snapshots older than the newest applied tick are ignored.
- Trusted-LAN only: no authentication or encryption. The host does override the
  claimed sender of app messages with the sender's assigned peer id.

Implementation: [`net_system.rs`](../../src/engine/ecs/system/net_system.rs)
and its `net_system/` modules (`protocol`, `transport`, `endpoint`).
