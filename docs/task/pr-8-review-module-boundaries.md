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
