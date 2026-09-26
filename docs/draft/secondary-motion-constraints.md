# Secondary motion constraints: authored still poses

Status: design discussion. The MMS below is illustrative; these components and methods do not exist yet.

## Visual goal

Rei(mu)'s `head_bow_ribbon` is a skinned mesh influenced by two bow chains rooted at `head_bow.001` and `head_bow.009`. Spring motion gives the ribbon life during head movement. After the head has been still enough for long enough, the ribbon should settle into an intentional silhouette rather than keep sagging or trembling indefinitely. The first slice returns each chain to its imported rest pose. A later slice can select among several authored still poses according to head orientation relative to gravity: pitched down or back, rolled left or right, and perhaps yawed. Pose changes should read as deliberately stylized, rather than as a coarse approximation of continuous physics.

The stillness test and the resulting pose are separate concerns. A useful test needs at least a motion threshold and a dwell time. It should observe the driving head or chain anchor in a stable parent local basis, so locomotion alone does not count as head motion. Pose selection needs a gravity relative orientation basis, which may be different from the stillness basis. Exactly which motion channels matter (translation, rotation, or both) and which anchor supplies them remain authoring choices to settle.

## Existing shape

Today `SecondaryMotion` owns `SpringBone` children, whose chains resolve imported glTF joints and rest rotations. The retained secondary motion system writes joint rotations after primary pose, AVC, and IK, then propagates them before skinning. It simulates enabled chains continuously. `IKChain` is a separate post pose constraint, while transform stream operators such as `QuatYawFollow`, `QuatTemporalFilter`, and `TransformApplyInverseLocal` show that MMS can express transform behavior as components. They do not currently constrain spring output. See [secondary motion system](../spec/secondary_motion_system.md) and [glTF authoring guide](../how_to/secondary_motion_for_gltf.md).

## Approach A: builder methods on `SecondaryMotion`

```mms
SecondaryMotion.new()
    .rest_when_still("[name='head_bow.001']", 0.02, 0.4) {
        SpringBone.from_root("[name='head_bow.001']")
            .virtual_end_length_ratio(1.0)
}
```

Here the `SecondaryMotion` root owns a policy targeted at one chain. The root can list policies without new child component types. The two values stand for a motion threshold and time in seconds; their units and measurement window would need explicit names before implementation. Multiple policies could become additional builder calls.

The cost is that stillness detection, pose selection, transition rules, and future constraints accumulate on the `SecondaryMotion` builder. A constraint must refer to a simulation child through a selector or name, adding a second binding path beside the tree. Ordering and conflicts between several policies become harder to see in MMS.

## Approach B: constraint components inside a simulation child

```mms
SecondaryMotion {
    SpringBone.from_root("[name='head_bow.001']")
        .virtual_end_length_ratio(1.0) {
            ReturnToRestWhenStill.new()
                .motion_threshold(0.02)
                .still_for(0.4)
        }
}
```

The constraint is a child of the simulation it controls. Its state and parameters have a visible place in the tree. A later `StillPoseSelector` could share the same stillness condition or replace `ReturnToRestWhenStill` without extending every spring builder. This also leaves room for other secondary motion types to participate. The tradeoff is extra syntax and an explicit rule for how constraints bind, run, and combine. A separate component is only useful if the runtime gives it a clear contract; it should not become an arbitrary second writer of imported joint transforms.

| Question | Builder method | Child constraint |
| --- | --- | --- |
| First rest pose slice | Fewest authored lines | More explicit behavior |
| Several pose or gating policies | Grows root API | Composes in the tree |
| Binding to a simulation | Target name or selector | Parent child relationship |
| Evaluation order | Hidden inside the root | Must be defined and visible |

**Tentative direction:** use a child constraint under each affected `SpringBone`. Keep the spring as the owner of final joint writes; evaluate its constraints against a resolved chain and return a target pose or blend weight to that owner. Two ribbon halves can each carry the policy, and an eventual shared stillness source could avoid duplicate measurements. This preserves the current retained binding model while giving the MMS surface room to grow.

## Slices and open decisions

1. **Return to rest.** Detect low driving motion for a dwell time, then blend the chain toward its imported rest rotations and stop its active spring response while settled. Resume promptly when motion exceeds a wake threshold; define how simulation state is reseeded so the ribbon does not jump. The rest target and blend must respect whatever primary pose is driving the joints that frame.
2. **Gravity relative still poses.** Author several targets for the same chain and select or blend them using the head's orientation relative to gravity. Give angular regions margins or hysteresis so small tracking noise does not rapidly swap silhouettes. Define how each authored pose is stored and retargeted to the imported skeleton.

Before choosing exact MMS names or numeric defaults, decide: which transform is the motion source; whether the threshold measures displacement, speed, angular motion, or a combination; how long the sample window is; how pause, seek, and tracking loss affect the timer; and what happens when multiple constraints request different poses. These choices determine whether the first API remains useful when the second slice arrives.
