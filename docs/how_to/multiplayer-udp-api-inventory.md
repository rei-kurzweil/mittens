# Multiplayer and UDP: component and signal inventory

Implemented API on this branch. Names are current MMS spellings; multiplayer
uses `Multiplayer*`, while general typed transport uses `Udp*`.

The four tables include authored/runtime component handles and both kinds of ECS
signal. **Events** can be observed with MMS `on(handle, "EventName", fn(event) {})`.
**Internal intents** are engine commands, not event subscriptions or extra MMS
constructors. Their component IDs are local handles, not network identities.

## Multiplayer components

| MMS component | Rust component | Creation / important API | Responsibility and current use |
| --- | --- | --- | --- |
| `MultiplayerSession` | `MultiplayerSessionComponent` | `.host(bind_address)`, `.join(host_address)`, `.from_env()`; builders `.player_name(name)`, `.tick_hz(rate)`, `.enabled(bool)`; methods `.is_host()`, `.peer_id()`, `.local_name()`, `.peer_ip(peer_id)`, `.send(name, text)` | Host/client session, peer roster, peer IDs/addresses, heartbeat/lifecycle and application messages. The current example uses this control plane alongside separate typed UDP channels. `from_env()` reads the specific `MITTENS_MULTIPLAYER_*` launcher variables; it is not general MMS environment access. |

The former `MultiplayerSync` component and its registration intent have been
removed. Use explicit typed publications/subscriptions and MMS receive bindings
for avatar and object telemetry. Session control does not apply transforms or
change physics authority.

## Multiplayer signals

| Signal | Kind / MMS exposure | Scope / producer | Payload | Meaning |
| --- | --- | --- | --- | --- |
| `MultiplayerConnected` | Event; observable payload | Session; `MultiplayerSystem` | `peer_id`, `is_host` | Local session has its peer identity; the host is peer 1. |
| `MultiplayerPeerJoined` | Event; observable payload | Session; `MultiplayerSystem` | `peer_id`, `name` | Another peer joined the session roster. Used by the example to create its channel group and configure the peer address. |
| `MultiplayerPeerLeft` | Event; observable payload | Session; `MultiplayerSystem` | `peer_id`, `name` | Another peer left or timed out. Used for channel cleanup and shared-object disconnect policy. |
| `MultiplayerMessage` | Event; observable payload | Session; `MultiplayerSystem` | `from`, `name`, `text` | Application message from another peer, originating from `.send(name, text)`. The current app-message transport is best effort, not reliable delivery. |
| `MultiplayerError` | Event; observable payload | Session; `MultiplayerSystem` | `phase`, `message` | Session configuration, bind, send or other reported protocol/transport failure. |
| `RegisterMultiplayerSession` | Internal intent; not `on(...)` | Session initialization → system registration | `component_id` | Activates/registers the session endpoint. |
| `MultiplayerSend` | Internal intent; not `on(...)` | Session `.send(...)` → `MultiplayerSystem` | `component_id`, `name`, `text` | Requests sending an application message through that session. |

## UDP components

| MMS component / handle type | Rust component | Creation / important API | Responsibility and current use |
| --- | --- | --- | --- |
| `UdpSocket` | `UdpSocketComponent` | `UdpSocket.bind("numeric-ip:port") {}`; `.publish(name)`, `.subscribe(name)`, `.ready()`, `.local_addr()` | First-class local socket handle. One socket carries multiple named typed channels and can be passed into MMS helpers/controllers. Emit or attach the handle to activate it. |
| `UdpPublication` | `UdpChannelComponent` with `publishing = true` | Returned by `socket.publish(name)`; configure `.to(peer_address).type(type_tag_or_record)`; methods `.ready()`, `.write(value)` | Outgoing typed channel. Configure before attaching/activating, then write values when ready. `UdpPublication {}` is not a supported direct constructor. |
| `UdpSubscription` | `UdpChannelComponent` with `publishing = false` | Returned by `socket.subscribe(name)`; configure `.from(peer_address).type(type_tag_or_record)`; `.ready()`, `.bind_transform(target, options)`, `.bind_morph(model, slot, options)` | Incoming typed channel. Either observe `UdpSample` in MMS or configure a direct transform/morph receive binding before activation. Binding methods return the same channel handle, not another component. `UdpSubscription {}` is not a supported direct constructor. |

## UDP signals

