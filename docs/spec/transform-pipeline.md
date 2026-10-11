# Transform operators

The MMS API is `TOP`, short for `TransformOperator`. Constructors choose the
operation; builder calls configure it. The filename and Rust evaluator names
retain historical pipeline/stream terminology. The
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
and refresh behavior. This migration changes authoring, not those policies.

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

The grouped MMS surface currently lowers to existing Rust components:
`TransformForkTRSComponent`, `TransformParentComponent`,
`TransformApplyInverseLocalComponent`, and `TransformCameraSpecificComponent`.
A single Rust TransformOperatorComponent and backend terminology renames remain
separate cleanup work. Backend component names/selectors are unchanged.

Fork components own typed `TransformVec3Op` / `TransformQuatOp` lists.
`TransformStreamSystem` parses them into `TransformPipelinePlan` /
`TransformForkTrsStage`, evaluates them, and returns the result plus downstream
children. `TransformSystem` handles hierarchy propagation, cached world matrices,
rendering, camera/light/collision updates, and continuing through nested operators.
This is an ordered plan over one inherited basis, not a second general graph.

Temporal values live in TransformStreamSystem, keyed by live owner and channel /
operation path, and are not serialized. The existing dt handling, smoothing math,
yaw conventions, and source dependency behavior are retained. Translation and
scale smoothing now have distinct channel keys. Cleanup of stale temporal entries
and resetting state after live operation-list edits remain follow-up work; they
are not completed by this namespace migration.
There is no per-frame MMS callback or new whole-world dependency scan.

Legacy Rust map/drop/filter/merge types remain for compatibility with Rust
fixtures and plans. Generated AVC capsule/body operators and gizmo operators now
use value configuration. Fork serialization folds legacy configuration children
into TOP builders and omits those helpers from the dumped output tree.
The separate legacy MMS component spellings are no longer registered.
After the user's smoke test, the next slice removes those Rust helper components
and their parser/dump exclusions, migrating remaining Rust fixtures to value
settings. The compatibility filtering is transitional, not the desired final
output contract.

## Validation and next work

Checks cover constructor/schema validation, nested outputs and downstream offset
order, source-driven refresh, GUID-preserving dump/reload, separate temporal state,
and old-map versus value-configured output across multiple frames. Existing camera
refresh and render/picking tests remain applicable.

The user will test this API migration before further transform/locomotion work.
`TOP.sample_translation().source(...).axes(...)` is still a proposal in the
[axis-sampling task](../task/transform-sample-translation-axes.md). AVC capsule
height is still driven by the current topology; the HMD Y contact issue and
crouch IK are intentionally separate later slices.
