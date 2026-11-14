# Typed telemetry and MMS-controlled application

Discussion draft, 2026-11-13. All APIs below are hypothetical pseudocode;
none of the schema, stream, binding, or channel methods shown are implemented.
The multiplayer rename does not change runtime behavior.

## Where the current abstraction makes decisions

`MultiplayerSession` owns discovery/handshake, peer identity, roster, and app
messages. `MultiplayerSync` additionally chooses a parent transform, publishes
its complete world pose, applies received translation/rotation/scale, and
turns off the ancestor Velocity for host-owned entities on clients. These
choices are currently coupled in `MultiplayerSystem::tick` and
`disable_local_simulation`.

The useful separation is transport → validated typed messages → MMS policy →
explicit scene writes. Receiving telemetry should not itself change a transform
or simulation. A multiplayer session can supply peer identities and relay rules
on top of transport without requiring transform replication.

## Agreed direction: typed channels and local bindings

The typical first-slice flow is imperative publication followed by declarative
receive binding. Also support imperative receive handlers for custom mapping.
The first declarative receive binding
maps an entire transform; imperative MMS handlers immediately support mapping
UDP channels to individual transform channels, with arbitrary script logic.
Declarative translation-only, rotation-only, scale-only and per-axis bindings
are later extensions, not prerequisites for the first slice.

Wire channels identify streams of typed data, not remote ECS components:

```text
wire ChannelId → LocalChannelId → MMS handler and/or explicit local binding
                                           ↓
                              ComponentId + transform target
```

Use a dedicated local channel arena with generational handles. Wire channel
IDs are assigned by the publishing authority and resolved to local handles;
independently allocated slotmap keys are not interchangeable across peers.
Bindings resolve component handles once, avoiding per-sample name/property
lookups. A channel can drive multiple local consumers or have no scene binding.
Removing a component invalidates its binding without destroying the channel.

Registration declares channel name, payload schema/version and coordinate
conventions. Updates carry compact channel identity, sequence/timing metadata
and binary typed payload, without repeating names or dictionary keys. Channel
IDs need generation and session/stream epoch protection against delayed packets
and reuse. Declarations need acknowledged/retried delivery; samples received
before registration must be dropped or buffered within an explicit bound.

Minimum numeric payloads include `f32`, `[f32; 2]`, `[f32; 3]`, `[f32; 4]`,
`[[f32; 4]; 4]`, and a whole-transform TRS record (translation vec3, quaternion
vec4, scale vec3). Also support explicit scalar types, bounded arrays/strings/
bytes and typed records; key-value data is useful where dynamic structure is
needed. JSON is not the default spatial telemetry representation. The binary
codec remains to be selected. Specify byte order, matrix layout, quaternion
ordering, units and local/world space. A matrix and TRS are distinct types:
whole-transform binding initially accepts TRS; matrix decomposition and shear
handling require a separate explicit decision.

## Socket versus channel

A socket owns the local UDP endpoint: bind address/port, operating-system socket,
nonblocking send/receive and bounded I/O buffers. It moves datagrams between
addresses. UDP itself has no named typed channels or registration handshake;
those are features of our protocol layered on top.

A channel is a named, typed stream multiplexed over that socket. It owns its
payload schema and publisher identity, wire registration, sequence tracking and
subscriptions. Several channels share one socket and port, and a channel does
not require its own OS socket or worker thread. A socket can communicate with
several peers; channel lookup must distinguish publishers, so equal names from
different peers do not accidentally merge. A logical publication can fan out
to several destinations, with wire registration tracked for each destination.

```text
UdpSocket bound to 0.0.0.0:7010
  ├─ tracker A / left-hand.pose         → TRS record
  ├─ tracker A / right-hand.translation → vec3
  └─ tracker B / temperature            → f32
```

A transform binding is a third responsibility: it applies a received channel's
values to a local scene target. Neither a socket nor an unbound channel implies
any transform writes or changes to physics ownership.

