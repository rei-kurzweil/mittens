# PR #8 review fixes: responsibility boundaries

The accepted review fixes touch oversized component_registry, eye-tracking,
UDP channel adapter/endpoint and grabbing modules. Keep this batch scoped.
Future splits should separate component-family authoring bindings, eye-provider
socket lifecycle, typed-channel registration/binding lifecycle, and grab
ownership transitions; keep their cohesive state machines together. Regressions
may live in focused family test modules. Do not split individual primitives.

Preserve bounded polling, direct numeric decoding and batched scene updates.
No new per-frame allocation, dynamic dispatch, socket recreation on unchanged
transport owners, or world scans just to split files. Use static/generic helpers
for deterministic send failure tests. Measure any meaningful hot-path change.

Validate failed registration recovery, duplicate morph writers, publication
fairness/retry timestamps, eye transport ownership, grab takeover, and the full
MMS typed-avatar example. The review tracker records individual acceptance.

## Lower-body support frame follow-up

Keep support-reference invalidation in the lower-body instance scheduler, with
support-query geometry in `support.rs` and regressions in `tests/visual_rigs.rs`.
If the scheduler grows further, split cohesive tracking/eligibility and retained
support-reference policy from pose solving. Preserve the existing ancestor walk,
direct reads and bounded BVH queries; no new frame allocations or world scans.
Verify switching frames at different heights, losing/acquiring a frame, unchanged
frame crouching and locomotion, and the full lower-body regression suite.

## Collision response builder implementation

Keep this slice focused on an explicit Slide/ReadOnly response policy, capsule
participation and observed contacts. Retain the existing static-contact solver,
using its shared retained/refitted query index for read-only moving proxies;
separate observation from response writes. Future splits should group scene
contact observation/response, index maintenance, avatar capsule authoring, and
MMS component-family bindings. No per-body BVHs, dynamic dispatch, repeated
geometry construction, or per-frame contact-buffer allocation after warmup.

Validate read-only overlaps/sweeps with unchanged roots and velocity, missing
response targets, indexed moving proxies and self/support filtering, runtime
mode transitions, MMS constructor/body forms and serialization, real remote
avatar capsules and local slide behavior. The broader ground-query redesign
remains separate from this builder/participation implementation.

Implemented: `AVC.collision_response`, generic `Collidable.response`, independent
native response permission/query owner, retained read-only proxies and observed
contacts. The remote example uses an enabled read-only capsule. Verified builder
forms/serialization/errors (2 checks), readonly behavior including sweeps and
missing targets (5 filtered checks), retained BVH (3), static contact (18 plus
1 intentionally ignored), capsule behavior (34), lower body (37), and multiplayer
(17). Filters overlap; these are not distinct-test totals. `cargo clippy --lib`
completes with warnings. A profiled two-peer scene also passes; user-reported
workstation activity means timing output is exploratory, with no speedup claim.
See `docs/how_to/avatar-collision-response.md` for the implemented scope; the
unified support-query redesign remains in the proposal.
