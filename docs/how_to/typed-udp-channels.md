# Typed UDP channels in MMS

Sockets are first-class component handles: store them, pass them to functions,
and share one bound port across many channels. Channels use ordinary methods
and type identities imported from `"types"`; there is no factory-body syntax or
global Bind component/table.

```mms parse-only
import { vec3, vec4, f32 } from "types"
let pose_type = {
    translation: vec3,
    rotation_quat_xyzw: vec4,
    scale: vec3
}
let socket = UdpSocket.bind("0.0.0.0:7011") {}
socket
let outgoing = socket.publish("left-hand.pose")
    .to("100.64.0.2:7012")
    .type(pose_type)
outgoing
```

Creating a socket expression registers a detached handle; emitting/attaching it
activates I/O through the normal component lifecycle. Socket.ready() reports
activation; local_addr() returns its actual bound address (including an assigned
ephemeral port), or null before activation. Bind errors emit UdpState.
publish/subscribe create
detached channel handles. Configure them before emission/attachment. Their
methods return the same handle, not another channel. Constructor braces belong
to UdpSocket; channel methods do not take component bodies.

## Imperative publication

```mms parse-only
if outgoing.ready() { outgoing.write(controller.world_trs()) }
```

The publishing schema must match the value exactly. write validates and queues
the latest value for the next socket-I/O phase. Multiple writes to one channel
before that phase coalesce; this is telemetry, not a reliable command queue.
Writing before compatible registration completes is an error. MMS chooses the
sampling event/rate; the avatar demo publishes at most 20 times per simulated
second from FrameTick. The native socket stays nonblocking and has bounded work.

## Transform bindings

```mms parse-only
let incoming = socket.subscribe("left-hand.pose")
    .from("100.64.0.1:7011")
    .type(pose_type)
incoming.bind_transform(hand, {
    name = "remote_hand_pose"
    space = "world"
})
```

The binding call returns the subscription handle. As a standalone MMS expression
statement, it emits/attaches that handle and activates the subscription. If you
instead store its return value in a variable, emit/attach that variable afterward.
This same rule applies to the `bind_morph` example below.

The second argument is an ordinary options table, not an executable block.
name labels the local subscription component; the wire channel name remains
left-hand.pose. Bindings default to local space. Set `space = "world"` on
publication or subscription when transferring a world pose. The bound value supplies the
whole translation/quaternion/scale transform; the quaternion is normalized.
The TRS schema above matches Transform.trs()/world_trs() exactly. For a hierarchy,
prefer a world root plus explicitly local head/hand channels so application does
not depend on the arrival order of separate world-space parent/child samples.

Configure bindings before activation. Different active subscriptions cannot
claim the same transform, including through different coordinate spaces. Loss
holds the last accepted pose; removing the target cannot redirect data to a
replacement component with a reused slot. Removing the subscription releases
its native receive participation. Removing the socket closes I/O and releases
its dependent runtime channels. Runtime channel handles are excluded from scene
serialization; the owning MMS code recreates their declarations.

Publications also support `bind_transform(target, options?)`. While enabled,
a publication samples the transform natively and sends changes using the
same TRS schema. A local-space binding can target `TOP.hinge`. Its translation
and scale are identity, and the receiver projects the rotation onto its
authored hinge axis and applies its local limits.

```mms parse-only
let outgoing = socket.publish("launcher.breech")
    .to(peer_address)
    .type(pose_type)
    .bind_transform(hinge)
let incoming = socket.subscribe("launcher.breech")
    .from(peer_address)
    .type(pose_type)
    .bind_transform(hinge)
outgoing.set_binding_enabled(false)
outgoing
incoming
on(pointer, "PointerGrabbed", fn(event) {
    if event.target != rear_half { return }
    incoming.set_binding_enabled(false)
    outgoing.set_binding_enabled(true)
})
on(pointer, "PointerReleased", fn(event) {
    if event.target != rear_half { return }
    outgoing.set_binding_enabled(false)
    incoming.set_binding_enabled(true)
})
```

`set_binding_enabled(bool)` can run after activation and keeps the channel
registered. Disabling an active transform publication queues one final
snapshot. The receiver keeps its last pose and discards samples received while
its binding is paused. Delivery, including the final snapshot, uses
best-effort UDP.

## Imperative receiving and partial writes

Without a binding, a subscription exposes UdpSample events:

```mms parse-only
let positions = socket.subscribe("position").from("100.64.0.1:7011").type(vec3)
positions
on(positions, "UdpSample", fn(event) {
    hand.set_translation(event.value, "world")
})
```

Transform.set_translation(vec3, space?) and set_rotation_quat(vec4, space?)
merge at intent execution, preserving untouched properties even when separate
handlers queue writes in the same drain. Their default space is local. This
supports fine-grained imperative mappings immediately; declarative individual
transform-property methods remain future extensions. Avoid competing manual
and declarative writes to the same property; ordinary intent order determines
the result, and a later received sample re-applies a declarative binding.

## Face morphs and externally driven hands

```mms parse-only
let blink = socket.subscribe("blink.left").from("100.64.0.1:7011").type(f32)
blink.bind_morph(avatar_gltf, "left_eye_blink", { name = "remote_left_blink" })
```

A scalar morph binding resolves a semantic MorphTargetMap slot within exactly
that GLTF instance, waits for imported targets, and accepts finite [0,1] weights.
It owns the final weight over local idle/tracking drivers until released.
Removing it restores ordinary local driver/base behavior. GLTF.morph_weight(slot)
reads the effective mapped value for publication; before import it returns zero.

PoseInput marks scripted/replayed/network head targets for local AVC embodiment
without activating a local input provider. XRHand(...).external(true)
marks hand targets supplied externally: OpenXR neither polls nor clears them.
Use XRHand.set_pose_valid(bool) for an externally driven hand; pose_valid()
reads validity for both hardware and external hands. Remote IK remains local.
The demo sends separate f32 validity channels (0 or 1) alongside hand poses,
then sets those flags from UdpSample handlers. No solved bones cross the wire.

## State, addresses and scope

UdpState carries ready and message, reporting readiness changes and errors.
A schema mismatch does not deliver samples. Compatible subscriptions share the
same native receive storage. from selects a numeric sender address and port;
it is routing/filtering, not authentication. Network names do not resolve local
ComponentIds. See [wire format and bounds](../spec/typed-udp-channel-wire.md).

The [multiplayer demo](multiplayer.md) uses the existing control session for
roster, address discovery and gameplay requests, and typed binary channels for
spatial/face telemetry. Session control heartbeats are independent of typed spatial samples.
Audio compression/worker research and AudioStream/voice chat are deferred in
[the design draft](../draft/typed-telemetry-mms-bindings.md).

Authored clips can call `model.set_morph_weight("left_eye_blink", weight)` with a
finite weight in `[0, 1]` after GLTF initialization. Pass `null` to release that
script value. Received bindings take priority over script values; script values
take priority over local tracker/fallback drivers. Both APIs resolve semantic
slots within that GLTF instance. The multiplayer desktop blink clip uses this
setter; remote copies receive its effective weights through the existing channels.

The native endpoint now leases sockets from the scene's shared `UdpSystem`, also
used by session, eye-tracking and terminal adapters. Its shared raw I/O does not
change channel schemas, ACK/epoch policy, binding semantics or the wire protocol.
Standalone Rust endpoints can use `ChannelSocket::bind_on(&udp, address)` to share
a transport owner explicitly; `bind(address)` retains standalone behavior.