Conceptual ownership: attached publish/subscribe components hold registrations
on their socket; detaching releases their participation. Multiple local
subscriptions to the same publisher/name/schema can share runtime receive
state. Removing the socket closes I/O and makes dependent channels unavailable;
removing one channel leaves the socket and other channels active.

Sockets are first-class MMS values: a script can create or reference a socket,
store it, pass it to a function, and share it across modules. Socket methods
produce publish/subscribe declaration builders; the attached declarations own
their channel participation. Exact constructor and lookup syntax is provisional.

## MMS type tags define channel payloads

Use first-class type tags imported from the language-owned `"types"` module,
not a separate networking `Types` factory or string-based type names. The
implemented primitive tags include `f32`, `vec2`, `vec3`, `vec4` and `str`.
Channel declarations consume those same identities; import aliases retain the
same meaning. Existing type-only primitive record expressions can describe a
TRS payload structurally:

```ts
import { vec3, vec4 } from "types"

let pose_type = {
    translation: vec3,
    rotation: vec4,
    scale: vec3
}
```

```ts
// Proposed socket methods using the existing type tags above.
// The socket may instead be passed in or obtained from an existing owner.
let udp = UdpSocket.bind("0.0.0.0:7010") {}
udp

let outgoing_pose = udp.publish("left-hand.pose")
    .type(pose_type) {}
outgoing_pose

let incoming_pose = udp.subscribe("left-hand.pose")
    .from(tracker_address)
    .type(pose_type) {}
incoming_pose
```

`udp` is a first-class socket handle, not a new socket per declaration.
`publish(...)` and `subscribe(...)` create channel declarations on that socket;
`.type(...)` accepts language type tags or record types, and `{}` uses the
normal component-expression construction/attachment rules. Reusing or passing
the handle does not rebind the port. Detaching a channel releases that channel;
closing/removing the socket owner invalidates dependent I/O even if handle
values remain. Stale socket handles must report an error rather than resolve
to a replacement socket. Exact creation, reference and explicit-close APIs
remain to be settled.

The publisher announces its schema; a subscriber states the schema it expects.
A subscription can wait before its publisher exists. Compatible registration
makes it ready; an incompatible type is an observable error and delivers no
samples. Registration readiness/loss/errors should be observable in MMS, with
exact event names still to be settled. Declarative bindings wait for readiness.
Script authors normally use channel names and handles, not numeric wire IDs.

A networking codec interprets these types as wire layouts and validates values
at the send/receive boundary. A type tag alone does not change the current MMS
numeric representation. Wire registration must encode a portable schema,
not Rust enum discriminants or process-local type handles. Canonical record
field order makes layout independent of source declaration order.

Quaternion xyzw semantics and coordinate conventions supplement the `vec4`
field type. Dedicated quaternion, matrix and richer transform tags remain
language type-system extensions to discuss; do not pretend they are already
exports. Future tags should also come from `"types"` rather than a parallel
network-only type system.

## A. Imperative receive handlers: fine-grained mapping immediately

```ts
// Proposed socket method declarations; imported type tags already exist.
import { vec3, vec4 } from "types"
let socket = UdpSocket.bind("0.0.0.0:7010") {}
socket
let position = socket.subscribe("left-hand.translation")
    .from(tracker_address).type(vec3) {}
position
let orientation = socket.subscribe("left-hand.rotation")
    .from(tracker_address).type(vec4) {}
orientation
let marker = T { R.sphere() {} }
marker

on(position, "Sample", fn(event) {
    marker.set_translation(event.value, "world")
})
on(orientation, "Sample", fn(event) {
    marker.set_rotation_quat(event.value, "world")
})
```

These are separate network streams applied to separate properties of one local
transform. MMS can instead remap axes, apply an offset, filter by sender, update
another component, or ignore a sample. Typed validation and configured stale
sequence rejection happen before delivery; a callback does not need to decode
bytes or repeatedly resolve its target.

