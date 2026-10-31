# Portable UDP and LAN keyboard relay

Status: proposal, 2026-10-31. Components, signals, APIs, files, and examples named
below remain proposed. A narrower text/edit-only
[UDP terminal prototype](../../docs/how_to/mitten-droid-shell.md) now supplies
normal TextInput events with ordering, acknowledgments, retries, and duplicate
suppression; it does not implement the generic socket or gameplay-key relay
components below. This is the transport/input slice of the
[MMS LAN development workflow](mms-lan-development.md).

Build small runnable Rust and MMS examples alongside the APIs before adding the
complete desktop dashboard. The immediate target is typing into a focused
headset field and receiving ordinary scene keyboard signals over the LAN.

## Proposed scope

Add two portable components and two systems:

| Rust / MMS component | System | Responsibility |
| --- | --- | --- |
| `UdpSocketComponent` / `UdpSocket` | `UdpSystem` | Bind a bidirectional socket, send datagrams, and deliver typed receive/completion/error signals. |
| `KeyboardRelayComponent` / `KeyboardRelay` | `KeyboardRelaySystem` | Forward a selected local keyboard or receive a remote one, with ordered delivery, acknowledgments, input ownership, and held-key cleanup. |

The relay has sender and receiver roles. Both use the same codec and protocol
state machine. Its receiver supplies a `RemoteKeyboardProvider` implementing
the existing `HidProvider` trait. Capture/routing and codec helpers are focused
modules, not additional ECS systems or Android-specific input implementations.

UDP is a useful general engine API independently of the keyboard relay. Keep
script launch, terminal evaluation, project listings, and asset transfer on the
HTTP control path initially. Receiving a UDP packet never implicitly evaluates
MMS. Full script payloads do not need to fit inside keyboard datagrams.

