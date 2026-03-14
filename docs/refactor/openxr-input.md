## 21. Addendum: `XR_EXT_hand_tracking` and HTC Vive Focus 3 support

This section extends the earlier guidance. It does **not** replace the action/profile-based controller path; it adds the recommended hand-tracking and HTC-specific notes needed for a game engine refactor.

### Summary

For a game engine with:
- OpenXR session setup working
- HMD pose tracking working
- rendering working

the recommended input architecture is:

1. **Keep controller input action-based**
   - use interaction profiles
   - use action poses/buttons/haptics
2. **Add `XR_EXT_hand_tracking` as the default raw hand path**
   - query joint poses
   - derive gestures yourself
3. **Optionally support `XR_EXT_hand_interaction`**
   - as a convenience / supplemental path
4. **Do not assume the runtime reports the ideal profile name**
   - always query the active interaction profile at runtime

This keeps controller support robust while making hand input more portable and engine-controlled.

---

## 22. Why keep both action-based controllers and hand tracking?

These are complementary:

### Controller path
Use OpenXR actions and interaction profiles for:
- aim pose
- grip pose
- trigger/click/squeeze
- thumbstick / touchpad
- haptics

This is the normal OpenXR controller workflow and should remain your primary path for motion controllers.

### Hand tracking path
Use `XR_EXT_hand_tracking` for:
- hand joint poses
- palm/wrist/finger tip transforms
- gesture derivation such as:
  - pinch
  - point
  - fist/grab
  - poke

This is a different source of input data and should not replace controller actions. It should sit alongside them.

---

## 23. `XR_EXT_hand_tracking` does not return a matrix tree

Important implementation note:

`XR_EXT_hand_tracking` does **not** return:
- a hierarchy
- parent-relative bones
- a tree of `mat4`

It returns a fixed set of **joint locations** for a hand skeleton.

Each joint gives you:
- a pose
- a radius
- validity/tracking flags

From that you can build:
- a **global joint matrix array**
- optionally a **local parent-relative matrix array**
- gesture state derived from distances/angles

So the engine-side output should look more like:

```rust
#[derive(Debug)]
struct HandSkeletonState {
    tracked: bool,
    joints_global: Vec<glam::Mat4>,
    joints_local: Vec<glam::Mat4>,
    radii: Vec<f32>,
}
```

---

## 24. Runtime extension checks to add

When initializing OpenXR, query and record:

```rust
fn print_input_related_extensions(exts: &xr::ExtensionSet) {
    println!("EXT_hand_tracking: {}", exts.ext_hand_tracking);
    println!("EXT_hand_interaction: {}", exts.ext_hand_interaction);
    println!("EXT_eye_gaze_interaction: {}", exts.ext_eye_gaze_interaction);
    println!(
        "EXT_hp_mixed_reality_controller: {}",
        exts.ext_hp_mixed_reality_controller
    );
}
```

For the refactor, the engine should track capability booleans like:

```rust
#[derive(Debug, Default)]
struct InputCapabilities {
    hand_tracking: bool,
    hand_interaction: bool,
    eye_gaze: bool,
    left_controller_pose: bool,
    right_controller_pose: bool,
    active_left_profile: Option<String>,
    active_right_profile: Option<String>,
}
```

---

## 25. Creating hand trackers with `XR_EXT_hand_tracking`

Conceptually, once `XR_EXT_hand_tracking` is enabled, create one tracker per hand:

```rust
use openxr as xr;

struct HandTrackers {
    left: xr::HandTrackerEXT,
    right: xr::HandTrackerEXT,
}

fn create_hand_trackers<G>(
    session: &xr::Session<G>,
) -> anyhow::Result<HandTrackers> {
    let left = session.create_hand_tracker(
        xr::HandEXT::LEFT,
        xr::HandJointSetEXT::DEFAULT,
    )?;

    let right = session.create_hand_tracker(
        xr::HandEXT::RIGHT,
        xr::HandJointSetEXT::DEFAULT,
    )?;

    Ok(HandTrackers { left, right })
}
```

