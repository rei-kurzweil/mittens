# One-shot XR eye-tracking source election

Date: 2026-09-06

Status: corrective design; implementation pending

Prerequisite: [XR eye-tracking registered component indexes](xr-eye-tracking-registered-component-indexes.md)

Revises: [Generic XR eye-tracking source selection](generic-xr-eye-tracking-source-selection.md)

Parent: [Eye and face tracking epic](epic/eye-and-face-tracking.md)

## Outcome

An avatar must have exactly one active eye-tracking source after startup discovery. `XREyeTracking`
opens all allowed candidates for a short configurable discovery window, records which candidates
produce valid eye data, and chooses the highest-priority working source when the window ends. It
then locks that choice and stops probing or arbitrating the other eye sources.

The selected source remains selected until the `XREyeTracking` component is removed and added again
or an explicit reinitialization/reselection operation is introduced and invoked. Ordinary frame
updates continue consuming data from the selected source; they do not reconsider source choice.

This replaces continuous per-frame source arbitration and the current independent gaze/closure
selection. One source owns all eye-tracking channels for that avatar.

## Why revise the first slice

The first generic slice creates source-specific children and resolves gaze and closure every frame.
It can therefore select HTC gaze and OSC closure at the same time, or change ownership as source
samples appear and disappear. That is useful for lower-face composition, but it is the wrong policy
for eyes:

- eye pose and eyelid signals should come from one coherent tracker and calibration;
- packet timing should not cause ownership changes during a session;
- lower-priority traffic should not unexpectedly take control after setup;
- inactive listeners should not remain open indefinitely;
- troubleshooting is simpler when an avatar has one stable selected source.

The generic component is a startup source election mechanism, not a live mixer.

## Component roles

Keep all source implementations as independently configurable components:

- `VRChatOSCEyeTracking`
- `HTCEyeTracking`
- `MediaPipeEyeTracking` (future implementation)

`XREyeTracking` owns only election policy, shared post-selection eye settings, and the normalized
output consumed by AVC. Source-specific children own endpoints, devices, models, decoding, and
source-specific calibration.

```mms
AVC {
    XREyeTracking.on()
        .priority(["htc", "vrchat_osc", "mediapipe"])
        .discovery_window_seconds(2.0) {
        VRChatOSCEyeTracking.listen("127.0.0.1", 9000)
        HTCEyeTracking.listen("127.0.0.1", 9002)
        MediaPipeEyeTracking.on()
    }
}
```

When source children are omitted, the selector creates defaults for the entries in its priority
list. An authored child replaces the default configuration for that source. A source omitted from
the priority list is not opened.

A source-specific component placed directly beneath AVC remains the explicit form. It bypasses
generic discovery and becomes that avatar's eye source immediately.

## Election lifecycle

Use an explicit state machine:

```text
Uninitialized
    -> Discovering { started_at, deadline, candidates }
    -> Selected { source, component }
    -> Unavailable
```

### Initialization

When `XREyeTracking` initializes:

1. Validate the ordered priority list and source-child topology.
2. Materialize missing default source children.
3. Start only the source candidates named in the priority list.
4. Set a monotonic deadline using `discovery_window_seconds`.
5. Clear all evidence and selected-source state from any prior lifetime.

Default discovery window: `2.0` seconds. The value must be finite and non-negative. A zero-second
window performs selection on the first system opportunity and is primarily useful for deterministic
tests or explicitly fast startup.

### Discovery window

During the bounded window, every candidate may receive and validate data. Record per candidate:

- whether valid eye data has been observed;
- the most recent valid-data timestamp;
- which eye channels were present;
- bind/device/unsupported status for diagnostics.

Do not select immediately when the first source sends a packet. Waiting until the deadline lets a
higher-priority device that initializes more slowly still win.

A candidate counts as working at the deadline only if it produced valid eye data and its most recent
sample is still within a short liveness threshold. A malformed packet, successful UDP bind, opened
camera, or unsupported placeholder does not count as working. This avoids allowing a single stale
startup packet to win the session.

### Election at the deadline

At the deadline:

1. Filter the priority list to candidates that are working.
2. Choose the first remaining source in the authored order.
3. Store the selected source and component identity on `XREyeTracking`.
4. Copy only that component's normalized complete eye state to the generic output.
5. Stop and release every unselected receiver/device subscription.
6. Emit one diagnostic/selection event describing the winner and observed candidate statuses.

Gaze, closure, pupil, wide, squeeze, or future eye channels do not elect separate sources. Missing
channels from the winner remain missing; the selector must not fill them from another transport.

### No source available

If no candidate is working when the window ends, enter `Unavailable`, release every candidate
receiver, and leave AVC's eye drivers neutral. Do not keep probing forever.

Recovery requires removal/re-addition or an explicit reselection operation. This makes behavior
bounded and predictable. A UI may offer “retry eye tracking” by replacing/reinitializing the generic
component rather than leaving permanent background discovery active.

### Selected operation

Once selected:

- poll or subscribe only to the chosen source;
- do not compare it with other source timestamps or priorities;
- do not automatically fail over;
- apply all available eye channels from that one source;
- retain selected-source provenance for diagnostics.