The setters above are proposed partial-write intents. Existing
`update_transform(position)` reads rotation and scale at call time, then queues
a full transform update. Independent handlers can overwrite one another's
queued changes. Partial writes must preserve untouched properties against the
state at intent execution. Establish those semantics in the first slice so
imperative translation/rotation mapping is safe immediately.

## B. Script-authored publication

```ts
// Hypothetical API. Use pose_type from the type-only record above.
let outgoing = socket.publish("left-hand.pose")
    .to(tracker_address)
    .type(pose_type) {}
outgoing
on(send_clock, "Tick", fn(event) {
    outgoing.write(controller.world_trs())
})
```

The publishing channel accepts explicit `.write(value)` calls. Destination
configuration is shown as `.to(...)`; its exact routing API remains provisional.
MMS chooses what to sample, when to publish, and where to send it. Native
transport handles bounded I/O, channel registration, encoding and sequence
metadata. A telemetry source need not be a scene component. A separate vec3
channel could publish only translation using the same model.

Binding a local source to publish automatically is deferred. It would need an
explicit sampling trigger, rate and phase relative to physics/XR tracking.
There is no automatic source sampling in the first slice.

A multiplayer session can expose equivalent typed channels with assigned peer
identity and explicit routing (host, peer, broadcast). Raw socket addresses and
session peer ids remain different concepts.

## C. Declarative receive binding: whole transform first

```ts
// Proposed subscription builder API. Alternative to imperative marker writes.
let incoming = socket.subscribe("left-hand.pose")
    .from(tracker_address)
    .type(pose_type)
    .bind_transform(marker)
    .space("world") {}
incoming
```

The subscription owns its optional local destination. There is no global `Bind`
component or built-in `Bind` table. Socket methods produce channel declarations;
a publishing channel accepts `.write(value)`, and a subscription can declare
how received samples apply locally. Without `.bind_transform(...)`, the
subscription exposes Sample events without performing scene writes.

This explicitly binds translation, quaternion rotation and scale as a single
transform update. MMS owns selection and subscription lifetime; native code can
perform typed direct updates without a script closure per target per frame.
Detaching the subscription releases its receive participation and write
ownership; shared channels and other subscribers remain active. Invalid target
handles cannot redirect updates to a replacement component. Whether target
loss leaves the subscription available for observation or stops it needs a
defined policy.

The initial binding needs defined stale-sample and timeout behavior; holding
the latest accepted value is a possible default. Interpolation, extrapolation
and more elaborate timeout actions are independent future choices, not implied
by whole-transform binding.

Later, property-specific binding methods on subscription builders could
shorten the imperative translation/rotation examples. Their names and exact
syntax remain open; they do not require a separate global binding API. They
would reuse the same channel and partial-write semantics. Interpolated rotation
must use normalized shortest-path quaternion interpolation rather than
component-wise lerp.

## Policy that must stay visible

- Physics ownership is explicit in MMS. Receiving a sample never silently
  disables Velocity. Scripts can choose a visual proxy, a kinematic target, or
  an explicitly disabled local simulation.
- Multiple writers targeting the same local transform property should fail
  binding registration unless an explicit arbitration policy is chosen. Manual writes during an active
  binding also need defined precedence/release behavior. Different transform
  properties can coexist through partial-write intents. A whole-transform binding claims
  all three properties; one source channel may feed several different targets.
- A raw UDP sender address is a routing/filtering key, not authenticated identity.
  Session-assigned identity and authority checks belong above raw transport.
- Schemas validate version, field types, finite vectors, valid quaternions and
  size limits before handlers run. Invalid packets produce bounded diagnostics.
- UDP telemetry can use latest-value delivery with bounded buffers and stale
  rejection. Commands such as kick/spawn need an explicit reliable-delivery
  policy; a typed schema does not provide delivery guarantees.
- Receive work, script callbacks and interpolation buffers remain bounded.
  Coalesce replaceable telemetry by stream/entity/channel when configured;
  do not coalesce commands. Keep the current nonblocking frame-loop approach.