Exact type/method names may vary slightly depending on the `openxr` crate version, but the engine logic should be structured around:
- left tracker
- right tracker
- per-frame joint query

---

## 26. Querying joints every frame

Each frame, query joint locations relative to your reference space:

```rust
fn locate_hand_joints(
    tracker: &xr::HandTrackerEXT,
    base_space: &xr::Space,
    time: xr::Time,
) -> anyhow::Result<Vec<xr::HandJointLocationEXT>> {
    let joints = tracker.locate_joints(base_space, time)?;
    Ok(joints)
}
```

Use the same style of reference space that the rest of the engine uses for tracked input, typically:
- stage/local/floor space
- whichever is already used for HMD/controller poses

The agent should treat hand joint queries similarly to controller pose queries:
- valid/tracked flags determine whether the data is usable
- pose data is then converted into engine transform types

---

## 27. Converting hand joint poses into engine matrices

Joint poses should be converted into matrices explicitly.

Example with `glam::Mat4`:

```rust
use glam::{Mat4, Quat, Vec3};
use openxr as xr;

fn pose_to_mat4(pose: xr::Posef) -> Mat4 {
    let t = Vec3::new(pose.position.x, pose.position.y, pose.position.z);
    let r = Quat::from_xyzw(
        pose.orientation.x,
        pose.orientation.y,
        pose.orientation.z,
        pose.orientation.w,
    );
    Mat4::from_rotation_translation(r, t)
}
```

Example with `Affine3A`:

```rust
use glam::{Affine3A, Quat, Vec3};
use openxr as xr;

fn pose_to_affine(pose: xr::Posef) -> Affine3A {
    let t = Vec3::new(pose.position.x, pose.position.y, pose.position.z);
    let r = Quat::from_xyzw(
        pose.orientation.x,
        pose.orientation.y,
        pose.orientation.z,
        pose.orientation.w,
    );
    Affine3A::from_rotation_translation(r, t)
}
```

The engine should store:
- one global matrix per joint
- optionally local matrices derived from parent-child relationships

---

## 28. Building a local hand skeleton tree

If the engine wants parent-relative joints for animation or gesture processing, compute them from global matrices:

```rust
fn local_from_global(parent_global: glam::Mat4, child_global: glam::Mat4) -> glam::Mat4 {
    parent_global.inverse() * child_global
}
```

So:
- OpenXR hand tracking provides **global joint poses in the chosen reference space**
- engine derives **local joint transforms** if needed

The coding agent should not assume OpenXR gives a ready-made transform tree.

---

## 29. Debug printing hand data

For debugging, the engine should initially print:
- wrist
- palm
- thumb tip
- index tip
- middle tip
- pinch distance

That gives enough signal to verify the system without overwhelming logs.

Example subset print:

```rust
fn print_joint_debug(joints: &[xr::HandJointLocationEXT]) {
    let interesting = [
        xr::HandJointEXT::PALM,
        xr::HandJointEXT::WRIST,
        xr::HandJointEXT::THUMB_TIP,
        xr::HandJointEXT::INDEX_TIP,
        xr::HandJointEXT::MIDDLE_TIP,
    ];

    for joint in interesting {
        let idx = joint as usize;
        let j = &joints[idx];

        let pos_valid = j.location_flags.contains(xr::SpaceLocationFlags::POSITION_VALID);
        let ori_valid = j.location_flags.contains(xr::SpaceLocationFlags::ORIENTATION_VALID);
        let pos_tracked = j.location_flags.contains(xr::SpaceLocationFlags::POSITION_TRACKED);
        let ori_tracked = j.location_flags.contains(xr::SpaceLocationFlags::ORIENTATION_TRACKED);

        println!(
            "{joint:?}: pos_valid={} ori_valid={} pos_tracked={} ori_tracked={} pos=({:.3}, {:.3}, {:.3}) radius={:.4}",
            pos_valid,
            ori_valid,
            pos_tracked,
            ori_tracked,
            j.pose.position.x,
            j.pose.position.y,
            j.pose.position.z,
            j.radius,
        );
    }
}
```

