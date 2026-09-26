# Secondary motion constraints: authored still poses

Status: first return-to-rest slice implemented. Use child constraint components and a dedicated
`SecondaryMotionConstraintSystem` in `src/engine/ecs/system/secondary_motion_constraint_system.rs`.
Gravity-relative still poses remain a future slice. The builder-method example below is only a
considered alternative.

## Visual goal

Rei(mu)'s `head_bow_ribbon` is a skinned mesh influenced by two bow chains rooted at `head_bow.001` and `head_bow.009`. Spring motion gives the ribbon life during head movement. After the head has been still enough for long enough, the ribbon should settle into an intentional silhouette rather than keep sagging or trembling indefinitely. The first slice returns each chain to its imported rest pose. A later slice can select among several authored still poses according to head orientation relative to gravity: pitched down or back, rolled left or right, and perhaps yawed. Pose changes should read as deliberately stylized, rather than as a coarse approximation of continuous physics.

The stillness test and the resulting pose are separate concerns. A useful test needs at least a motion threshold and a dwell time. It should observe the driving head or chain anchor in a stable parent local basis, so locomotion alone does not count as head motion. Pose selection needs a gravity relative orientation basis, which may be different from the stillness basis. Exactly which motion channels matter (translation, rotation, or both) and which anchor supplies them remain authoring choices to settle.

## Existing shape

Today `SecondaryMotion` owns `SpringBone` children, whose chains resolve imported glTF joints and rest rotations. The retained secondary motion system writes joint rotations after primary pose, AVC, and IK, then propagates them before skinning. It simulates enabled chains continuously. `IKChain` is a separate post pose constraint, while transform stream operators such as `QuatYawFollow`, `QuatTemporalFilter`, and `TransformApplyInverseLocal` show that MMS can express transform behavior as components. They do not currently constrain spring output. See [secondary motion system](../spec/secondary_motion_system.md) and [glTF authoring guide](../how_to/secondary_motion_for_gltf.md).

## Considered approach: builder methods on `SecondaryMotion`

```mms
SecondaryMotion.rest_when_still("[name='head_bow.001']", 0.02, 0.4) {
    SpringBone.from_root("[name='head_bow.001']")
        .virtual_end_length_ratio(1.0)
}
```

Here the `SecondaryMotion` root owns a policy targeted at one chain. The root can list policies without new child component types. The two values stand for a motion threshold and time in seconds; their units and measurement window would need explicit names before implementation. Multiple policies could become additional builder calls.

The cost is that stillness detection, pose selection, transition rules, and future constraints accumulate on the `SecondaryMotion` builder. A constraint must refer to a simulation child through a selector or name, adding a second binding path beside the tree. Ordering and conflicts between several policies become harder to see in MMS.

## Chosen approach: constraint components inside a simulation child

```mms
SecondaryMotion {
    SpringBone.from_root("[name='head_bow.001']")
        .virtual_end_length_ratio(1.0) {
            ReturnToRestWhenStill {
                motion_threshold(0.02)
                still_for(0.4)
            }
        }
}
```

The constraint is a child of the simulation it controls. Its state and parameters have a visible place in the tree. A later `StillPoseSelector` could share the same stillness condition or replace `ReturnToRestWhenStill` without extending every spring builder. This also leaves room for other secondary motion types to participate. The tradeoff is extra syntax and an explicit rule for how constraints bind, run, and combine. The constraint component does not write imported joint transforms itself.

| Question | Builder method | Child constraint |
| --- | --- | --- |
| First rest pose slice | Fewest authored lines | More explicit behavior |
| Several pose or gating policies | Grows root API | Composes in the tree |
| Binding to a simulation | Target name or selector | Parent child relationship |
| Evaluation order | Hidden inside the root | Must be defined and visible |

Use a child constraint under each affected `SpringBone`. Two ribbon halves can each carry the policy; an eventual shared stillness source could avoid duplicate measurements.

### Rei(mu) prefab placement

Author this in a Rei(mu)-specific secondary motion component prefab, following the tree shape of
[`bisket.mms`](../../assets/components/secondary_motion/bisket.mms). The repo currently has a
bow-only [`rei-mu-bow.mms`](../../assets/components/secondary_motion/rei-mu-bow.mms) with the two
`SpringBone.from_root(...)` chains, and [`examples/rei(mu).mms`](../../examples/rei(mu).mms)
imports it. For the VR test, each ribbon `SpringBone` in that prefab now owns a
`ReturnToRestWhenStill` child. A future broader Rei(mu) prefab can collect her other spring bones
in one place, following Bisket's organization while retaining Rei(mu)'s own selectors and tuning.

The one-chain snippet above shows placement. The real ribbon has two chains, rooted at
`head_bow.001` and `head_bow.009`; apply the constraint to both when both ribbon halves need the
same settling behavior. These are authored children of `SpringBone`, not constraints on the
`head_bow_ribbon` mesh node itself.

## Dedicated constraint system

`SecondaryMotionConstraintSystem` owns registration, binding, state, and per-frame evaluation for these child components. The existing `SecondaryMotionSystem` continues to own spring binding, integration, and final joint writes. Their interface should be a small per-chain directive, rather than letting both systems write the same transforms. For the first slice, the directive can describe a rest-pose target, blend weight, and whether to park or reseed the spring state. The exact Rust type is an implementation detail.

The constraint system retains constraint-to-chain ownership and resolved motion-source IDs. It learns about construction, removal, reparenting, GLTF reload, and relevant configuration changes through lifecycle notifications. It must not scan the component tree or resolve selectors each frame. An unbound or invalid constraint leaves the chain's existing simulation behavior intact and reports a diagnostic.

The intended frame sequence is:

```text
primary pose writes and transform propagation
    → SecondaryMotionConstraintSystem samples driving motion and updates directives
    → SecondaryMotionSystem simulates or parks each chain and applies its directive
    → propagate changed chain transforms
    → refresh skinned-mesh palettes
```

This places stillness measurement before spring output changes the joints, so a swaying ribbon does not keep waking itself. The constraint system reads stable primary-pose transforms; the spring system remains the single writer of imported joints. The directive must also tell the spring when to reseed its simulated tails on waking, avoiding a visible jump from stale state. If a settled chain is fully parked, the spring system still has to write the chosen pose whenever the primary pose would otherwise change those joints.

For the first slice, allow one active pose constraint per chain. Reject a second conflicting pose constraint with a clear binding diagnostic. Define priority or composition only when there is a concrete second policy to combine. Keep the selected pose and its transition state in the constraint system, while the spring system applies the resulting target and blend.

## Slices and open decisions

1. **Return to rest (implemented).** The first joint's parent is sampled relative to the nearest Transform above the owning GLTF. Motion is linear speed plus angular speed at a 0.1 m radius, smoothed over about 0.1 s. The authored `motion_threshold` is in m/s equivalent; `still_for` is seconds. After the dwell, the chain blends to imported rest rotations over 0.25 s and parks. Motion above 1.5 times the threshold wakes it, blends out over 0.12 s, and reseeds simulated tails. The Rei(mu) ribbon prefab uses `0.02` and `0.4` for its two chains; VR inspection should guide tuning.
2. **Gravity relative still poses.** Author several targets for the same chain and select or blend them using the head's orientation relative to gravity. Give angular regions margins or hysteresis so small tracking noise does not rapidly swap silhouettes. Define how each authored pose is stored and retargeted to the imported skeleton.

Before the second slice, decide how authored still poses are stored and combined, how to handle tracking loss and seek, and how multiple constraints on one chain should compose. The first slice accepts one pose constraint per chain and diagnoses duplicates.
