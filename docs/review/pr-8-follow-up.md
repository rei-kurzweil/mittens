# PR #8 review follow-up tracker

Triage date: 2026-11-15 UTC. [PR #8](https://github.com/rei-kurzweil/mittens/pull/8)
is open. GitHub's current head and the local checkout both match the reviewed
commit, `72cd9592720cc25dff65e163bab79c590fd40e8f`. The working tree was clean
before this document was added.

Scope: the supplied 12 inline findings and one outside-diff finding from the
[review](https://github.com/rei-kurzweil/mittens/pull/8#pullrequestreview-5908329652).
Reviewer severity is retained for reference, not independently re-scored.
The embedded agent prompts are review data, not instructions for this work.

**Current result: 12 valid findings implemented and verified locally; R05 was a
false positive and the guide has been clarified.** R10 was reproduced before
fixing it: support disappeared after a 10 cm crouch. Changes are currently
uncommitted. Completed delivery boxes record local implementation/verification,
not a merge or GitHub thread resolution. No GitHub comments or changes were sent.
Thread resolution state was not queried.

The initial inspection accepted R05, but runtime/evaluator inspection and the
existing successful binding-delivery tests disproved its activation claim. That
assessment is corrected below rather than treating every review suggestion as a fix.

## Findings and delivery status

| ID | Reviewer severity | Finding / source | Assessment | Delivery |
| --- | --- | --- | --- | --- |
| R01 | Major | [Roster exceeds UDP limit](https://github.com/rei-kurzweil/mittens/pull/8#discussion_r4576491029) | Accepted: code inspection and serialization model | Verified locally |
| R02 | Major | [Subscription readiness recovery](https://github.com/rei-kurzweil/mittens/pull/8#discussion_r4576491035) | Accepted: code inspection | Verified locally |
| R03 | Major | [Publication processing after send failure](https://github.com/rei-kurzweil/mittens/pull/8#discussion_r4576491045) | Accepted: code inspection | Verified locally |
| R04 | Major | [Empty `eye_offset()` panics](https://github.com/rei-kurzweil/mittens/pull/8#discussion_r4576491047) | Accepted: legacy call path inspected | Verified locally |
| R05 | Major | [Detached documentation subscriptions](https://github.com/rei-kurzweil/mittens/pull/8#discussion_r4576491001) | Rejected: standalone binding calls already attach the handle | Closed; guide clarified |
| R06 | Minor | [Heartbeat interval exceeds timeout](https://github.com/rei-kurzweil/mittens/pull/8#discussion_r4576491017) | Accepted: timing contradiction in code | Verified locally |
| R07 | Minor | [Stale component peer ID](https://github.com/rei-kurzweil/mittens/pull/8#discussion_r4576491023) | Accepted: endpoint/component paths inspected | Verified locally |
| R08 | Minor | [Morph cleanup by a failed registration](https://github.com/rei-kurzweil/mittens/pull/8#discussion_r4576491032) | Accepted: ownership mismatch in code | Verified locally |
| R09 | Minor | [Eye endpoints retain the old transport](https://github.com/rei-kurzweil/mittens/pull/8#discussion_r4576491041) | Accepted: cache and transport checks inspected | Verified locally |
| R10 | Minor | [Crouched visual-rig support](https://github.com/rei-kurzweil/mittens/pull/8#discussion_r4576491015) | Accepted: reproduced, fixed and regression-tested | Verified locally |
| R11 | Minor | [Grab release undoes takeover](https://github.com/rei-kurzweil/mittens/pull/8#pullrequestreview-5908329652) | Accepted: takeover/release paths inspected | Verified locally |
| R12 | Minor | [Unicode codec-benchmark paths](https://github.com/rei-kurzweil/mittens/pull/8#discussion_r4576491006) | Accepted: Rust compilation reproduced | Verified locally |
| R13 | Minor | [Invalid benchmark trial pairing](https://github.com/rei-kurzweil/mittens/pull/8#discussion_r4576491011) | Accepted: current script reproduced | Verified locally |

## Evidence, proposed actions and verification

### R01 — Bound the complete roster

- [x] Implemented and verified locally; uncommitted. See verification record below.
- Evidence: [`snapshot_chunks`](../../src/engine/ecs/system/multiplayer_system/protocol.rs)
  retains the entire roster in its first chunk even when it exceeds the budget.
  The [endpoint](../../src/engine/ecs/system/multiplayer_system/endpoint.rs) admits
  up to 16 clients with individually truncated 64-byte names, without an aggregate
  size check. Removing `MultiplayerSync` did not remove this roster path.
- Reproduction: a Python serialization model of the v2 packet, host + 16 clients,
  empty telemetry, maximal `u64` tick/sequence and 64-byte names measures **1,563
  bytes** for ASCII names and **2,651 bytes** for quote-only names, versus the
  1,200-byte limit. This models serialization; it is not a native endpoint test.
- Implemented action: an exact encoded-size admission check includes the host,
  escaping, assigned IDs and maximal envelope counters. An over-budget join is
  rejected before its ID is consumed, the roster changes, or a Welcome is sent.
  The host emits `MultiplayerError` with phase `admission`; the client remains
  unwelcomed. Unicode/emoji support and the 16-client upper bound are preserved;
  long names may reduce actual capacity. No fragmentation or partial roster
  application was introduced. Full maximum-length rosters would require a separate
  fragmentation/reassembly change if that capacity becomes a requirement.
- Verification: native tests for the maximum permitted/admitted roster, escaped
  and Unicode names, boundary-size joins, and empty telemetry. Encode every
  publication within `MAX_DATAGRAM`; show existing admitted peers remain connected
  beyond `PEER_TIMEOUT` when another join is rejected. If fragmenting, test loss,
  reordering, duplicates and incomplete generations without false peer departures.

### R02 — Clear registration errors at successful creation

- [x] Implemented and verified locally; uncommitted. See verification record below.
- Evidence: [`UdpChannelSystem`](../../src/engine/ecs/system/udp_channel_system.rs)
  sets `active` after inserting the native subscription but leaves `c.error` intact.
  Its later readiness projection retains subscription errors and suppresses ready.
- Implemented action: clear the old registration error at the successful native
  creation boundary. Do not indiscriminately clear errors during every ready poll.
- Verification: fail registration without a peer, configure the peer, register
  successfully, and exchange declarations without samples. Assert readiness
  recovers before any sample; later schema/receive/binding errors must remain visible.

### R03 — A failed publication must not starve the others

- [x] Implemented and verified locally; uncommitted. See verification record below.
- Evidence: [`ChannelSocket::poll`](../../src/engine/udp_channels/endpoint.rs)
  uses `?` inside its publication loop. Also, `send` returns `Ok(false)` for
  `WouldBlock`, but the loop currently advances `last_declare` anyway.
- Implemented action: process retry and timeout work for all publications, then
  return the first send error. Advance a declaration timestamp only on `Ok(true)`;
  neither a hard error nor `WouldBlock` means a declaration was transmitted.
- Verification: deterministically fail the first publication's send and prove a
  later publication still retries and expires when appropriate. Cover successful
  sends, `WouldBlock`, and repeated hard errors; assert retry timestamps and
  readiness transitions. Prefer a test-only failure hook over changing hot-path
  dispatch or relying on network-dependent routing errors.

### R04 — Reject empty legacy `eye_offset()` safely

- [x] Implemented and verified locally; uncommitted. See verification record below.
- Evidence: [`apply_call`](../../src/scripting/component_registry.rs) indexes
  `args[0]` for `eye_offset`. Legacy `spawn_tree` forwards each materialized call's
  arguments directly to it; RuntimeSpec arity validation does not protect that path.
- Implemented action: use `arg_f32_arr::<3>(args, 0)?`.
- Verification: construct a legacy materialized AVC call with no arguments and
  invoke `spawn_tree`; require `Err`, not a panic. Retain valid-offset roundtrip
  and nonfinite-value rejection checks. A RuntimeSpec-only arity test is insufficient.

### R05 — Binding expression statements already activate subscriptions

- [x] Rejected the activation-bug claim; clarified the guide and verified delivery.
- Corrected evidence: [`udp_channel_api.rs`](../../src/scripting/udp_channel_api.rs)
  returns the same component handle from both binding methods. In
  [`evaluator.rs`](../../crates/meow-meow-script/src/evaluator.rs),
  `Statement::Expression` dispatches `HostRequest::Attach` when its result is a
  `ComponentObject`. Consequently, the guide's standalone binding calls already
  emit/attach their subscriptions. An additional bare handle statement is redundant.
- Action: clarify the distinction between a standalone binding expression and
  storing its return value in a variable; the latter requires emission/attachment.
- Verification: the existing
  `mms_socket_methods_publish_bind_and_deliver_samples` and
  `morph_subscription_owns_only_its_avatar_slot_and_releases_on_removal` tests
  use the guide's binding-call-as-statement pattern without an additional emission.
  Both pass and verify actual transform/morph sample delivery, not merely parsing.

### R06 — Keep control updates inside the timeout

- [x] Implemented and verified locally; uncommitted. See verification record below.
- Evidence: [`register_session_on`](../../src/engine/ecs/system/multiplayer_system.rs)
  accepts any finite positive `tick_hz`, capped at 120. At `0.2` Hz its interval
  is five seconds; the endpoint timeout is three seconds.
- Implemented action: require at least 1 Hz for positive control rates, providing
  margin below the three-second timeout; reject smaller positive rates with a configuration error.
  Preserve finite-value handling and the upper cap. Independent heartbeat timing
  is an alternative if slow roster updates are useful, but is a larger change.
- Verification: test below/at the selected minimum and the existing invalid-value
  and upper-limit behavior. Exercise the system scheduler and real endpoint
  liveness across several timeout windows; an endpoint-only test that manually
  calls `publish` bypasses the affected scheduler. Prefer controlled time to sleeps.

### R07 — Project the actual connection state

- [x] Implemented and verified locally; uncommitted. See verification record below.
- Evidence: the [endpoint](../../src/engine/ecs/system/multiplayer_system/endpoint.rs)
  clears `local_peer_id` in `reset_client_view`; the
  [system](../../src/engine/ecs/system/multiplayer_system.rs) updates component
  `peer_id` only on `Connected`.
- Implemented action: synchronize the component from the endpoint's actual local
  ID, mapping `None` to zero after polling. Do not clear the local ID whenever
  any remote `PeerLeft` event occurs; another participant leaving is not a local
  disconnect. Handle the case where disconnect occurs before a roster arrives.
- Verification: host `Bye`, host timeout, reconnect/assigned ID, and an unrelated
  remote peer leaving. Check both the Rust component and MMS `peer_id()` projection.

### R08 — Release only an acquired morph binding

- [x] Implemented and verified locally; uncommitted. See verification record below.
- Evidence: [`remove`](../../src/engine/ecs/system/udp_channel_system.rs) clears
  morph state from the authored component before examining the removed runtime
  channel. A duplicate-writer rejection never inserts a native subscription but
  retains that authored target/slot.
- Implemented action: establish ownership using the removed runtime
  `Channel::Subscription` and its acquired binding before clearing morph state.
  Preserve socket-dependent teardown and normal owner cleanup.
- Verification: A owns a slot and receives a nonzero weight; B fails to acquire
  it. Remove B without sending A another sample and assert A's external factor
  remains unchanged. Remove A and confirm driver/base restoration; repeat through
  socket removal and against a separate GLTF instance/slot.

### R09 — Rebind cached eye endpoints when owners change

- [x] Implemented and verified locally; uncommitted. See verification record below.
- Evidence: [`tick_with_udp`](../../src/engine/ecs/system/xr_eye_tracking_system.rs)
  reuses the OSC/HTC endpoint maps without tracking transport identity.
  [`UdpSystem::poll`](../../src/engine/udp/mod.rs) rejects endpoints from another
  owner; both eye polling sites discard that error. Clones of the same owner are
  valid and must not trigger a rebind.
- Implemented action: compare underlying transport owners and release old leases
  before rebinding. Owner clones retain endpoints. Clear old bind-failure bookkeeping
  and OSC field-combining caches on a switch; the component retains its last known
  sample until fresh data arrives. Existing bind retry behavior is preserved.
- Verification: receive OSC and HTC packets through standalone `tick`, switch
  to scene `tick_with_udp`, and receive fresh packets again on the same configured
  ports. Verify old leases are released, diagnostics show the new owner, cloned
  owners keep their sockets, and switching back works.

### R10 — Retain crouched visual-rig support

- [x] Reproduced before the fix; implemented and verified locally, uncommitted.
- Evidence: [support queries](../../src/engine/ecs/system/ik/lower_body/support.rs)
  use the minimum rest-FK sole height as `bottom` for collision-disabled avatars.
  The [caller](../../src/engine/ecs/system/ik/lower_body/system.rs) derives those
  soles under the current body frame; the default support margin is `0.05`.
  Existing [visual-rig tests](../../src/engine/ecs/system/ik/lower_body/tests/visual_rigs.rs)
  cover standing support and an absent floor, not the crouch transition.
- Reproduction: `collision_disabled_visual_rig_retains_support_during_crouch`
  failed before the fix at 10 cm, with `support: None` and soles below the floor.
- Implemented action: retain an acquired floor height for visual-rig support
  queries and shift that reference with locomotion-frame vertical movement.
  Re-query the BVH every update; removed support does not persist. Clear old foot
  references on rig replacement. This uses the existing ancestry walk, direct
  frame reads and per-avatar state, without world scans or per-update allocations.
  Received root/input transforms are never corrected by this path.
- Scope: this fixes retention across crouching. First acquisition without any
  previous support still uses rest-FK soles; starting already crouched is not a
  covered guarantee of this change.
- Verification: retain support and planted solved soles through the transition,
  leave received root/input transforms untouched, and preserve missing-floor and
  scale behavior. Check candidate-query bounds as well as final surface acceptance.

#### Follow-up — Invalidate cached support when the input frame changes

- [x] Reproduced, fixed and verified for the support-frame follow-up.
- The additional review of `LowerBodyIkSystem::tick` was valid: an unmatched
  frame previously produced a zero vertical delta while still retaining the old
  world-space floor height. The original crouch fix did not distinguish that
  transition from an unchanged frame.
- Regression: `collision_disabled_support_reacquires_floor_after_frame_change`
  failed before the fix, selecting the floor at height 0 after reparenting the
  input to a different frame at height 0.4. Both floors remained present.
- Change: compute a retained floor only for matching known frames, or when both
  frames are absent (the existing fallback behavior). Changing, losing or
  acquiring a known frame discards the cached height and uses fresh sole-based
  support acquisition. No new allocation, scene write or world scan is introduced.
- Verification: the regression passes with walking enabled and disabled, repeated
  switches between heights, known → unknown → known frame transitions, an
  unchanged unknown-frame crouch, correct floor identity/sole height, and unchanged
  received input/root transforms. `cargo test --lib lower_body`: **37 passed**;
  `git diff --check`: passed. Logs: `/tmp/pr8-frame-before.log` and
  `/tmp/pr8-frame-lower-body.log`.

### R11 — Preserve the new parent when a grab loses ownership

- [x] Implemented and verified locally; uncommitted. See verification record below.
- Evidence: [`GrabbableSystem`](../../src/engine/ecs/system/grabbable_system.rs)
  detects a target no longer under `pointer_parent` and releases it. `release`
  then unconditionally restores the original parent when the target still exists.
- Implemented action: restore the original parent only while the target is still
  parented to this grab's pointer parent. Otherwise stop local following and
  release ownership/events without undoing the replacement parent.
- Verification: original parent A, pointer parent P, replacement parent B, all
  distinct. After takeover/release, assert B and the target's world pose remain,
  the old grab no longer follows, and `PointerReleased` occurs once. Retain normal
  pointer release, deleted-target and removed-original-parent coverage.

### R12 — Generate Rust-compatible Unicode paths

- [x] Implemented and verified locally; uncommitted. See verification record below.
- Evidence: [codec benchmark script](../../scripts/benchmark_udp_channel_codec.sh)
  uses default `json.dumps` for both generated `#[path]` strings. A temporary
  `café.rs` module reproduced Rust's `incorrect unicode escape sequence` error
  with default JSON escaping; the same module compiled with `ensure_ascii=False`.
- Implemented action: preserve Unicode in both generated Rust path literals. Keep
  serialization of paths for TOML distinct; TOML and Rust have different escape
  rules. Check unusual control characters separately if adopting a general encoder.
- Verification: run the codec benchmark from an actual Unicode checkout path,
  including spaces, and retain normal-path behavior. The triage reproduction only
  compiled generated module paths; it did not run the full benchmark there.

### R13 — Require one-to-one benchmark trial pairing

- [x] Implemented and verified locally; uncommitted. See verification record below.
- Evidence: [summarizer](../../scripts/summarize_udp_transport.py) constructs a
  shared-trial dictionary without duplicate checks and iterates only separate
  trials. Three temporary CSV variants based on the committed `vr2.csv` all
  incorrectly exited successfully: duplicate shared trial, duplicate separate
  trial, and an additional shared trial `999`.
- Implemented action: reject duplicate IDs per dataset, and require identical ID
  sets for each separate/shared workload and measurement kind before comparing.
  Validate before filtering lossy UDP runs. Preserve intentionally single-mode
  reports, including runtime-only reports, while rejecting a missing counterpart
  in a comparison that is actually present but incomplete.
- Verification: valid pairs in different row orders; duplicates on either side;
  unmatched IDs in both directions; an entirely missing measurement kind on one
  side; lossy rows; single-mode reports; and all existing committed CSVs. Valid
  paired results must remain numerically unchanged.

## Work order and acceptance evidence (completed locally)

1. Address roster availability and channel state/ownership: R01–R03 and R08.
2. Fix the legacy panic and verify documentation activation semantics: R04–R05.
3. Fix session/transport lifecycle: R06–R07 and R09.
4. Reproduce/fix solver behavior and fix grab takeover: R10–R11.
5. Harden benchmark tooling: R12–R13 before relying on new comparisons.

Keep each change scoped and update its delivery entry with commit, regression
name, command, result and any remaining limitation. Respect the existing
`docs/task/` responsibility-split proposals before editing oversized source
files; do not turn these fixes into unrelated module refactors or add runtime
dispatch/allocations solely to make failure testing convenient.

Suggested final regression checks after the relevant fixes:

```sh
cargo test --lib multiplayer_
cargo test --lib udp_
cargo test --lib eye_tracking
cargo test --lib lower_body
cargo test --lib shared_grab_tests
cargo test --lib grabbable
cargo clippy --lib --tests
git diff --check
```

The targeted regression results below, rather than the original broad tests
alone, support the completed boxes. UDP checks used permitted loopback networking.

## Implementation verification record

All changes below are in the working tree, not yet committed. No review threads
were marked resolved. Logs are available in this session under `/tmp/pr8-*.log`.

| Check | Outcome | Relevant findings |
| --- | --- | --- |
| `cargo test --lib multiplayer_` | 17 passed | R01 full short-name roster, ASCII/escaped/emoji size-limited admission, ten seconds of admitted-peer liveness, maximal envelope encoding; R06 rate validation and 1 Hz scheduling; R07 unrelated departure, host Bye, reconnect, timeout; two-peer typed-avatar demo |
| `cargo test --lib udp_` | 27 passed, 1 intentionally ignored | R02 registration recovery without a first sample and retained later schema errors; R03 deterministic send failure, later retry/timeout, `WouldBlock` timestamp behavior; R08 rejected duplicate removal preserving the active morph owner; R05 actual binding-statement delivery |
| `cargo test --lib eye_tracking` | 24 passed | R09 real OSC/HTC packets through standalone → shared → cloned shared → standalone owners, old lease release and unchanged clone counters |
| `cargo test --lib lower_body` | 36 passed | R10 standing/crouch/standing at scales 0.5/1/1.7, untouched input/root transforms, floor removal and locomotion-height changes; existing real-rig walking tests |
| `cargo test --lib pointer_grab_events_tests` | 1 passed | R11 distinct original/pointer/replacement parents, replacement world pose, single release notification and stopped old following |
| `cargo test --lib shared_grab_tests` | 3 passed | Existing MMS grab/handoff, disconnect and late-receiver regressions |
| `cargo test --lib legacy_eye_offset` | 1 passed | R04 missing-argument legacy `spawn_tree` returns an error rather than panicking |
| `python3 -m unittest discover -s scripts/tests -p 'test_summarize_udp_transport.py'` | 5 passed | R13 duplicates on either side/runtime, unmatched IDs including lossy rows, missing measurement kinds, order independence, single-mode reports and unchanged recorded valid output |
| Summarizer against all committed transport CSVs | 10 files accepted | R13 existing-data compatibility |
| Codec benchmark in temporary Unicode checkout path | 1 codec measurement passed | R12 actual `é` + space checkout, both generated Rust paths compiled; no timing conclusions drawn |
| `git diff --check` | Passed | Whitespace verification |

`cargo clippy --lib --tests` was rerun. The first pass found a new non-test build
error because the endpoint peer-ID getter had been test-only; that was fixed.
The subsequent default run reaches four **preexisting** `clippy::approx_constant`
errors in unchanged `src/main.rs:120,237` (three occurrences) and
`src/scripting/display_car_tests.rs:396` (one occurrence). Those literals were
also checked in `HEAD`; this batch does not modify them.

`cargo clippy --lib --tests -- -A clippy::approx_constant` completes successfully,
with warnings. This isolated baseline allowance is a command-line verification
choice, not a source lint suppression. The default strict check is **not clean**;
its baseline errors and the repository's warnings remain recorded here.

## Security and review-summary notes

The [review summary](https://github.com/rei-kurzweil/mittens/pull/8#issuecomment-6886882748)
also calls out roster availability and the lack of authentication. R01 can affect
ordinary allowed joins; it does not require an exploit demonstration to warrant
a fix. Address pinning and UUID epochs are routing/lifecycle checks, not peer
authentication. Preserve that distinction in documentation; authentication,
encryption and hostile-network deployment are separate design scope, not implicit
requirements added to this review-fix batch.

## Original triage verification performed

- Read current GitHub PR metadata through the GitHub connector and compared its
  head SHA with local `git rev-parse HEAD`.
- Inspected all 13 reported code/documentation paths and relevant callers/tests.
- Ran the roster serialization model, three malformed trial-pairing CSV cases
  against the current script, and both Unicode Rust compilation cases in temporary
  directories. No implementation files or committed benchmark data were changed.
- The original documentation-only pass did not implement fixes or rerun Clippy.
  The later implementation and validation record above supersedes that initial state.
