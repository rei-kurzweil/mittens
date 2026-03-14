# VR input 2: current XR input wiring and speculative transform filtering

This doc is a follow-up to [docs/spec/vr-input.md](docs/spec/vr-input.md).

It covers two things:

1. **How VR input works currently** in the engine.
2. A **speculative design direction** for more declarative transform post-processing, especially temporal smoothing.

Scope:
- This doc is about the **current OpenXR pose flow** for controllers and hands.
- It also sketches a possible future transform pipeline for smoothing, filtering, and derived motion.
- It does **not** define button/trigger gameplay semantics yet.

---

## Current state: what the engine does today

At a high level, the engine currently has **two OpenXR pose sources** that can drive the same authored ECS shape:

- **controller pose actions** (`aim` / `grip`)
- **hand-tracking root pose** (currently chosen from `WRIST`, with `PALM` fallback)

Those sources are unified by `OpenXRSystem`, then applied to authored `ControllerXRComponent` nodes by updating a child `TransformComponent`.

Conceptually:

```text
OpenXR runtime
	-> OpenXRSystem::render_xr()
		-> cache hand root poses
		-> cache controller aim/grip poses
	-> OpenXRSystem::tick_with_queue()
		-> choose preferred pose source for each ControllerXRComponent
		-> update TransformComponent child
			-> renderables / raycasters / helpers move with that transform
```

---

## 1. Suggesting input profiles

### Current controller action setup

When an OpenXR session is created, `OpenXRSystem` initializes a controller action path:

- creates one `ActionSet`
- creates two pose actions:
	- `aim_pose`
	- `grip_pose`
- creates subaction paths for:
	- `/user/hand/left`
	- `/user/hand/right`
- creates action spaces for left/right aim and left/right grip
- attaches the action set to the session

This is the engine’s current **controller-style** OpenXR path.

### Profiles currently suggested

The engine currently suggests bindings for these interaction profiles:

- `/interaction_profiles/khr/simple_controller`
- `/interaction_profiles/oculus/touch_controller`
- `/interaction_profiles/htc/vive_controller`
- `/interaction_profiles/htc/vive_focus3_controller`
- `/interaction_profiles/valve/index_controller`
- `/interaction_profiles/microsoft/motion_controller`
- `/interaction_profiles/ext/hand_interaction_ext`

The suggested component paths currently wired are:

- `/user/hand/left/input/aim/pose`
- `/user/hand/right/input/aim/pose`
- `/user/hand/left/input/grip/pose`
- `/user/hand/right/input/grip/pose`

### Important semantics of “suggested profiles”

This is intentionally **best-effort**, not a guarantee.

OpenXR profile suggestion means:

- the engine tells the runtime which interaction profiles and component paths it knows how to use
- the runtime decides whether and how those bindings become active

So the runtime may:

- accept the suggestions
- ignore unsupported profiles
- bind some paths and not others
- expose no bound sources at all for the current device/runtime configuration

This is why the engine logs:

- current interaction profiles for left/right hands
- bound sources for `aim_pose` / `grip_pose`
- action active status

Those diagnostics matter because `sync_actions()` succeeding does **not** imply that useful pose sources were actually bound.

### Current role of `XR_EXT_hand_interaction`

Right now, `XR_EXT_hand_interaction` is only partially used:

- the extension is enabled if the runtime advertises it
- the hand interaction profile path is included in binding suggestions

But the engine does **not yet** consume hand-interaction-specific values like:

- `pinch_ext/value`
- `grasp_ext/value`
- `aim_activate_ext/value`
- `poke_ext/pose`

So the current engine state is:

- **controller actions are consumed directly**
- **raw hand tracking is consumed directly**
- **hand interaction profile support exists mainly as a compatibility/probing layer**

---

## 2. How we currently handle hand tracking

### Initialization

If the runtime advertises `XR_EXT_hand_tracking`, the engine enables the extension and then queries:

- `supports_hand_tracking(system)`

If that returns true, the engine creates:

- one `HandTracker` for the left hand
- one `HandTracker` for the right hand

### Per-frame joint query

During `OpenXRSystem::render_xr(...)`, after getting `predicted_display_time`, the engine locates joints for each hand relative to the OpenXR reference space:

```text
reference_space.locate_hand_joints(left_tracker, predicted_display_time)
reference_space.locate_hand_joints(right_tracker, predicted_display_time)
```

This is important:

- hands are sampled at the same predicted time as view poses
- hand tracking is queried in the same OpenXR reference space used by the XR camera path

### Root-pose selection

The engine currently does **not** yet build or expose the full hand skeleton into ECS.

Instead it collapses the hand-tracking result to a single “root pose” per hand.

Current selection rule:

1. use `WRIST` if its position and orientation are valid
2. otherwise use `PALM` if its position and orientation are valid
3. otherwise treat the hand root as unavailable

This gives a pragmatic first step:

- the existing “controller boxes” or hand debug boxes can move with tracked hands
- the engine can validate hand-tracking availability without first designing a whole hand-skeleton ECS layer

### Meaning of the current hand root

The chosen root pose is **not** the same thing as a controller grip pose.

That distinction matters:

