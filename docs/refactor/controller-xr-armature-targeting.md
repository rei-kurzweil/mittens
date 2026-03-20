# ControllerXR armature targeting for VTuber hands

This note explores how XR controller / hand-tracking input should drive a **subset of an imported armature** (for example the VTuber model’s hands) without changing `src/` or examples yet.

The immediate motivation is:
- `examples/vr-input.mms` shows that transform-pipeline smoothing works well for **controller proxy objects**,
- but hand-tracking rotation still does not feel smooth in the same way,
- while translation feels smooth enough,
- and the next likely target is not free-floating cubes, but the VTuber model’s hand/arm bones.

The main conclusion is:

> The current `ControllerXRComponent` semantics are good for “drive my child transform,” but they are the wrong semantic shape for “drive an existing subtree inside a spawned glTF armature.”

So this is primarily a **topology / semantics problem**, not just a filter-tuning problem.

---

## 1. What the current setup is good at

Today the engine’s XR pose path is roughly:

- `OpenXRSystem` samples controller actions and/or hand-root poses,
- it resolves one pose for each `ControllerXRComponent`,
- it finds a **child `TransformComponent`**,
- it updates that child transform,
- any authored `TransformPipeline` under that transform can smooth channels before driving visible children.

That fits the topology used in `examples/vr-input.mms`:

```text
ControllerXR
  Transform
    TransformPipeline
      TransformForkTRS
        TransformMapTranslation
        TransformMapRotation
          QuatTemporalFilter
        TransformMapScale
        TransformMergeTRS
      TransformPipelineOutput
        Transform
          cube renderable
```

This is a very good fit for:
- controller gizmos,
- held tools,
- rays / pointers,
- debug cubes,
- helper anchors,
- any other “tracked object owns its presentation subtree” pattern.

The reason smoothing works well here is that the topology is simple:
- one XR pose source,
- one driven transform,
- one short presentation subtree.

---

## 2. Why this shape breaks down for VTuber hands

A glTF / VRM avatar is not authored as:

```text
ControllerXR
  Transform
    ... hand bones ...
```

It is authored/spawned more like:

```text
Transform (avatar root)
  GLTF
  ... imported transform tree ...
    arm
      hand
        finger bones ...
```

Important constraints:
- the armature hierarchy already exists,
- `SkinnedMeshSystem` expects those transforms to stay in the imported hierarchy,
- we should not disrupt that hierarchy just to insert `ControllerXR`,
- a hand controller should probably affect **some subset** of the armature, not “whatever children happen to be under a controller node.”

So if we try to reuse current `ControllerXR` semantics literally, we get an awkward mismatch:
- `ControllerXR` wants to be a **driver-parent**,
- but the avatar hand bones are already **embedded children** of the avatar skeleton.

That means “nest `ControllerXR` under part of the armature” is semantically confusing, and “make armature bones children of `ControllerXR`” is structurally wrong.

---

## 3. Why controller rotation can feel smoother than hand-tracking rotation

This is not just one thing, but a combination of factors.

## 3.1 Controller poses are already interaction-oriented

A controller `Aim` / `Grip` pose is usually:
- device-defined,
- runtime-stabilized,
- intended for interaction,
- relatively rigid.

That makes it a good source for a simple `QuatTemporalFilter`.

By contrast, hand tracking currently uses a reduced hand-root pose (`wrist` / `palm`) that is:
- anatomical,
- noisier,
- not necessarily aligned to a desired held-object or avatar-hand orientation,
- not equivalent to a stable grip frame.

So even with the same filter, the input signal itself is not the same kind of thing.

## 3.2 The current hand path is still a single-root approximation

The current VR input doc explicitly describes hand tracking as:
- raw joints reduced to a single root-ish pose for now,
- not a full articulated hand/armature drive.

That means current hand rotation smoothing is trying to stabilize a **single anatomical root pose**, not a semantically richer hand rig.

If what we want is a believable avatar hand, the right target is likely:
- a hand / forearm subtree,
- or a mapped subset of armature joints,
- not a single proxy transform.

## 3.3 Translation and rotation tolerate noise differently

The user observation matches a common pattern:
- translation can look “fine” with modest jitter,
- rotation jitter is much more visually obvious,
- especially on hands, wrists, and held-object orientation.

So it makes sense that translation feels acceptable while rotation does not.

---

## 4. What we probably want instead

The more useful long-term feature is:

> Each XR hand/controller should be able to drive a named or resolved subset of the avatar armature.

For example:
- left controller / hand tracking drives left wrist + left hand + left finger subtree,
- right controller / hand tracking drives right wrist + right hand + right finger subtree.

