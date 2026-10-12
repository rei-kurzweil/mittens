# Transform operators

The MMS API is `TOP`, short for `TransformOperator`. Constructors choose the
operation; builder calls configure it. All kinds are represented by one
`TransformOperatorComponent`, evaluated by `TransformOperatorSystem`. The
[cleanup task](../task/transform-operator-api-cleanup.md) records migration status
and the manual-test checkpoint before changing AVC capsule-height behavior.

## Input, output, and nesting

An ordinary operator consumes the world basis supplied by its parent and passes
its result to every direct child. Children are output content, including another
operator, a T, or a transparent component containing renderables. There is no
output marker or required first T. A downstream T composes its local TRS after
its parent's result; operators do not rewrite their parent transform.

```mms
T.position(1.0, 2.0, 3.0) {
    TOP.fork_trs().drop("rotation").drop("scale") {
        T.position(0.0, 0.5, 0.0) { R.cube() {} }
        TOP.extract_yaw() { T { R.cube() {} } }
    }
}
```

The camera-specific settings form is an explicit exception described below.
Referenced transforms are data sources, not additional topology parents.

## Constructors and builders

| MMS operation | Configuration and behavior |
| --- | --- |
| `TOP.fork_trs()` | Decompose the inherited world matrix into translation, quaternion rotation, and scale; apply ordered channel operations; recompose. Unspecified channels pass. |
| `.drop("translation" / "rotation" / "scale")` | Replace that channel's operation list with Drop: zero translation, identity rotation, or unit scale. |
| `.pass(channel)` | Replace that channel's operation list with Pass. Repeated pass/drop settings use the last setting. |
| `TOP.smooth(channel, factor)` or `.smooth(channel, factor)` | Append temporal smoothing to the selected channel. Factor is finite and non-negative; it is the existing exponential smoothing coefficient, not a frequency in Hz. Rotation uses the existing quaternion filter. |
| `TOP.sample_ancestor("translation" / "rotation", skip)` or matching builder | Append contextual sampling from an ancestor T's world transform. Skip is a non-negative integer: 0 = nearest ancestor T, 1 = next ancestor T. It does not count operator nodes. |
| `TOP.extract_yaw()` or `.extract_yaw()` | Append the existing quaternion Y projection, stripping pitch and roll. |
| `TOP.yaw_follow(threshold, rate)` or `.yaw_follow(threshold, rate)` | Append stateful body-yaw following. Threshold is radians, rate radians/second; both are finite and non-negative. `.initial_yaw(angle)` and `.forward_plus_z(bool)` configure the immediately preceding yaw-follow operation. |
| `TOP.parent(source)` | Route a referenced transform's world basis to downstream content. `.source(ref)` changes the source; `.root(ref)` scopes query resolution. `TOP.parent()` retains the existing unconfigured behavior. |
| `TOP.inverse_local(source)` | Postmultiply the inherited matrix by the inverse of a referenced T's authored local matrix: `input_world * inverse(source_local)`. `.source(ref)` changes the source. |
| `TOP.sample_translation().source(ref)` | Replace selected world translation coordinates from exactly one referenced T. `.axes("y")` or any nonempty set of x/y/z chooses coordinates; default xyz. `.root(ref)` scopes queries. Preserve the full incoming linear basis and apply downstream T offsets afterward. |
| `TOP.camera_specific("monoscopic" / "stereoscopic")` | Select a settings T under a marker attached directly to an anchor T. See the exception below. |

`TOP.drop(channel)` is also a constructor. It creates a fork with the selected
channel dropped. Other constructor shorthand for channel operations creates a
fork with that operation configured. Unknown channels, wrong kinds of builders,
negative coefficients/rates, and fractional ancestor counts are rejected.

Builders operate in order within each channel. Pass/drop reset the selected
channel's list; smooth, sample, and yaw operations append. The final lists are
stored on the live operator, not encoded as nested configuration components.
Translation, rotation, and scale remain independent. Configuration and temporal
state identity must not be confused: two operators with identical settings have
separate retained state.

## Sources and refresh

Sources accept component variables, query strings, and `"@uuid:..."` strings
through the existing ComponentRef machinery. A parent operator's optional root
scopes queries; GUID references resolve directly. Dumps retain referenced GUIDs
so a saved scene can resolve its sources again.

Inverse-local operators retain their last valid inverse when a source becomes
unresolved or singular. They reject sources within their own output subtree.
The existing reverse dependency index refreshes affected consumers when the
source changes. Parent routing retains its existing resolution, cycle protection,
and refresh behavior.

