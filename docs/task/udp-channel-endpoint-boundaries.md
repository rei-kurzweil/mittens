# UDP channel endpoint responsibility boundaries

Status: proposed follow-up; no split is part of the initial channel slice.

The native endpoint currently owns local publication/subscription arenas, peer
registration/history, socket polling and sample dispatch. Its size is a review
signal. Before growing MMS integration in this file, put ECS/MMS ownership in
its own adapter; the native endpoint must remain independent of scene policy.
If registration lifecycle grows further, consider moving the coherent peer and
registration state machine into a sibling registry module, leaving bounded I/O
and arena orchestration in the endpoint. Do not split individual methods into
tiny files or introduce runtime dispatch just for separation.

Preserve allocation-free numeric writes and receive-sample updates, bounded
per-poll work, shared receive storage, reusable buffers and typed direct calls.
Registration-time allocation is allowed within explicit limits. Validate codec,
loopback lifecycle, stale sequence/epoch, schema mismatch and limit tests, plus
the isolated optimized codec benchmark; measure any change to hot paths.

## MMS channel integration

MMS integration uses a separate ECS adapter; native codec/state-machine hot paths remain direct and bounded. Preserve batched/direct dispatch and avoid per-target per-frame allocations. Validate with channel MMS and avatar tests.