Full dump if needed:

```rust
fn print_all_joints(joints: &[xr::HandJointLocationEXT]) {
    for (i, j) in joints.iter().enumerate() {
        println!(
            "joint[{i}] pos=({:.3}, {:.3}, {:.3}) rot=({:.3}, {:.3}, {:.3}, {:.3}) radius={:.4} flags={:?}",
            j.pose.position.x,
            j.pose.position.y,
            j.pose.position.z,
            j.pose.orientation.x,
            j.pose.orientation.y,
            j.pose.orientation.z,
            j.pose.orientation.w,
            j.radius,
            j.location_flags,
        );
    }
}
```

---

## 30. Debug printing matrices

If the engine already uses matrices internally, also print selected joint matrices:

```rust
fn print_joint_matrix(name: &str, pose: xr::Posef) {
    let m = pose_to_mat4(pose);
    println!("{name} = {m:?}");
}
```

This is useful for validating:
- handedness
- coordinate system assumptions
- model-space vs reference-space interpretation

---

## 31. Deriving gestures from `XR_EXT_hand_tracking`

The reason to add `XR_EXT_hand_tracking` is not just pose display — it is gesture derivation.

### Pinch detection
Use thumb tip and index tip distance:

```rust
fn pinch_distance(joints: &[xr::HandJointLocationEXT]) -> f32 {
    let thumb = &joints[xr::HandJointEXT::THUMB_TIP as usize];
    let index = &joints[xr::HandJointEXT::INDEX_TIP as usize];

    let a = glam::Vec3::new(
        thumb.pose.position.x,
        thumb.pose.position.y,
        thumb.pose.position.z,
    );
    let b = glam::Vec3::new(
        index.pose.position.x,
        index.pose.position.y,
        index.pose.position.z,
    );

    a.distance(b)
}
```

Suggested early debug:

```rust
let d = pinch_distance(&joints);
println!("left pinch distance = {d:.4} m");
```

Typical first-pass threshold:
- around `0.02` to `0.03` meters

### Point detection
Possible heuristic:
- index finger extended
- other fingers curled
- index tip sufficiently forward from palm
- fingertip chain relatively straight

### Grab/fist detection
Possible heuristic:
- all fingertips close to palm
- finger chains folded
- average tip-to-palm distance below threshold

The exact gesture math should live in the engine’s own hand-input layer rather than being tied to OpenXR API code.

---

## 32. Relationship to `XR_EXT_hand_interaction`

`XR_EXT_hand_interaction` still has value and should remain in the report.

Recommended interpretation:

- **Use `XR_EXT_hand_tracking` as the default raw hand path**
- **Use `XR_EXT_hand_interaction` as a supplemental convenience path**

Why:
- hand tracking gives raw joints and broad engine control
- hand interaction gives runtime-defined semantics when available
- supporting both improves compatibility and flexibility

So the agent should not remove hand-interaction support just because hand tracking is added.

---

## 33. HTC Vive Focus 3 controller profile

For controller support, there is a **specific OpenXR interaction profile** for Vive Focus 3 controllers:

- `/interaction_profiles/htc/vive_focus3_controller`

This should be treated as the primary HTC Focus 3 controller profile when suggesting controller bindings.

Typical bindings are expected to include:
- left/right aim pose
- left/right grip pose
- trigger
- squeeze
- thumbstick / trackpad depending on runtime mapping
- haptics

Example conceptual profile use:

```rust
let profile = instance.string_to_path("/interaction_profiles/htc/vive_focus3_controller")?;
```

The engine should add this profile to its supported controller profile list.

---

## 34. Important runtime caveat for Focus 3

Do **not** assume that a Vive Focus 3 runtime will always report the active profile as:

- `/interaction_profiles/htc/vive_focus3_controller`

In practice, some runtimes may expose fallback HTC controller profiles instead.

So the correct logic is:

1. suggest bindings for the Focus 3 profile
2. optionally suggest bindings for fallback HTC profiles you want to support
3. query the current active interaction profile at runtime
4. trust the runtime’s active profile result, not the headset name alone

