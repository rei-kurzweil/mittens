# Typed UDP channels: native foundation

Status: native foundation implemented, with an MMS/scene adapter and migrated
Mitten demo. See [MMS channel usage](../how_to/typed-udp-channels.md). Entry point:
[`engine::udp_channels`](../../src/engine/udp_channels/mod.rs). Design and next
steps: [task](../task/typed-udp-channels-and-mms-bindings.md).

## Schema and codec

Schema accepts MMS TypeTag and RecordType identities through TransportValue.
Existing tags are f32, vec2, vec3, vec4 and str. Closed records contain 1–32
primitive fields, sorted lexicographically. Ordinary tables of tags are not
record types. No Rust discriminants, memory layouts or local arena handles are
serialized. Future mat4/quaternion tags remain language extensions; four vec4
fields can already represent 64 bytes of matrix-shaped storage without defining
matrix semantics or a new language tag.

The initial codec is a small explicit fixed layout for this closed type
vocabulary: numeric lanes are finite IEEE f32 values encoded little-endian;
strings have a little-endian u16 byte length followed by UTF-8. Values follow
schema order without per-sample names or type tags. No serde/JSON is used in the
native channel wire path. This selects an initial versioned channel codec, not
a new general-purpose serialization format. Postcard and bincode also support
raw binary floats ([Postcard specification](https://postcard.jamesmunns.com/wire-format),
[bincode specification](https://docs.rs/crate/bincode/2.0.1/source/docs/spec.md));
this slice avoids an extra serialization model/dependency for runtime MMS
schemas. No comparative performance claim against those codecs has been made.

Native numeric encode/decode uses caller-owned buffers without heap allocation.
The MMS-value adapter materializes owned transport arrays/tables/strings only
when requested; it is used by the MMS host and unbound subscription callbacks. MMS numbers currently
cross this boundary as f64: encoding rounds to f32 and rejects overflow or
nonfinite values. Native finite f32 bits, including signed zero and subnormals,
round-trip exactly. Payload validation rejects truncation, trailing bytes,
wrong vector lengths, extra/missing/duplicate record fields and invalid UTF-8.
Quaternion semantic validation belongs in a future typed binding.

## Version 1 datagrams

Maximum datagram size is 1,200 bytes, including a 38-byte header. There is no
fragmentation in this slice; oversize declarations/writes fail explicitly.

| Header field | Bytes |
| --- | ---: |
| Magic `MCHN` | 4 |
| Version (1) | 1 |
| Kind | 1 |
| Publisher socket epoch (UUID bytes) | 16 |
| Wire channel ID, little-endian u64 | 8 |
| Sequence, little-endian u64 | 8 |

Kinds are explicitly assigned: Declare=1, Ack=2, Sample=3, Withdraw=4, Reject=5.
Control sequence is zero; sample sequence starts at one. ACK/reject echo the
publisher epoch and channel ID, and are matched against the expected sender.
Wire IDs are nonzero, monotonic and never reused within an epoch; exhaustion
is an error. Together with a fresh socket epoch this prevents slot-reuse
collisions. Local PublicationId, SubscriptionId and ChannelId are separate
slotmap generational handles and never appear on the wire.

Declare carries a u8-length channel name, then schema kind (primitive=0,
record=1). Primitive schemas carry one tag byte. Record schemas carry a u8 field
count, then each canonical u8-length field name and tag byte. Explicit tags:
f32=1, vec2=2, vec3=3, vec4=4, str=5. Names contain 1–96 UTF-8 bytes without NUL.
Unknown versions/tags/kinds and noncanonical/duplicate fields are rejected.

Sample carries only schema-defined bytes. A TRS record has 10 f32 lanes (40
payload bytes, 78 bytes including the channel header). Control packets other
than Declare have no payload. Sequence metadata currently orders samples;
capture timestamps and batched samples remain future wire-version work.

## Native endpoint semantics and bounds

One ChannelSocket multiplexes publications and subscriptions. Each publication
currently names one numeric destination; fan-out can use multiple publications.
Duplicate publication destination/name pairs and incompatible local subscription
schemas for one sender/name fail before activation. Compatible subscriptions
share received-channel storage. Only explicitly subscribed publisher addresses
and names can establish receive channels. This is filtering, not authentication.

Declarations retry every 250 ms and continue as heartbeats after acknowledgment.
Only compatible registration is acknowledged. Publication writes before ACK
return an error; values are not silently queued. Samples are unreliable and
latest-value: higher sequence wins, values coalesce within a poll, and the OS
send-buffer-full case returns false. Commands requiring delivery guarantees
must not rely on this telemetry behavior.

Three seconds without accepted channel traffic loses the subscription; three
seconds without an ACK loses publication readiness. Poll drives these timeouts
using a monotonically advancing Instant. Histories preserve sequence floors
across timeout and unsubscribe, reject retired socket epochs, and tombstone
withdrawals even when a withdrawal arrives before its declaration. Removing a
publication sends a best-effort withdrawal; dropping the socket is detected by
remote timeout. Detaching one subscription leaves other consumers active.

Fixed limits per socket: 64 publications, 64 subscriptions, 64 active received
channels, 16 remembered publisher addresses, 64 retired epochs per publisher,
and 256 channel-history entries per current publisher epoch. Histories never
evict protection against delayed packets; capacity exhaustion fails closed and
requires a new socket lifecycle. These are initial native limits, not a final
MMS configuration API.

Poll processes at most 256 datagrams. Receive values reuse per-channel buffers;
numeric packet processing and sample writes allocate no per-packet heap storage.
Registration may allocate bounded schema/name/history state. Events are capped
at 256 until drained; overflow increments dropped_events. Readiness and latest
sample remain queryable even if notification events overflow. Samples expose
borrowed bytes valid until the next mutable socket operation. Poll does not
mutate scene components, integrate physics or run IK.

## Validation and measurement

Run `cargo test --lib udp_channels` with UDP loopback permission: 12 functional
tests cover codec layout, malformed input, shared/multiple channels, schema
mismatch, late subscription, timeout/retry, sequence floors, sender filtering,
restart, withdrawal ordering, limits and bounded events. One measurement test
is ignored in the normal suite.

Run `scripts/benchmark_udp_channel_codec.sh` for an isolated release build of
the channel modules and their workspace-locked dependencies, without rebuilding
the renderer. It runs offline and reuses a target subdirectory for build cache.
It performs 200,000 iterations per measurement with black_box inputs and reused
output buffers, with no timing assertions.

Representative local run (nanoseconds per operation, subject to machine noise):

| Numeric shape | Binary bytes | JSON array bytes | Binary encode / decode ns | JSON encode / decode ns |
| --- | ---: | ---: | ---: | ---: |
| vec3 | 12 | 32 | 7 / 12 | 65 / 77 |
| vec4 | 16 | 42 | 7 / 13 | 77 / 95 |
| TRS | 40 | 85 | 8 / 26 | 234 / 201 |
| Four-vec4 record | 64 | 109 | 10 / 34 | 254 / 298 |

A mixed label/weight record used 15 binary bytes versus 35 JSON object bytes;
transport-value encode/decode measured about 36/112 ns versus JSON Value
encode/decode 101/203 ns. Those owned dynamic representations differ, so this
is illustrative rather than an identical end-to-end MMS comparison.

All sizes exclude the channel/UDP/IP headers and registration traffic. Numeric
JSON baselines are bare fixed arrays, without the existing multiplayer JSON
field names/envelope. Results measure codec work only, not networking, script
execution, frame rate or remote latency. The demo now uses this foundation for spatial/face telemetry. The existing
JSON multiplayer protocol remains the roster/gameplay control plane.