This implies a different semantic contract:
- `ControllerXR` should behave more like a **pose source**,
- while some other layer defines **where that pose gets applied**.

That is very different from the current “find my child transform and drive it” behavior.

---

## 5. Key semantic mismatch to solve

Today:
- `ControllerXR` is effectively a **driver component with implicit local target = one child transform**.

For avatar hands we probably need:
- a **pose-source component**, and
- an explicit **targeting / retargeting / output-routing** mechanism.

This is the real refactor seam.

---

## 6. Transform-pipeline implications

The transform pipeline is still the right place for smoothing and channel shaping.

But it is currently being used in a topology where:
- the input is inherited parent-world transform,
- and the output typically drives authored descendants under the pipeline node.

For armature targeting, the desirable shape is different:
- XR pose source exists outside the avatar skeleton,
- pipeline filters/processes that pose,
- pipeline output should target one or more **existing transform nodes inside the imported skeleton**.

So the interesting question is not “do we use the transform pipeline?”

It is:

> How does a pose/pipeline output address transforms that already exist elsewhere in the ECS tree?

That is where explicit targeting semantics become necessary.

---

## 7. Likely design direction: `ControllerXR` as pose source, not subtree owner

A cleaner model would be:

```text
ControllerXR = pose source
TransformPipeline = optional filter / mapper / retargeter
Target binding = explicit destination(s) inside avatar armature
```

That makes `ControllerXR` conceptually closer to:
- `InputComponent`
- `RayCastComponent`
- other source-ish components

rather than:
- “a transform parent that owns the presentation subtree.”

### Why this is better

It lets us say:
- “left XR hand provides a pose stream,”
- “apply rotation smoothing,”
- “drive the avatar’s left hand chain,”

without requiring any hierarchy surgery on the imported glTF skeleton.

---

## 8. What explicit target semantics might look like

There are a few plausible directions.

## Option A: `ControllerXR` keeps current child-drive semantics, and we add a separate armature driver component

Conceptually:

```rust
HandArmatureTargetComponent {
    source: XrLeftHand,
    root_bone: "J_Bip_L_Hand",
    mode: HandSubtree,
}
```

Pros:
- preserves current `ControllerXR` behavior for cubes/tools/rays,
- keeps avatar-armature driving as a separate concern,
- clearer migration path.

Cons:
- now there are two different XR pose-consumer semantics in the engine.

## Option B: generalize `ControllerXR` so it can target external transforms

Conceptually:

```rust
ControllerXRTargetMode {
    ChildTransform,
    ExplicitTransform(ComponentId),
    ArmatureSemantic(AvatarHandLeft),
}
```

Pros:
- one concept handles both proxy objects and avatar targets.

Cons:
- `ControllerXR` becomes overloaded,
- mixes source selection with target resolution,
- awkward for imported glTF hierarchies whose `ComponentId`s are runtime-generated.

## Option C: introduce a generic pose-routing layer

Conceptually:

```text
ControllerXR / HandTrackingPoseSource
  -> TransformPipeline
  -> TransformTarget / ArmatureRetargetTarget
```

Pros:
- cleanest semantics,
- scales to more sources than XR,
- makes pipeline outputs explicit.

Cons:
- bigger conceptual refactor.

### Recommended direction

For clarity, the best long-term direction is closest to **Option C**.

For minimum disruption, the most practical first step may be **Option A**:
- keep `ControllerXR` child-driving semantics for current use cases,
- add a new explicit avatar/armature-targeting concept for imported skeletons.

---

## 9. Why direct nesting under armature is the wrong abstraction

The temptation is something like:

```text
... arm bone ...
  ControllerXR
    hand bone subtree
```

But that creates several problems:
- `ControllerXR` is not really part of the avatar anatomy,
- imported skeleton topology becomes polluted with input-source components,
- left/right XR semantics become entangled with asset structure,
- retargeting becomes harder when swapping avatars,
- it still assumes the driver owns descendants instead of targeting existing bones semantically.

So even if this could be made to work mechanically, it is probably the wrong mental model.

---

## 10. Why explicit armature semantics are likely necessary

For a VTuber avatar, “left hand” is not just a transform node ID.

It is more like:
- left wrist root,
- left hand chain,
- maybe finger subset,
- maybe a forearm-hand blend zone,
- maybe asset-specific names under a stable semantic role.

So the engine likely needs a semantic layer like:
- `AvatarHandLeft`
- `AvatarHandRight`
- maybe later `AvatarFingerIndexLeft`, etc.

That semantic layer can then resolve to actual imported transform nodes for a specific asset.