Translation sampling retains resolved handles and refreshes indexed consumers
when sources change. Query results invalidate on World selector/topology revision,
not ordinary typed pose updates. Missing, ambiguous, invalid, cyclic, or
non-finite selected sources preserve the input and expose a validation status.
Consumers such as AVC must disable contact while a sample is invalid/pending.
See the [axis-sampling task](../task/transform-sample-translation-axes.md) for
dependency limits and verification.

Source caching is not yet uniform across operator kinds. Parent routing still
resolves references and scans World for dependent parent operators during normal
T-change handling; multiple changes can trigger multiple scans in one frame.
Inverse-local already indexes consumers but re-resolves its reference when
evaluated. The [retained source bindings and invalidation task](../task/transform-operator-source-bindings-and-invalidation.md)
plans shared caching/indexed refresh and records the sampler's current broad
invalidation policy. This is transform-change-driven work, not an unconditional
per-frame operator-system scan.

## Camera-specific settings exception

Camera markers configure their parent anchor T. Their first direct T child is a
settings transform, not output content. The anchor's other children are outputs.
The settings remain live local transforms so the gizmo system can update them.

```mms
T {
    TOP.camera_specific("monoscopic") { T.scale(2.0, 2.0, 2.0) {} }
    TOP.camera_specific("stereoscopic") { T.scale(3.0, 3.0, 3.0) {} }
    R.cube() {}
}
```

The selected matrix is `anchor_world * selected_settings_local`. There is one
effective ECS matrix shared by rendering and picking, not a matrix per eye.
Repeated refresh does not compound settings. See
[camera-specific transforms](transform-camera-specific.md) for the full contract.
Moving these live settings into explicit references or typed values is deferred;
their exceptional role remains explicit at this API checkpoint.

## Runtime representation and state

Every TOP constructor creates a `TransformOperatorComponent` with a typed
`TransformOperatorKind`: `ForkTrs`, `Parent`, `SampleTranslation`, `InverseLocal`, or `CameraSpecific`.
Fork, parent-source, inverse-source, and camera settings are plain values inside
the enum. They are not separate ECS components. A live operator keeps its own
identity and temporal state, regardless of whether its settings equal another's.

The source modules are `ecs/component/transform_operator.rs` and
`ecs/system/transform_operator.rs`. The backend component name is
`transform_operator`; consumers inspect the typed kind to distinguish routing,
forking, and camera settings. Retired backend component names are not aliases.
Source queries should use durable GUIDs or labels when identifying a particular
operator. Kind-specific handling in topology refresh, attachments, source
collection, and transform mutation now matches that enum.

Fork settings own typed `TransformVec3Op` / `TransformQuatOp` lists.
`TransformOperatorSystem::evaluate_operator` evaluates borrowed live settings
and returns a processed matrix plus ordinary children. It does not traverse
configuration subtrees, clone channel lists, or allocate a plan on propagation.
`TransformOperatorPlan` remains available for standalone evaluation of snapshots.
`TransformSystem` handles hierarchy propagation, cached matrices, and renderer,
camera, light, and collision updates.

The former separate fork/parent/inverse/camera ECS types, map/drop/merge components,
and standalone filter/yaw/ancestor configuration components are removed. Rust
fixtures use operator constructors and values. Serialization reads those values
directly; there is no legacy configuration folding or fork-child exclusion helper.
Camera-specific markers retain the explicitly documented live-settings exception.

Temporal values live in TransformOperatorSystem, keyed by live owner, channel,
and operation path, and are not serialized. Existing dt handling, smoothing math,
yaw conventions, and source dependencies are preserved. Cleanup of stale temporal
entries and state reset after live operation-list edits remain follow-up work.
There is no per-frame MMS callback or new whole-world dependency scan.

## Validation and next work

Checks cover constructor/schema validation, nested outputs and downstream offset
order, source-driven refresh, GUID-preserving dump/reload, separate temporal state,
and independent channel/instance state. Ordinary forks expose every direct child
as output; camera refresh and render/picking tests remain applicable.

The TOP migration/consolidation passed the user’s smoke test. Axis-selective
sampling and XR capsule Y decoupling are implemented and have automated coverage.
The [AVC task](../task/avc-capsule-before-hmd-tracking.md) retains headset acceptance
and calibration limits. Crouch IK follows that manual checkpoint.
