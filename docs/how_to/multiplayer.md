# Multiplayer (first slice)

Status: implemented first slice. Desktop/loopback/LAN verified by tests; no
interpolation, prediction, or reliable delivery yet. See the
[roadmap](../task/multiplayer-roadmap.md) for what comes next.

## Run the example

Two processes on one machine:

```sh
MITTENS_MULTIPLAYER_ROLE=host   MITTENS_MULTIPLAYER_NAME=rei cargo run --release -- load examples/multiplayer-basic.mms
MITTENS_MULTIPLAYER_ROLE=client MITTENS_MULTIPLAYER_NAME=dan cargo run --release -- load examples/multiplayer-basic.mms
```

Across a LAN, the host binds `0.0.0.0:7010` by default; set
`MITTENS_MULTIPLAYER_ADDR=<host-ip>:7010` on the client and allow UDP 7010 on the host.
Addresses are numeric; hostnames are not resolved.

### Two computers over Tailscale

Use the same example on both computers; no separate scene is required. Both
must run a compatible branch/protocol. On Rei's computer, get the numeric IPv4
address with `tailscale ip -4`, then run:

```sh
MITTENS_MULTIPLAYER_ROLE=host MITTENS_MULTIPLAYER_NAME=rei cargo run --release -- load examples/multiplayer-basic.mms
```

On Dan's computer, replace `<rei-tailscale-ip>` with that numeric address:

```sh
MITTENS_MULTIPLAYER_ROLE=client MITTENS_MULTIPLAYER_NAME=dan MITTENS_MULTIPLAYER_ADDR="<rei-tailscale-ip>:7010" cargo run --release -- load examples/multiplayer-basic.mms
```

The default host bind `0.0.0.0:7010` includes its Tailscale IPv4 interface.
Tailnet access rules and the host firewall must permit this UDP traffic. Use
numeric addresses; the current example does not resolve MagicDNS hostnames.
Tailscale handles connectivity between the machines, including relayed paths
when direct connectivity is unavailable; see its
[connection types](https://tailscale.com/docs/reference/connection-types).
This two-computer workflow has not been verified by the local loopback tests.
The channel-based migration must preserve it alongside the two-window local
workflow; the demo uses MultiplayerSession for control and typed channels for telemetry.

Each desktop window shows full local and remote Mitten avatars. WASD/right-drag
moves the local rig; scripted hand targets exercise arm IK without a headset. Desktop idle blinks
use a one-second interval so the networked eye closures are easy to observe.
The receiving machine solves AVC/IK locally and binds the received blink weights
to that avatar's morph slots. The orange ball is simulated only by the host.
Press **E** to kick it; client requests remain host-authoritative.

The control protocol is version 2 (including pose-independent heartbeats); run
the same checkout on both computers. The control session uses UDP 7010. Typed telemetry uses UDP 7011 on the host and
7012 on the client, so tailnet/host firewall policy must also allow those ports
between the machines in both directions. Peer IPs come from the control session;
localhost is not hardcoded into the channel setup. This demo is a two-peer scene.

Set `let use_xr = true` in the demo on a machine using XR hardware. Its head and
hands then come from InputXR/controllers with gamepad locomotion. Each machine
may choose its own mode; remote rigs use PoseInput and external hand targets,
never the receiver's local tracking. Physical XR/Tailscale validation is still
required; automated tests run the desktop mode headlessly.

See [typed channel authoring](typed-udp-channels.md) for the implemented API.
The session/MultiplayerSync examples below describe the older automatic
replication API, which remains supported separately from the migrated demo.

## Authority model

| State | Owner | Path |
| --- | --- | --- |
| Roster (who is connected) | Host | Carried in every snapshot; clients diff it into `MultiplayerPeerJoined`/`MultiplayerPeerLeft` |
| Player pose | The peer that owns the player | Client → host (`PlayerPose`), host → all (`Snapshot.players`) |
| World/physics entities | Host | Host → clients (`Snapshot.entities`); clients disable the driving `Velocity` |
| Gameplay intents | Requested by anyone, decided by host | `MultiplayerSession.send(name, text)` → `MultiplayerMessage` on the host |

Clients never talk to each other directly; the host relays.

## MMS surface

```mms parse-only
let net = MultiplayerSession.from_env() {}   // or .host("0.0.0.0:7010") / .join("192.168.1.20:7010")
net

// This peer's player: publish the pose of the parent transform.
T { MultiplayerSync.local_player() C3D {} }

// Host-owned body: the host simulates and publishes; clients only apply.
Gravity.enabled(true) { Velocity { T { MultiplayerSync.host("ball") R.sphere() {} } } }

// Another peer's player, typically spawned from MultiplayerPeerJoined.
fn proxy(event) { return T { MultiplayerSync.remote_player(event.peer_id) R.cube() {} } }
on(net, "MultiplayerPeerJoined", fn(event) { net_proxies.attach(proxy(event)) })
```

`MultiplayerSession` constructors: `host(addr?)`, `join(host_addr)`, `from_env()`
(`MITTENS_MULTIPLAYER_ROLE` = `host`|`client`, `MITTENS_MULTIPLAYER_ADDR`, `MITTENS_MULTIPLAYER_NAME`).
Builders: `player_name(name)`, `tick_hz(hz)` (default 20), `enabled(bool)`.
Methods: `send(name, text)`, `is_host()`, `peer_id()`, `local_name()`.

Events on the session component:

| Event | Payload |
| --- | --- |
| `MultiplayerConnected` | `peer_id`, `is_host` |
| `MultiplayerPeerJoined` / `MultiplayerPeerLeft` | `peer_id`, `name` |
| `MultiplayerMessage` | `sender`, `name`, `text` (`from` is an MMS keyword) |
| `MultiplayerError` | `phase`, `message` |

`MultiplayerSync` replicates the **world** pose of its parent `Transform`, applied with
`SetTransformTrs` in world space. Only one `MultiplayerSession` may be active per
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

Implementation: [`multiplayer_system.rs`](../../src/engine/ecs/system/multiplayer_system.rs)
and its `multiplayer_system/` modules (`protocol`, `transport`, `endpoint`).