This would also make avatar swapping much cleaner.

---

## 11. Relation to `docs/spec/hand-tracking-armature.md`

The hand-tracking armature doc already points in the right general direction:
- OpenXR hand tracking should be treated as joint data,
- mapped onto a glTF armature,
- then filtered via the transform pipeline,
- with `SkinnedMeshSystem` doing the normal downstream work.

The missing bridge here is specifically:

> how controller/hand pose sources and transform-pipeline outputs should target imported armature subtrees semantically.

That is the gap this note is calling out.

---

## 12. Suggested authored mental model

The authored mental model we probably want is something like:

```text
AvatarRoot
  GLTF(vtuber)
  AvatarHandBinding(left)
    XR pose source = left controller / left hand tracking
    smoothing = QuatTemporalFilter(...)
    target = AvatarHandLeft semantic subtree

  AvatarHandBinding(right)
    XR pose source = right controller / right hand tracking
    smoothing = QuatTemporalFilter(...)
    target = AvatarHandRight semantic subtree
```

The important part is that the binding is **about relationship**, not about ownership in the transform tree.

The hand-binding component does not need to sit between bones in the hierarchy.

It just needs to know:
- which source hand/controller it listens to,
- which avatar/subtree it targets,
- which filtering/retargeting rules apply.

---

## 13. What the transform pipeline should probably do here

For this avatar-hand use case, the transform pipeline should remain responsible for:
- temporal filtering,
- per-channel shaping,
- optional rotation/translation split behavior,
- possibly later retargeting math.

But it probably should not be the thing that *discovers* avatar hand bones implicitly from local children.

So the likely split is:
- **source resolution**: XR/controller/hand tracking system,
- **filtering / processing**: transform pipeline,
- **target resolution**: avatar-hand binding / armature semantics layer,
- **application**: update existing transform nodes in the imported skeleton.

---

## 14. First practical target: VTuber hand roots, not full fingers

A reasonable staging plan is:

### Stage 1
- target the VTuber’s left/right hand or wrist roots,
- smooth rotation strongly,
- preserve translation behavior,
- do not attempt finger articulation yet.

### Stage 2
- optionally extend to a larger subtree (wrist + hand + selected bones),
- maybe include forearm alignment.

### Stage 3
- integrate true hand-joint mapping from OpenXR hand tracking,
- drive finger bones with explicit joint mapping / retargeting.

This matches the current observation that hand-root rotation is the most obvious quality problem.

---

## 15. Important design conclusion

The main architectural change should be:

> Stop treating `ControllerXR` as the thing that must own the transforms it drives.

Instead:
- `ControllerXR` (or a future XR hand source) should provide a pose stream,
- transform pipeline should filter/process that stream,
- a separate explicit targeting layer should apply the result to avatar armature semantics.

That preserves:
- imported glTF hierarchy integrity,
- `SkinnedMeshSystem` expectations,
- current controller-proxy topology for simple cases,
- and a clean path toward VTuber hand driving.

---

## 16. Proposed direction for later implementation

When implementation time comes, the lowest-risk path is probably:

1. **Keep current `ControllerXR` behavior unchanged** for child-driven proxy objects.
2. Add a new **avatar hand binding / armature targeting** concept in ECS.
3. Resolve target bones by semantic role or asset-specific mapping.
4. Use transform-pipeline filtering on the source pose before application.
5. Start by driving only left/right wrist or hand roots.
6. Later extend to full hand-joint mapping for articulated fingers.

That avoids breaking current examples while opening the correct path for VTuber hand control.

---

## 17. Open questions

1. Should the first avatar target be `wrist` or `hand` root for the VTuber asset?
2. Should controller-driven avatar hands and hand-tracking-driven avatar hands share one binding component, or be separate source types?
3. Should armature targeting be by:
   - semantic role (`AvatarHandLeft`),
   - explicit bone name,
   - explicit runtime node lookup,
   - or a layered fallback?
4. Does the transform pipeline need explicit support for external output targets, or should target application live outside the pipeline runtime?
5. For hand tracking, should the initial avatar-hand rotation come from `wrist`, `palm`, or a synthesized pose frame?
6. How should controller `Aim` vs `Grip` map onto avatar hand orientation semantics?

---

## 18. Recommended current stance

Until implementation begins, the best framing is:
- current `ControllerXR` semantics are correct for child-owned tracked helper objects,
- they are not sufficient for imported avatar armature targeting,
- VTuber hand driving should be designed as **source → filter → semantic armature target**,
- not as “put `ControllerXR` into the middle of the skeleton tree.”

That is the cleanest path to better hand-rotation behavior without damaging armature topology.