Raw UDP does not preserve delivery or order. The keyboard protocol must handle
those properties explicitly; held-key snapshots cannot recover missing typed
characters or deletions. See the [Rust UDP contract](https://doc.rust-lang.org/std/net/struct.UdpSocket.html)
and [UDP reliability guidance](https://www.rfc-editor.org/rfc/rfc8085.html#section-3.3).

## Existing integration points

| Existing code | Reuse / required change |
| --- | --- |
| [HTTP components](../../src/engine/ecs/component/http_server.rs) and [systems](../../src/engine/ecs/system/http_server_system.rs) | Follow component registration, queued intents, scoped events, and removal conventions. UDP needs bounded queues and persistent workers; do not copy thread-per-request behavior. |
| [Portable input](../../src/engine/user_input.rs) | Reuse engine-owned keys, `KeyboardInputRecord`, and `TextInputFrameEvent`; add a normalized ordered capture representation and explicit routing. |
| [HID providers](../../src/engine/user_input/providers.rs) | Mount the remote keyboard as its own provider with a stable ID; preserve local/remote held-state separation. |
| [Keyboard signals](../../src/engine/ecs/system/keyboard_input_system.rs) | Preserve `KeyDown`, `KeyPress`, and `KeyUp` semantics. Gameplay signals are suppressed while text input is focused. |
| [Text input](../../src/engine/ecs/system/text_input_system.rs) | Feed the focused field through normal text/edit events; preserve `TextInputChanged` and focus notifications. |
| [Android scene runtime](../src/scene_runtime.rs) | Replace the current neutral-input update call with the host/provider update path when a relay is mounted. XR tracking remains independent. |

Subscribing to existing `KeyDown`/`KeyPress` events alone is insufficient. Their
payloads contain `code` and `key`, without committed text, and capture suppresses
them during text editing. `InputState` currently stores keyboard and text edits
in separate vectors; it is not a complete cross-stream ordering contract.

## Generic UDP component and Rust API

Proposed construction and runtime surface:

| Surface | Contract |
| --- | --- |
| `UdpSocketComponent::bind(address)` | Numeric IPv4/IPv6 socket address; port `0` selects an ephemeral port. Bind asynchronously on initialization. |
| `.with_enabled(bool)` | Defaults to true. Disabled components own no live socket. |
| `.with_max_datagram_bytes(limit)` | Default 1,200 bytes for the complete UDP payload, including application envelopes; initial hard cap 65,507 bytes for both address families. |
| `IntentValue::RegisterUdpSocket { component_id }` | Create the runtime or report a bind/configuration failure. Registration is idempotent for unchanged configuration. |
| `IntentValue::UdpSendTo { component_id, send_id, remote_addr, payload }` | Queue one owned binary datagram; validate size and destination before accepting it. |
| `IntentValue::UdpSetEnabled { component_id, enabled }` | Disable closes the socket and invalidates pending work; enable creates a new runtime generation. |
| `UdpSystem::remove_component(component_id)` | Close, stop/wake the worker, and discard pending deliveries for that runtime. |

Use a typed `UdpSendId` for Rust correlation. Reserve it before queueing a send;
the MMS methods below expose it as a decimal string to avoid integer precision
loss. Reject queue admission failures observably. A missing, disabled, removed,
or failed socket must not silently consume a send request.
Invalid component references fail at the authoring/admission boundary. Report
runtime send failures through `UdpError` while the component exists. Removing a
component is an explicit cancellation boundary for already-admitted sends;
callbacks are not promised after their owning component has been deleted.

Start with at most eight live sockets and raw send/receive queues capped at
256 datagrams or 256 KiB per socket, whichever is reached first. Bound aggregate
worker and queue memory as well. Queue limits are transport configuration;
relay limits below provide additional bounds on retained/reordered input.

Rust sketch, using proposed types:

```rust
let socket = world.add_component(
    UdpSocketComponent::bind("0.0.0.0:0")
        .with_max_datagram_bytes(1200),
);
world.init_component_tree(socket, &mut queue);

systems.rx.add_handler_closure(
    SignalKind::UdpDatagram,
    socket,
    move |_world, _emit, signal| {
        if let Some(EventSignal::UdpDatagram(packet)) = signal.event.as_ref() {
            // Typed SocketAddr, owned/shared bytes, scoped to this socket.
            inspect(packet.remote_addr, packet.payload.as_ref());
        }
    },
);
```

This is an API sketch, not a standalone compilable example. The real Rust echo
example must also register handlers before driving updates, wait for readiness,
issue sends through intents, and shut down cleanly.

### UDP event signals

Add matching `EventSignal` variants and `SignalKind` entries. Payload structures
live in a focused network-signal module. Events are scoped to the socket.

| Signal | Rust payload / MMS fields |
| --- | --- |
| `UdpReady` | `{ local_addr }`: actual bound address, including the chosen port. |
| `UdpDatagram` | `{ remote_addr, payload, size_bytes }`: sender address and complete bytes. MMS additionally exposes `body_text`, null for invalid UTF-8. |
| `UdpSent` | `{ send_id, remote_addr, size_bytes }`: the local OS accepted this datagram; it does not acknowledge peer receipt. |
| `UdpError` | `{ send_id?, phase, code, message, remote_addr? }`: phases include configuration, bind, send, receive, and queue admission. Stable codes identify size, address, and queue errors. |

Rust uses `SocketAddr` and shared immutable byte storage such as `Arc<[u8]>`.
MMS uses address strings and arrays of integers in `0..255`. Validate every byte
without truncation/coercion. A zero-length datagram is valid. Materialize text
and MMS arrays only for consumers that request those views; keep direct typed
delivery available for the relay. Define and test the MMS conversion cost.

Expose `socket.stats()` with bounded counters for received/sent packets, bytes,
oversize drops, queue drops, and coalesced errors. Do not emit an error for every
packet in an overload flood. No ordering, retry, evaluation, JSON parsing, or
keyboard injection belongs in this general socket component.

### MMS bindings and minimal echo example

| MMS surface | Behavior |
| --- | --- |
| `UdpSocket.bind(address) {}` | Materialize and initialize a socket component. |
| `.enabled(bool)`, `.max_datagram_bytes(limit)` | Constructor/builder configuration. |
| `socket.send_text(remote_addr, text)` | UTF-8 encode, queue one datagram, return its send ID. Count encoded bytes. |
| `socket.send_to(remote_addr, payload)` | Queue one binary datagram and return its send ID. |
| `socket.set_enabled(bool)` | Queue a lifecycle change. |
| `socket.stats()` | Read a bounded diagnostics snapshot. |
| `on(socket, "UdpReady" / "UdpDatagram" / "UdpSent" / "UdpError", callback)` | Normal scoped MMS signal subscriptions with the fields above. |

Proposed MMS echo receiver:

```mms
let socket = UdpSocket.bind("0.0.0.0:7001").max_datagram_bytes(1200) {}
socket

on(socket, "UdpDatagram", fn(packet) {
    socket.send_to(packet.remote_addr, packet.payload)
})
```

The sender binds `0.0.0.0:0`, waits for `UdpReady`, then sends to the headset's
LAN address and port. Echo replies use the packet's observed sender address and
port. Numeric unicast addresses are sufficient initially; hostname resolution,
discovery, broadcast, and multicast are separate extensions.

## Keyboard relay component and signals

Use one configured sender or receiver role per component. Multiple relays may
exist, but the first development host accepts one active PC controller and the
sender captures one selected local keyboard provider.

| Rust / MMS surface | Contract |
| --- | --- |
| `KeyboardRelayComponent::sender(socket, peer_addr)` / `KeyboardRelay.sender(socket, peer_addr) {}` | Capture and forward keyboard/text input through the referenced UDP socket. |
| `KeyboardRelayComponent::receiver(socket)` / `KeyboardRelay.receiver(socket) {}` | Decode the keyboard protocol and supply a remote HID provider. |
| `.with_enabled(false)` / `.enabled(false)` | Default disabled: declaring a component does not start capture or authorize injection. |
| `relay.open(lease)` | Install a runtime lease obtained through the authenticated control connection; validate role, socket, target, and limits. |
| `relay.set_enabled(bool)` | Explicitly enter/leave forwarding mode. Enabling requires a valid lease and ready socket. |
| `relay.close()` | End this input epoch, release held keys, and discard unconsumed operations. |
| `relay.stats()` | Read state, last applied sequence, pending bytes/events, RTT, retries, and resets. |

`open` and `close` are queued operations; leases/credentials are runtime state
and must not be serialized into saved MMS component ASTs. One relay claims a
socket's input consumer while open; a second claim fails explicitly. Ordinary
unclaimed sockets deliver `UdpDatagram` as above. Use a separate socket for a
simultaneous general-purpose MMS service; relay packet processing must not wait
for authored MMS callbacks or inject each packet through two routes.

Add internal `RegisterKeyboardRelay`, `KeyboardRelayOpen`,
`KeyboardRelaySetEnabled`, and `KeyboardRelayClose` intents, all scoped to a
component. Removing the relay or its referenced socket revokes the lease and
releases its provider state.

| New signal | Fields / meaning |
| --- | --- |
| `KeyboardRelayStateChanged` | `{ state, reason, input_epoch }`; states include idle, opening, active, paused, and faulted. |
| `KeyboardRelayProgress` | `{ input_epoch, applied_through, pending_operations, pending_bytes, rtt_ms }`; coalesced at a limited UI cadence. |
| `KeyboardRelayError` | `{ phase, code, message, input_epoch, applied_through }`; malformed protocol, expired lease, target change, exhausted retry window, or overload. |

The received keyboard still generates the existing `KeyDown`, `KeyPress`,
`KeyUp`, `TextInputChanged`, and `TextInputFocusChanged` signals through ordinary
engine input. Do not add parallel `NetworkKeyDown` gameplay semantics. Relay
signals report transport/session state, rather than bypassing focus routing.

MMS receiver configuration sketch:

```mms
let socket = UdpSocket.bind("0.0.0.0:7001") {}
socket
let keyboard = KeyboardRelay.receiver(socket).enabled(false) {}
keyboard

on(keyboard, "KeyboardRelayStateChanged", fn(state) {
    // Update the example's connection/forwarding indicator.
})
```

The control handshake later supplies `keyboard.open(lease)` and explicit enable.
The sender uses the same pattern with `KeyboardRelay.sender(socket, address)`.
The automatic Android listener and project browser remain available while
forwarding is disabled; listener startup does not capture the PC keyboard.

## Capture, text, and focus ownership

Create a portable ordered operation type, provisionally `KeyboardRelayOperation`:

- `Key`: physical `KeyCode`, typed logical key (named or character), and
  `KeyboardTransition::{Down, Press, Up}`. Preserve case/layout and modifiers.
- `Text`: a committed UTF-8 string or one supported text-edit operation such as
  Backspace, DeleteForward, MoveCaretLeft, or MoveCaretRight.

Normalize each local input event once into this ordered stream before creating
the existing per-frame keyboard/text vectors. An initial key press produces
Down then Press; OS repeats produce Press; releases produce Up. Committed text
and editing operations retain their position relative to those key operations.
Use explicit portable key-name mappings, not debug formatting or platform
scancodes. Unsupported physical keys are reported; they must not all collapse
into one held `Unidentified` key. Independent committed text can still be sent.

The receiver updates held/gameplay state from Key operations and inserts text
only from explicit Text operations. It must not regenerate a second character
or Backspace from the logical key. Extract a shared input-state builder for
this path instead of feeding already-normalized edits back through the current
`UserInput::handle_event` derivation a second time. Rust callers and MMS relay
components use the same normalization and receiver implementation.

Committed text is required for Unicode, paste, and input methods. The current
Winit adapter handles keyboard events but does not handle `Ime::Commit`, and
the current key-text path filters control characters. Add committed-text
handling and suppress any platform duplicate delivery explicitly. Bare Enter
or Tab key delivery does not establish newline insertion, focused submission,
or focus traversal. Define those through normal input routing: the first
multiline receiver example must demonstrate Enter inserting a newline; a
terminal can initially use its Run button until focused submission is wired.
Preserve the existing supported editing keys and document further editor-key
extensions separately.

While forwarding, the desktop keyboard has one destination. Route capture
before local gameplay/text consumers; consume the selected keyboard stream
exclusively, leaving the client's pointer/browser controls usable. Provide a
visible exit control and a reserved exit shortcut handled before forwarding.
Those shortcut events never reach the headset. Ordinary terminal editing and
optional draft mirroring remain separate modes. Exclude remote providers from
capture so two peers cannot echo keyboard input back and forth.

When keyboard ownership changes, release held state in the previous destination
and suppress already-held keys until release and a fresh press. Entering
forwarding must not leave local movement stuck; exiting must not introduce a
fresh local press from a key still held for the headset. Keep the physical
source ledger separate from the destination's delivered state.

The receiver owns text focus. Advertise a target/focus generation with the
input lease; never send raw ECS handles as network targets. Check it when input
is consumed, not only when a datagram arrives. A focus change invalidates queued
input for the previous target, clears held state, and requires a new input epoch.
Settle focus changes at operation boundaries so a delayed text operation cannot
land in the newly focused field. Reconcile this explicitly with the current
system order: gameplay keys run early in the update, while text edits run after
gesture-driven focus changes. Do not validate once early and later deliver
those edits to a different field. Report the consumed prefix on cancellation;
do not automatically replay remaining edits into the new target.

## Wire protocol and delivery rules

Keep the protocol codec/state machine independent of socket I/O, MMS, and ECS.
The initial codec can use bounded UTF-8 JSON for inspectable examples. The wire
payload is versioned and validated before it becomes an input operation.
All message types, including acknowledgments and resets, validate the lease
capability and generations; an unauthenticated ACK cannot discard pending edits.

An authenticated HTTP handshake grants a short-lived input lease bound to the
host/controller identity, active scene generation, target generation, and a
fresh input epoch. The receiver validates the lease capability before accepting
input, replies to the authenticated sender's observed UDP address/port, and
pins that peer for the epoch. A peer-address change requires renewal. Do not
treat a packet's claimed IP or UDP `connect()` as authentication. Plaintext
capabilities assume the same trusted development LAN as the parent proposal;
they do not provide traffic confidentiality.

| Message | Required content / behavior |
| --- | --- |
| Open / ready | Protocol version, lease/controller and generation identities, capability, negotiated limits, and neutral starting state. Data is rejected until ready. |
| Input | Identity envelope, first operation sequence, and a nonempty ordered operation array. Retries reuse the same sequences and contents. |
| Ack | Highest contiguous applied operation sequence and remaining receive capacity, bound to the same epoch. |
| Heartbeat / state | Current epoch, acknowledgment cursor, and held-key snapshot tagged with the operation sequence it describes. |
| Reset / close | End the epoch with reason and last applied cursor. Stop accepting further input under that epoch. |

Sequence operations rather than packets, so repacking a retry cannot change an
edit's identity. Bound sequence values to integers exactly representable by MMS
(initially `1..2^31-1`) and renew the epoch before wrap. Within the retained
window, duplicate sequences with different contents are a protocol error.
Retain the applied high-water mark for the epoch: older already-applied
sequences are ignored/re-acknowledged even after their contents are evicted.

The sender retains unacknowledged operations in a bounded window, including
the final key release or character after typing stops. The receiver buffers
only a bounded amount of reordered input and consumes a contiguous prefix.
Never skip a missing edit and continue: losing Backspace changes the meaning
of later text. Ignore duplicate queued/applied operations; retransmit and
re-acknowledge when an acknowledgment was lost.

Ack means the engine consumed that prefix through its normal input/command
path. It does not mean the GPU displayed it, a gameplay callback succeeded, or
MMS submitted from a terminal finished. Keep separate queued and applied cursors
so a duplicate arriving while work awaits an update is not injected twice.
Do not acknowledge only because the socket worker received the bytes.

Retries use an RTT-aware timeout with exponential backoff and finite age and
retry limits. Heartbeats, acknowledgments, and retries are paced and included
in the same traffic budget; lower the send rate on sustained loss. Respect the
receiver's advertised capacity. The [UDP usage guidelines](https://www.rfc-editor.org/rfc/rfc8085.html#section-3.1)
motivate this pacing; merely using a private LAN does not make unlimited retry
traffic useful. No busy retransmission loops or unlimited reliable queues.

A held-key snapshot can reconcile releases only at its corresponding applied
sequence. It never synthesizes text, Press, or a fresh Down across epochs. A
lost key-down/character still needs the ordered stream; a lost final release
needs retransmission, with timeout cleanup as the final recovery path.

On timeout, exhausted window, queue overflow, focus loss/change, forwarding-mode
exit, controller replacement, scene replacement, suspend, or socket/relay
removal: revoke the input epoch, clear pending edits, and release that provider's
held keys. Preserve other providers' state. Show the last applied cursor and a
visible interruption/error rather than silently dropping text or replaying it
after reconnect. Start the next epoch neutral and require fresh presses; a key
already held on the PC must not turn into a new press in the replacement scene.

### Limits and large text

These are starting defaults for experiments, not measured Focus 3 results:

| Limit | Initial proposal |
| --- | --- |
| UDP payload | 1,200 encoded bytes including the complete envelope. |
| Pending / reordered operations | At most 256 operations and 64 KiB per direction/relay, whichever is reached first; also cap total across relays. |
| Engine consumption | At most 64 operations per update, preserving order; measure and tune the budget. |
| Heartbeat / idle timeout | 100 ms / 1 second; negotiated and measured on the real LAN. |
| Traffic | At most 100 datagrams/s and 64 KiB/s per relay, including protocol traffic, with backoff under loss. |

Budget admission happens before buffering an operation. Avoid evicting old text
to make room for new text. Small pastes may be split at UTF-8 character boundaries
into consecutive operations; size each complete encoded datagram, including JSON
escaping. The first prototype rejects a paste exceeding its negotiated pending
limit with a visible error before sending a partial paste. Larger paste support
can use the reliable control channel with the same sequence/target barrier; do
not add an unbounded UDP fragmentation/reassembly protocol for clipboard data.

The 1,200-byte limit is an application starting point, not a guarantee about
every network path. Avoid fragmentation and measure the actual router/headset
path, following [message-size guidance](https://www.rfc-editor.org/rfc/rfc8085.html#section-3.2).

## Systems, update order, and lifetime

`UdpSystem` owns socket runtimes, generation tags, bounded byte/packet queues,
diagnostics, and worker shutdown. Start with a bounded number of persistent
workers, at most one per live socket; never create a thread per datagram. Use
reusable receive storage large enough to detect oversized ordinary datagrams
(for example 65,536 bytes), discard oversize packets, and handle platform
truncation errors. The [Rust receive contract](https://doc.rust-lang.org/std/net/struct.UdpSocket.html#method.recv_from)
allows excess bytes to be discarded when the buffer is too small; a truncated
prefix must never become a valid `UdpDatagram`. Use wakeups/bounded waits rather
than blocking socket work or spinning on the XR thread. Wake retiring workers
immediately and keep potentially blocking joins outside the XR update path.

`KeyboardRelaySystem` owns relay registration, exclusive capture routing,
lease/target checks, and input-provider attachment. The pure relay runtime owns
encoding, sequence windows, timers, ACKs, and held-key reconciliation. Its
network/timer work can share the socket worker; it must progress independently
of scene MMS callbacks and XR rendering. Worker threads never mutate World,
TextInput focus, or XR state.

At a defined host input boundary:

1. Process lifecycle/reset and target changes; revoke obsolete input first.
2. Drain bounded socket/relay work and admit only valid contiguous operations.
3. Capture/route local input and poll mounted remote HID providers exactly once.
4. Run existing input, text, signal, and command processing with ownership checks
   at the actual consumption points; cancel an invalidated remainder.
5. Publish the applied cursor and flush bounded sends/completions.

Provider mounting is an explicit Universe/host integration task: `SystemWorld`
does not currently own `Universe::input_providers`. Add a host input driver that
mounts/unmounts receiver providers and services relay work before provider polling.
Retain the meaning of `Universe::update(dt, &snapshot)` for explicit supplied
snapshots; use the provider driver in the Android host and normal examples.

Publish release transitions through an active provider before detaching it.
Today `KeyboardInputSystem` drops bookkeeping for absent providers; relying on
that alone can omit KeyUp for scripts tracking their own held state. If immediate
removal requires a new explicit per-provider release hook, test that hook and
keep it in the shared input layer.

Tag queued transport work with component/runtime generations so old internal
packets/completions cannot reach a replacement runtime. Generic UDP cannot
identify a delayed wire packet arriving at a rebound port. The relay's identity,
controller epoch, scene generation, and focus generation reject those packets
across their respective lifetime changes. Once a reset happens, packets cannot
extend the old lease. While suspended, continue status/expiry service and reject
scene input; never replay a typing backlog on resume. Shutdown revokes input,
wakes/stops workers, then releases sockets.

The default Android listener and browser are app-owned. Its keyboard receiver
can be app-owned too, but its provider/lease targets only the active scene. A
script launch cannot destroy the PC connection, and it cannot inherit the old
scene's held keys. No new renderer features or intermediate render targets are
needed for this work.

## Proposed file boundaries and bindings

Keep portable implementation in the engine, with Android limited to startup,
permission, lifecycle, and host-driver integration:

| Proposed module | Responsibility |
| --- | --- |
| `src/engine/ecs/component/udp_socket.rs` | Socket configuration and registration/serialization. |
| `src/engine/ecs/component/keyboard_relay.rs` | Sender/receiver configuration and runtime control intents. |
| `src/engine/ecs/system/udp_system.rs` | Socket runtime/worker ownership and bounded delivery. |
| `src/engine/ecs/system/keyboard_relay_system.rs` | ECS registration, capture/routing, and receiver integration. |
| `src/engine/network/udp.rs` | Typed datagrams, send IDs, queue limits, stats, and runtime handles. |
| `src/engine/network/keyboard_relay/` | Codec, protocol windows/timers, leases, and deterministic protocol tests. |
| `src/engine/user_input/keyboard_stream.rs` | Ordered normalization and shared input-state application. |
| `src/engine/user_input/remote_keyboard.rs` | Provider adapter and explicit neutral/release behavior. |
| `src/engine/ecs/signals/network.rs` | Focused UDP/relay event and intent payload definitions. |
| `src/scripting/network_runtime_config.rs` and focused network binding modules | RuntimeSpec declarations, construction, methods, signal names, and event-table conversion. |

Wire up component/system module exports, registration/removal, intent dispatch,
signal kinds, `on(...)` lookup, event payload conversion, RuntimeSpec signatures,
and MMS serialization round-trips. Completing a legacy registry entry alone
does not establish that the retained RuntimeSpec path supports the API.

Several integration files are already oversized, including `system_world.rs`,
the scripting registries, `runtime_config.rs`, and `host.rs`. Extract the affected
responsibility before expanding those files, following [AGENTS.md](../../AGENTS.md).
Keep new modules focused and normally below 1,400 lines. Share direct/batched
paths and byte storage; do not add per-draw work, repeated socket creation, or
extra MMS serialization just to dispatch from one system to another.

## Examples and acceptance slices

The example names below are proposed. Each slice should produce runnable
examples with build/run instructions, rather than UI mockups referencing APIs
that do not exist:

| Proposed example | Demonstrates |
| --- | --- |
| `examples/udp-echo.rs` | Rust component registration, ephemeral ports, binary send/reply, scoped signals, errors, and cleanup without a desktop window. |
| `examples/udp-echo.mms` plus launcher | The same UDP facility through MMS `on(...)` and methods; accept bind/peer settings rather than baking in the headset address. |
| `examples/lan-keyboard-client.mms` plus launcher | Connection and forwarding controls, exclusive capture, relay status, and a visible way to stop forwarding. Runs inside desktop Mittens. |
| `android/examples/lan-keyboard-server.mms` | Focusable editable field, visible text/caret, and a scene key-status display using supported custom UI materials; mounted by the Android host. |
| Rust relay integration harness | Same receiver path and signals with deterministic loss/duplication/reordering, without requiring a headset. |

1. Implement generic UDP and its Rust/MMS echo examples. Test port `0`, bind
   conflict, invalid addresses/bytes, UTF-8 and binary/empty packets, oversize
   receive/send, queue limits, removal/re-enable, and stale completions. Use
   allocated ephemeral ports for tests instead of fixed ports shared by CI.
2. Add ordered keyboard normalization, routing, and a local provider-loopback
   harness. Verify Shift plus characters, changed logical values on release,
   Down/Press/Up and OS repeat order, Unicode, committed input, paste/deletion,
   caret movement, newline behavior, no duplicated edits, and exclusive routing.
   Enter and exit capture with keys already held. Verify local and remote
   keyboards can hold the same key independently.
3. Implement relay leases, windows, ACKs, timers, and limits with a simulated
   clock/network. Drop the first/final character, Backspace, modifier release,
   and ACK; duplicate and reorder operations; exhaust queues; change target or
   scene with data in flight. Assert exact text and signal counts, bounded
   memory/traffic, explicit interruption on failure, and no stale reactivation.
4. Connect the real UDP relay and keyboard client/server examples. Add Android
   `INTERNET` permission and default development-host lifecycle integration.
   Verify PC-to-Focus-3 typing, game keys, focus changes, stop-forwarding,
   disconnect, packet interruption, suspend/resume, repeated launch, and shutdown.
   Test that key releases reach script subscribers and other providers stay held.
5. Reuse these pieces in `shell-client.mms` / `shell-server.mms`. Keep the
   terminal, file browser, project transfer, and script-launch ownership defined
   by the parent proposal; their code delivery stays on the reliable control
   path. Optional draft snapshots may use UDP later with their own revisions.

Validate both Rust compilation and real MMS evaluation through RuntimeSpec;
`cargo check --example` alone cannot validate an included `.mms` file. On Focus 3,
measure input-to-visible-text latency and frame/update cost while idle, typing,
pasting, transferring assets, and recovering from loss. Compare with the HTTP
input prototype if present; do not claim UDP improves latency before measuring.
