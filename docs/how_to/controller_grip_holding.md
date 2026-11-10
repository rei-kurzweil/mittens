# Holding tools from the OpenXR grip

`Pointer.grab_pose("controller_grip")` uses the current OpenXR grip action pose
for `Grabbable.to_hand(true)` objects. This remains an opt-in experiment;
paper-craft uses the original hand-bone holding mode after an in-headset test
showed incorrect tool orientation with controller-grip holding.
The pointer ray can still come from a `RestAttachment` on the avatar hand; choosing
a holding pose does not change ray aiming or the avatar IK target.

```javascript
XRHand.new(true,"Right","GripAim").laser() {
    T {
        RestAttachment.new("[name='J_Bip_R_Hand']","[name='J_Bip_R_Middle3']") {
            Pointer.grab_pose("controller_grip") {}
        }
    }
}
```

The default remains `grab_pose("hand_bone")`, retaining the existing explicit
`grab_attachment` or AVC-provided hand attachment. `Grabbable.hand_offset` applies
to that hand-bone mode. Controller-grip mode aligns the prop's authored grip origin
directly with the controller pose, without the wrist correction. Objects ease into
the grip initially; once settled, controller movement does not incur another layer
of smoothing. Authored object scale is retained.

Controller-grip mode requires a controller ancestor and a valid raw grip sample.
It does not substitute aim orientation, solved hand bones, or a stale sample when
grip tracking is unavailable. New grabs require a tracked grip; held objects retain
their last pose relative to the rig until tracking returns.

For script experiments, `controller.get_grip_pose()` returns a copied world-space
TRS table with `translation`, `rotation_quat_xyzw`, and `scale`, compatible with
`Transform.world_trs(...)`. It returns `null` when untracked or disabled:

```javascript
let grip=controller.get_grip_pose()
if grip!=null {tool.world_trs(grip)}
```

Rust uses the same cached sample through
`ControllerXRComponent::get_grip_pose(&World) -> Option<TransformTrs>`.
The engine's native grab path calls this directly; it does not run per-frame MMS
callbacks. The copy follows rig movement while remaining independent of avatar
hand-bone corrections. Tracking caches are cleared each XR frame.

Automated tests use distinct aim, grip, and hand-bone poses and cover native holding,
MMS copying, tracking loss, and scale preservation. The black gohei still needs an
in-headset grip check.
