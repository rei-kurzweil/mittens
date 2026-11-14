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

Support both imperative and declarative binding. The first declarative binding
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

## A. Imperative receive handlers: fine-grained mapping immediately

```ts
// Hypothetical MMS API; registration and channel lookup syntax are provisional.
let socket = UdpSocket.bind("0.0.0.0:7010") {}
socket
let position = socket.channel(tracker_address, "left-hand.translation", Types.vec3())
let orientation = socket.channel(tracker_address, "left-hand.rotation", Types.quat_xyzw())
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
// Hypothetical API. Declare the schema once; publish values repeatedly.
let outgoing = socket.publish_channel("left-hand.pose", Types.transform_trs())
on(send_clock, "Tick", fn(event) {
    outgoing.send(tracker_address, controller.world_trs())
})
```

MMS chooses what to sample, when to publish, and where to send it. Native
transport handles bounded I/O, channel registration, encoding and sequence
metadata. A telemetry source need not be a scene component. A separate vec3
channel could publish only translation using the same model.

A multiplayer session can expose equivalent typed channels with assigned peer
identity and explicit routing (host, peer, broadcast). Raw socket addresses and
session peer ids remain different concepts.

## C. Declarative binding: whole transform first

```ts
// Hypothetical API. This is an alternative to imperative writes to marker.
let pose = socket.channel(tracker_address, "left-hand.pose", Types.transform_trs())
let binding = Bind.channel(pose)
    .to_transform(marker)
    .space("world") {}
binding
```

This explicitly binds translation, quaternion rotation and scale as a single
transform update. Receiving data without a binding performs no scene writes.
MMS owns selection and binding lifetime; native code can perform typed direct
updates without a script closure per target per frame. Removing the binding
releases its subscription and write ownership. Invalid target handles cannot
redirect updates to a replacement component.

The initial binding needs defined stale-sample and timeout behavior; holding
the latest accepted value is a possible default. Interpolation, extrapolation
and more elaborate timeout actions are independent future choices, not implied
by whole-transform binding.

Later, declarative property binding could shorten the imperative examples:

```ts
// Future convenience, outside the first declarative slice.
Bind.channel(position).to(marker, "translation").space("world") {}
Bind.channel(orientation).to(marker, "rotation_quat").space("world") {}
```

These would reuse the same channel and partial-write semantics. Interpolated
rotation must use normalized shortest-path quaternion interpolation rather
than component-wise lerp.

## Policy that must stay visible

- Physics ownership is explicit in MMS. Receiving a sample never silently
  disables Velocity. Scripts can choose a visual proxy, a kinematic target, or
  an explicitly disabled local simulation.
- Multiple writers targeting the same local transform property should fail
  binding registration unless
  an explicit arbitration policy is chosen. Manual writes during an active
  binding also need defined precedence/release behavior. Different transform properties
  can coexist through partial-write intents. A whole-transform binding claims
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
   receive/send and explicit sender selection; select the codec separately.
2. Imperative Sample handlers plus execution-time partial transform writes,
   allowing translation-only and rotation-only mappings immediately.
3. Declarative binding of a whole TRS channel to a local transform, with explicit
   coordinate space, lifecycle, ordering, timeout and write-ownership semantics.

Validate both paths against the same sender: whole-transform declarative
updates, independent imperative translation/rotation updates without clobbering,
multiple consumers, target deletion, channel reuse, stale packets and samples
arriving before channel registration. Preserve bounded nonblocking frame work
and direct/batched application paths.

No source implementation changes are part of this draft. Replacing
MultiplayerSync and migrating existing examples remain separate implementation
work. Future declarative property bindings can reuse these primitives.

Open design choices: schema declaration syntax; channels as first-class
observables versus scoped events; binding components versus an RX operator;
codec selection; and explicit local/world conversion behavior.