- Define a receive → MMS dispatch → mutation flush point before camera
  publication. Publication needs a separate, explicit sampling phase/rate so
  post-physics and XR-tracking samples have predictable timing.

## First implementation slice

1. Typed binary channels with registration, session-scoped identity, bounded
   receive/send, imperative `.write(value)` and explicit routing/sender selection;
   select the codec separately.
2. Imperative Sample handlers plus execution-time partial transform writes,
   allowing translation-only and rotation-only mappings immediately.
3. Subscription-owned `.bind_transform(target)` for a whole TRS channel, with explicit
   coordinate space, lifecycle, ordering, timeout and write-ownership semantics.

Validate both paths against the same sender: whole-transform declarative
updates, independent imperative translation/rotation updates without clobbering,
multiple consumers, target deletion, channel reuse, stale packets and samples
arriving before channel registration. Preserve bounded nonblocking frame work
and direct/batched application paths.

No source implementation changes are part of this draft. Replacing
MultiplayerSync and migrating existing examples remain separate implementation
work. Future declarative property bindings can reuse these primitives.

## Phase 2 / v2 investigation: network audio and voice chat

After the typed-channel basics, whole-transform receive binding, and bindings
for individual transform properties (translation, rotation and scale) are
working, investigate streaming audio channels and voice chat. This is future
scope, not a requirement for the initial telemetry implementation.

Use `AudioStream` as the proposed name for a continuously fed audio source,
alongside AudioClip, AudioInput and AudioOscillator. AudioSource remains the
umbrella term for sound-producing graph nodes. AudioStream is independent of
UDP: custom Rust producers, decoders and other transports can feed it.

Separate three responsibilities:

- AudioStream consumes buffered PCM frames and participates in the existing
  audio graph, effects and output routing like other sources.
- A Rust writer handle lets arbitrary producer code enqueue frames through a
  bounded buffer, with explicit format, overflow and lifetime semantics.
- An audio subscription binding owns the network receive/decode/timing path
  that feeds a chosen AudioStream. Encoded network frames do not go directly
  into the PCM source without decoding and format validation.

```ts
// Phase 2 sketch only; audio_frame_type and these APIs are not implemented.
let voice = AudioStream.channels(1).sample_rate(48000) {}
voice

let incoming_voice = some_socket.subscribe("voice")
    .from(peer_address)
    .type(audio_frame_type)
    .bind_audio(voice) {}
incoming_voice
```

The receiver explicitly chooses its playback target, following the same
subscription-owned binding model as transforms. There is no global Bind API.
The example assumes attachment beneath an appropriate AudioOutput for audible
playback; merely receiving a channel does not make it audible. Positional voice
could combine an audio channel with separate transform channels for the
speaker's location, subject to the audio system's spatial-playback support.

Producers push PCM frames; the audio callback pulls buffered frames without
waiting for networking, decoding or arbitrary producer code. Define underrun
behavior, bounded latency/overflow policy, producer ownership and shutdown,
channel layout, sample-rate conversion and format changes. Callback work must
avoid blocking locks and allocation. A UdpAudioSource convenience component
could be considered later, but should build on AudioStream and subscription
binding rather than introduce a separate UDP-specific playback implementation.

Investigate microphone capture, audio-frame/codec negotiation, timestamps and
clock drift, bounded jitter buffering, loss handling, playback scheduling,
mute/push-to-talk, and per-peer lifecycle. Audio frames need their own delivery
and buffering policy; do not assume telemetry latest-value coalescing or
transform interpolation is appropriate for audio. Keep capture, decoding and
playback work off blocking scene-update paths. Codec and audio-source binding
APIs should be designed after the basic channel and transform paths are proven.

Open design choices: schema declaration syntax; channels as first-class
observables versus scoped events; socket creation/ownership and destination
routing syntax; codec selection; and explicit local/world conversion behavior.