This matters especially when testing against different runtimes or compatibility layers.

---

## 35. Printing the active interaction profile for each hand

The engine should log the active profile at runtime for `/user/hand/left` and `/user/hand/right`.

Example:

```rust
fn print_current_profile<G>(
    instance: &xr::Instance,
    session: &xr::Session<G>,
    user_path_str: &str,
) -> anyhow::Result<()> {
    let user_path = instance.string_to_path(user_path_str)?;
    let profile = session.current_interaction_profile(user_path)?;

    if profile == xr::Path::NULL {
        println!("{user_path_str}: no active interaction profile");
    } else {
        let s = instance.path_to_string(profile)?;
        println!("{user_path_str}: active profile = {s}");
    }

    Ok(())
}
```

Recommended early debug:

```rust
print_current_profile(&instance, &session, "/user/hand/left")?;
print_current_profile(&instance, &session, "/user/hand/right")?;
```

This is especially important for:
- Oculus support
- Vive Focus 3 support
- runtime compatibility debugging
- deciding which bindings are actually live

---

## 36. Recommended engine-side input precedence

For the refactor, the coding agent should structure input source precedence roughly as:

1. **Tracked controllers active**
   - use controller action poses/buttons/haptics
2. **No active controllers, but hand tracking available**
   - use `XR_EXT_hand_tracking`
   - derive gestures and hand pointer pose
3. **Hand interaction available**
   - optionally merge/supplement semantics from hand interaction actions
4. **No tracked hand/controller input**
   - fall back to eye/HMD-based pointer if desired

This avoids coupling the engine too tightly to any single runtime-specific hand interaction path.

---

## 37. Recommended data flow for the refactor

The current refactor already has:
- HMD pose tracking
- XR rendering

The next input stages should probably be organized as:

### Stage A: Controller path
- action set
- controller profile suggestions
- pose spaces
- per-frame action sync
- per-frame controller pose/action extraction

### Stage B: Hand tracking path
- enable `XR_EXT_hand_tracking`
- create left/right hand trackers
- locate joints each frame
- convert joint poses into engine matrices
- compute gestures
- expose semantic hand state to gameplay/UI

### Stage C: Debug/diagnostics
- print extension support
- print active interaction profiles
- print selected joint debug info
- print pinch distance and/or a few matrices

This gives the coding agent a clean incremental plan instead of mixing all input modes together.

---

## 38. Practical recommendation for the agent

The coding agent should aim for these output-facing engine abstractions:

```rust
#[derive(Debug, Default)]
struct ControllerState {
    tracked: bool,
    aim_pose: glam::Mat4,
    grip_pose: glam::Mat4,
    trigger_value: f32,
    squeeze_value: f32,
}

#[derive(Debug, Default)]
struct HandState {
    tracked: bool,
    joints_global: Vec<glam::Mat4>,
    radii: Vec<f32>,
    pinch_distance: f32,
    is_pinching: bool,
    is_pointing: bool,
}

#[derive(Debug, Default)]
struct XrInputState {
    hmd_pose: glam::Mat4,
    left_controller: ControllerState,
    right_controller: ControllerState,
    left_hand: HandState,
    right_hand: HandState,
}
```

This keeps the engine interface stable even if the low-level OpenXR input plumbing changes.

---

## 39. Combined conclusion

The report’s earlier action/profile guidance still stands.

The additions for the refactor are:

- keep **controller input** action-based
- add **`XR_EXT_hand_tracking`** for raw hand joints
- derive gestures yourself from joint data
- treat hand tracking as joint poses, **not** a ready-made matrix tree
- add support for **`/interaction_profiles/htc/vive_focus3_controller`**
- do not assume runtimes always report the ideal HTC profile
- always print/query the active interaction profile at runtime

This combined approach gives the broadest and most engine-friendly foundation for:
- Oculus support
- Vive Focus 3 support
- custom gesture handling
- controller + hand coexistence
- long-term OpenXR portability