| Signal | Kind / MMS exposure | Scope / producer | Payload | Meaning |
| --- | --- | --- | --- | --- |
| `UdpSample` | Event; observable payload | Unbound subscription; `UdpChannelSystem` | `value` | A decoded typed sample for imperative MMS handling. A subscription configured with `bind_transform` or `bind_morph` applies samples directly instead of also emitting this event. |
| `UdpState` | Event; observable payload | Socket/publication/subscription whose state changed; `UdpChannelSystem` | `ready`, `message` | Readiness/error-state transition. `message` is empty when there is no reported error. Configuration/call failures can also be returned directly by MMS methods. |
| `RegisterUdp` | Internal intent; not `on(...)` | Socket/channel initialization → `UdpChannelSystem` | `component_id` | Registers a socket, publication or subscription through the normal component lifecycle. |

## Boundaries and related APIs

- `MultiplayerSystem` and `UdpChannelSystem` are systems, not authored components.
- The multiplayer session/control protocol is still JSON; typed UDP telemetry is
  binary. A session and a typed socket are separate handles in the current example.
- `.type(...)` uses tags/record types imported from `"types"`. Those are type
  values, not components or signals.
- There is no `Bind`/`UdpBind` component or built-in binding table. Transform/morph
  bindings are ordinary subscription methods with options tables.
- `PointerGrabbed` and `PointerReleased` are generic local interaction events,
  scoped to a pointer with `pointer` and `target` payload fields. They are not
  multiplayer/UDP events. The ordinary MMS `shared_grabbable(...)` helper uses
  them to publish ownership and object poses; it is not a native component.
- PoseInput, AVC/WalkingIK, morph read/write methods, AudioStream and proposed
  VoiceChatSystem belong to other API families. AudioStream/voice integration
  remains future work; it is not part of this implemented transport inventory.

## Underlying transport and reuse

All these native UDP paths currently use Rust's standard-library
`std::net::UdpSocket`; there is no dedicated third-party UDP networking crate
underneath them. The typed-channel endpoint is our own code, not Tokio, Quinn,
Renet, or another session/reliability library.

The MMS socket/channel API, multiplayer session transport, OSC/HTC eye receivers,
and terminal UDP input now use the shared native `UdpSystem`. `SystemWorld.udp`
is the common scene owner. The installed terminal provider and shell-client
sender use it too. Standalone Rust senders/endpoints/tests can create their own
owner of the same implementation through the backward-compatible constructors;
`bind_on` / `new_on` allow explicit sharing.

UdpSystem owns common bind/send/receive/error handling, one reusable 64 KiB
receive buffer, endpoint leases and diagnostics. Consumers preserve their own
wire formats, registration/session/ACK policy, semantic buffers and update phases.
This does not merge all protocols into one wire format or one socket, or make
channel samples reliable. Channel ACK/retry still applies to registration only.

Polling is bounded per endpoint and directly borrows receive storage; sending
ACKs inside callbacks is supported. Concurrent/recursive polling defers instead
of blocking on the shared scratch buffer. Final endpoint-lease drop closes the
socket, with weak diagnostic registrations pruned on inspection/bind. There are
no extra per-packet transport queues/copies or worker threads.

`UdpSystem` is a native service, **not another MMS component or signal**. The four
tables above and public MMS socket/binding syntax are unchanged. Eye receive work
is now explicitly bounded at 256 datagrams per source/update. Terminal receiver
and ACK budgets remain 64/16; session/channel budgets remain 256. Oversized
packets are discarded before their protocol callback.

The [benchmark report](../benchmarks/udp-transport-locality.md) distinguishes the
original buffer-only experiment from measurements of the actual runtime owner.
Shared I/O does not imply a guaranteed speedup; central lifecycle/error handling
and unchanged external protocol compatibility are the main maintenance benefit.

## Sources and further reading

- [Session component](../../src/engine/ecs/component/multiplayer_session.rs)
- [Multiplayer MMS configuration](../../src/scripting/registries/multiplayer_runtime_config.rs)
- [Socket/channel components](../../src/engine/ecs/component/udp_channel.rs)
- [UDP MMS methods and bindings](../../src/scripting/udp_channel_api.rs)
- [Event and intent definitions](../../src/engine/ecs/signals/signal.rs)
- [MMS event payload conversion](../../src/scripting/runner.rs)
- [Multiplayer system](../../src/engine/ecs/system/multiplayer_system.rs)
- [UDP channel system](../../src/engine/ecs/system/udp_channel_system.rs)
- [Shared UDP transport](../../src/engine/udp/mod.rs)
- [Native typed endpoint](../../src/engine/udp_channels/endpoint.rs)
- [Separate multiplayer transport](../../src/engine/ecs/system/multiplayer_system/transport.rs)
- [Terminal UDP transport](../../src/engine/user_input/udp_text/mod.rs)
- [Eye tracking UDP receivers](../../src/engine/ecs/system/xr_eye_tracking_system.rs)
- [Run the multiplayer example](multiplayer.md)
- [Author typed channels](typed-udp-channels.md)
- [Branch acceptance and follow-ups](../task/multiplayer-branch-acceptance-and-followups.md)