- a controller `grip` pose is an interaction/runtime-defined holding pose
- a hand `palm` or `wrist` pose is anatomical joint data

So current hand-root behavior should be understood as:

- “use a stable tracked hand-root-ish pose to drive a transform”
- not “pretend OpenXR hand tracking already gives us a canonical controller-equivalent grip pose”

---

## 3. Current unification path: OpenXR API -> OpenXRSystem -> ControllerXRComponent -> TransformComponent

This is the core wiring that exists today.

### Authored ECS shape

Current authored pattern:

```text
ControllerXRComponent
	TransformComponent
		renderable / mesh / helper / ray origin subtree
```

Semantics:

- `ControllerXRComponent` declares **which hand** and **which pose kind** the subtree wants
- the child `TransformComponent` is the actual node that gets driven
- children under that transform move with the resolved XR pose

### What `ControllerXRComponent` means today

It still has controller-oriented fields:

- `hand: Left | Right`
- `pose: Aim | Grip`

But operationally it has become a more general **XR hand-side pose driver**.

Today, the pose source might be:

- controller action pose
- hand-tracking root pose

The authored component is still the same.

### Where sampling happens

Time-sensitive OpenXR work happens in `render_xr(...)`:

- wait for frame
- get `predicted_display_time`
- locate views
- update hand root caches
- sync controller actions
- locate controller action spaces
- update pose caches

This is the correct place for XR sampling because predicted display time is available there.

### Where world mutation happens

ECS mutation happens in `tick_with_queue(...)`:

- iterate registered `ControllerXRComponent`s
- choose a cached pose source
- find the child `TransformComponent`
- compose the pose under the active XR rig world matrix
- convert the desired world pose into local translation + local rotation
- write the transform
- emit `IntentValue::UpdateTransform`

This separation keeps the design clean:

- `render_xr` does runtime-facing XR sampling
- `tick_with_queue` does ECS-facing transform application

### Current source precedence

The current code-level precedence is:

1. **hand root pose**, if available for that hand
2. otherwise **controller action pose** matching `Aim` or `Grip`
3. otherwise no pose

This is worth stating clearly because it is easy to assume the inverse.

The user-observed behavior may still look like “controllers when controllers are active, hands when controllers are absent” if the runtime deactivates hand roots while controllers are in use.

But the actual code path is currently:

$$\text{preferred pose} = \text{hand root} \;\text{else}\; \text{controller action}$$

That means future configurability would be useful.

### Rig composition and local transform application

Once a pose is chosen, the engine composes it under the current XR rig transform:

$$M_{world} = M_{rig} \cdot M_{xr\_pose}$$

Then it derives the local transform for the target child transform:

- local translation by multiplying desired world position by the parent world inverse
- local rotation by removing the parent world rotation from the desired world rotation

So the resulting child transform behaves like any other authored transform in the hierarchy.

### Why this unification is useful

This design is already doing something important:

- the authored ECS topology does **not** need to care whether the source was controller actions or hand tracking
- both source types are converted into the same downstream representation: a driven `TransformComponent`

That means rendering, ray setup, helper meshes, and later gesture systems can all hang off the same subtree pattern.

---

## 4. What is unified today vs what is still separate

### Already unified

- left/right hand side selection
- transform driving into ECS
- rig-relative world composition
- authored subtree shape under `ControllerXRComponent`

### Still separate

- controller actions use profile suggestion + action spaces
- hand tracking uses raw joint location queries
- hand interaction action values are not yet consumed
- full hand skeleton / gestures are not yet exposed as ECS components

So today’s unification is a **pose-driver unification**, not yet a fully unified input abstraction.

---

## 5. Speculative next step: `TransformTemporalFilter`

The current transform path is intentionally raw:

- sampled pose
- immediate transform write

That is good for correctness and debugging, but it leaves no declarative way to express:

- smoothing
- lag
- springy follow behavior
- motion damping
- overshoot / jiggle-like temporal response

This suggests a new concept:

- `TransformTemporalFilterComponent`

### Intent of `TransformTemporalFilterComponent`

Unlike `TransformFilterComponent`, which is about **which channels a subtree inherits from its parent**, a temporal filter would be about **how a transform stream evolves over time**.

In short:

- `TransformFilterComponent` is a **spatial / inheritance filter**
- `TransformTemporalFilterComponent` is a **time-domain filter**

That distinction is important.

### Minimum useful version

A pragmatic first version could operate on a full transform stream:

```rust
pub struct TransformTemporalFilterComponent {
		pub smooth_translation: Option<f32>,
		pub smooth_rotation: Option<f32>,
		pub smooth_scale: Option<f32>,
}
```

or, more explicitly:

```rust
pub enum TemporalFilterMode {
		Exponential,
		CriticallyDamped,
		Spring,
}

pub struct TransformTemporalFilterComponent {
		pub translation_mode: Option<TemporalFilterMode>,
		pub rotation_mode: Option<TemporalFilterMode>,
		pub scale_mode: Option<TemporalFilterMode>,
		pub translation_strength: f32,
		pub rotation_strength: f32,
		pub scale_strength: f32,
}
```

This would already help with:

- slightly noisy hand tracking
- controller ray stabilization
- camera-relative helper transforms
- dynamic secondary motion

---

## 6. How `TransformTemporalFilter` could work with `TransformFilter`

These two concepts are complementary.

### `TransformFilterComponent`

From the earlier transform docs, `TransformFilterComponent` means:

- translation inheritance can be passed through or removed
- rotation inheritance can be passed through or removed
- scale inheritance can be passed through or removed

This is about **what transform basis is passed into a subtree**.

### `TransformTemporalFilterComponent`

A temporal filter would mean:

- given an input transform stream over time
- produce a smoothed / lagged / spring-like output transform stream

This is about **how a transform changes from frame to frame**.

### Together

The combined mental model would be:

1. spatially shape the inherited transform basis
2. temporally shape the motion over time

Conceptually:

```text
parent world transform
	-> TransformFilterComponent
		-> filtered transform basis
			-> TransformTemporalFilterComponent
				-> smoothed output transform
					-> child visuals / helpers / bones
```

That makes sense for gizmos, XR hands, and secondary motion.

Examples:

- **gizmo visuals**: inherit translation + rotation, drop scale, no temporal smoothing
- **XR hand proxy**: inherit normally, but smooth rotation slightly
- **jiggle helper chain**: inherit a filtered basis, then use spring temporal filtering

---

## 7. Is this really a “map”, or is it a “fork”? 

This is the right question.

If we want to independently process translation, rotation, and scale, then a single `TransformMap` node is probably too vague.

Splitting TRS is more naturally a **fork**:

```text
Transform
	-> fork into T / R / S streams
		-> process each stream independently
	-> merge back into a Transform
```

So the more precise conceptual operators are probably:

- **TransformForkTRS**
- **TransformMapTranslation**
- **TransformMapRotation**
- **TransformMapScale**
- **TransformMergeTRS**

That is a cleaner model than saying “TransformMap splits T/R/S”, because splitting is not mapping — it is branching.

---

## 8. A speculative declarative transform pipeline

Using the user’s example as motivation, the more general shape might be:

```text
ControllerXRComponent
	raw_transform (TransformComponent)
		TransformForkTRS
			TranslationStream
			RotationStream
				RotationTemporalSmooth
			ScaleStream
		TransformMergeTRS
			filtered_transform (TransformComponent)
				renderable subtree
```

Or, in a more compact authoring style:

```text
ControllerXR
	T(raw)
		TransformForkTRS
			TransformMapTranslation(identity)
			TransformMapRotation(TemporalSmooth)
			TransformMapScale(identity)
		TransformMergeTRS
			T(filtered)
				hand mesh / debug box / ray origin
```

### Why this is attractive

Because the same structure could express more than hand smoothing:

- controller ray stabilization
- dynamic bones
- antenna / tail / ear lag
- jiggle physics proxies
- camera-follow anchors
- stabilized UI attachments

This is much broader than OpenXR.

---

## 9. A practical caution: start simpler than the full fork/merge graph

The full fork/map/merge graph is expressive, but it is also a much bigger design step.

A pragmatic evolution path would be:

### Phase 1

Add one simple stateful component:

- `TransformTemporalFilterComponent`

that directly smooths some or all of:

- translation
- rotation
- scale

### Phase 2

Let it operate per-channel more explicitly:

- translation-only smoothing
- rotation-only smoothing
- scale-only smoothing

### Phase 3

If that proves useful, generalize toward:

- `TransformForkTRS`
- channel processors
- `TransformMergeTRS`

This reduces design risk.

In other words:

- **first build the useful temporal behavior**
- **later generalize the authoring model if we still want the graph abstraction**

---

## 10. Recommended terminology

To keep the model clear, the following distinctions seem useful:

- **TransformFilter**: filters inherited parent transform channels for a subtree
- **TransformTemporalFilter**: filters a transform stream over time
- **TransformForkTRS**: splits transform into translation / rotation / scale channels
- **TransformMergeTRS**: recombines channels into a transform
- **TransformMapTranslation/Rotation/Scale**: applies per-channel processing

This keeps:

- topology / inheritance concerns
- temporal concerns
- channel-routing concerns

as separate concepts.

---

## 11. Summary

Current engine behavior:

- suggests a set of controller and hand-interaction profiles to the OpenXR runtime
- consumes controller `aim` / `grip` poses via action spaces when available
- consumes raw hand tracking via `XR_EXT_hand_tracking`
- reduces hand tracking to a per-hand root pose (`WRIST`, fallback `PALM`)
- unifies both sources in `OpenXRSystem`
- drives a child `TransformComponent` under each `ControllerXRComponent`

This is already a useful unification point.

Speculative next direction:

- add `TransformTemporalFilterComponent` for time-domain smoothing / spring response
- keep it conceptually separate from `TransformFilterComponent`
- consider a future transform processing pipeline with TRS fork / per-channel filtering / merge
- treat TRS splitting as a **fork**, not merely a map

That would give the engine a more general declarative motion-processing vocabulary that could be used for:

- XR hands/controllers
- gizmos
- dynamic bones
- jiggle-style motion
- secondary follow rigs