If the selected source stops producing data, use per-channel staleness rules to release eye-bone and
morph ownership to neutral/rest values. Remain in `Selected`; loss of data does not restart election
or activate a fallback source.

This distinction is important: selected-source liveness controls whether its current values may be
applied, not whether another source may take ownership.

## Reinitialization semantics

`World::init_component_tree` is idempotent today and does not invoke `Component::init` twice for the
same component record. Therefore removal followed by adding a new `XREyeTracking` component is the
initial supported retry mechanism.

If a reusable operation is needed, add an explicit `reselect()`/`restart_discovery()` intent or
method that:

- closes the selected receiver;
- clears retained samples, evidence, and diagnostics;
- returns the selector to `Discovering` with a new deadline;
- does not rebuild or reinitialize the avatar/GLTF subtree.

Do not overload ordinary per-frame initialization checks to restart discovery implicitly.

## Eye tracking versus face tracking

The one-source rule applies to the eye-tracking semantic group. Lower-face tracking is expected to
support composition because different devices may provide complementary mouth, jaw, cheek, or lip
signals.

MediaPipe must expose eye and mouth tracking separately at the component/API boundary:

- `MediaPipeEyeTracking` participates in the one-shot eye-source election;
- `MediaPipeMouthTracking` participates in the future lower-face routing/mixing policy.

They share the planned engine-side
[`MediaPipeSystem`](optional-mediapipe-tracking-bridge.md#engine-owner-mediapipesystem) for
receiver/session state, decoding, normalization, and independent channel snapshots. The external
Python bridge owns the shared webcam capture and inference backend. `MediaPipeEyeTracking`,
`MediaPipeMouthTracking`, and hand tracking subscribe to that provider while retaining their own
semantic components and ownership policies. Hand/eye/mouth traffic has independent freshness;
receiving one channel never makes another channel live.

Do not let the future face mixer feed MediaPipe mouth availability into eye-source election, or vice
versa. Each capability has independent validity and lifecycle even when produced from the same
camera frame.

## Implementation changes

1. Add explicit `Uninitialized`, `Discovering`, `Selected`, and `Unavailable` runtime state to
   `XREyeTrackingComponent` or `XREyeTrackingSystem` state keyed by component ID.
2. Add the validated `discovery_window_seconds(f32)` builder with a `2.0` default and a monotonic,
   injectable test clock.
3. Restrict default materialization and receiver startup to the priority allow-list.
4. During discovery, collect bounded candidate evidence without exposing samples to AVC as separate
   competing drivers.
5. At the deadline, elect one whole source and close/unsubscribe every loser immediately.
6. Replace per-channel source selection in `resolve_generic_trackers` with copying all normalized
   eye channels from the one selected source.
7. Add selected-source stale release without fallback or automatic reelection.
8. Make direct source-specific AVC children bypass discovery. Diagnose/reject multiple direct eye
   sources and a generic selector combined with a direct source sibling.
9. Preserve the manual transport-selection example using direct `VRChatOSCEyeTracking` and
   `HTCEyeTracking` components.
10. Add `MediaPipeMouthTracking` only with the MediaPipe implementation or its lower-face task; keep
    it out of the eye-source enum.

## Tests

- Default discovery lasts two seconds under an injected clock.
- A low-priority source arriving first does not win before the deadline.
- A higher-priority source arriving near the deadline wins if it is still live.
- A higher-priority source that emitted only one stale early packet does not beat a live lower one.
- Exactly one whole source supplies gaze, closure, and every other eye channel after election.
- The winner remains fixed when another source later begins sending.
- Loss of the winner releases stale channels but does not select another source.
- Unselected sockets/cameras/subscriptions are closed at election.
- No working source enters `Unavailable` and stops discovery.
- Removing and adding the selector starts a fresh discovery window with no retained evidence.
- A future explicit restart operation has the same clean-state behavior.
- A direct source-specific AVC child starts immediately and does not create generic candidates.
- Ambiguous direct topology is diagnosed and never falls back to newest-packet arbitration.
- MediaPipe eye and mouth components can share a backend while maintaining independent semantic
  state and ownership.

## Acceptance criteria

- An avatar has no more than one selected eye-tracking source.
- Source election occurs once, after a configurable startup window, rather than every frame for the
  component's lifetime.
- Default `XREyeTracking.on()` waits two seconds so slower high-priority hardware can participate.
- Priorities select one complete eye source; channels are never mixed across eye transports.
- After election, only the winning eye receiver remains active.
- A stopped winner becomes neutral/stale without automatic failover.
- Reselection occurs only through explicit lifecycle action.
- Lower-face tracking remains free to mix complementary sources.
- MediaPipe exposes separate eye and mouth components even if both share one capture/inference
  provider.

## Expected files

- `src/engine/ecs/component/xr_eye_tracking.rs`
- `src/engine/ecs/system/xr_eye_tracking_system.rs`
- `src/engine/ecs/system/avatar_control_system.rs`
- `src/scripting/component_registry.rs`
- `src/scripting/runtime_config.rs`
- eye-tracking examples and focused tests
- future MediaPipe provider and lower-face routing files
