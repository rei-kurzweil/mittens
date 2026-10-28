# Indexed transform dependencies and spring contact

Date: 2026-10-28

Status: implemented; automated validation recorded below. Headset smoke test pending.

## Scope

- [x] Replace repeated whole-world TransformParent discovery with a retained index.
- [x] Inspect other transform operators for the same pattern.
- [x] Let spring tails contact regular static collidables, including the floor.
- [x] Let one avatar's spring colliders affect another avatar's spring chains.
- [x] Verify and enlarge Bisket, Rei(mu), and Lumae hand colliders.
- [x] Keep collision configuration serializable and provide per-chain opt-outs.

## Transform dependencies

World retains typed operator IDs and an operator-settings revision. TransformOperatorSystem
caches resolved TransformParent targets and reverse dependencies for their nearest transform
and its structural ancestors. A source/subtree change visits its indexed consumers. Consumers
inside the changed subtree retain the previous exclusion rule.

Topology, selector metadata/GUID changes, and typed operator edits invalidate the cache;
ordinary typed pose writes do not. Target matrices remain live rather than caching poses.
Missing targets can resolve after creation/renaming/reparenting, and removed targets or operators
leave the index on the next revision. This slice keeps the existing propagation and subsystem
notification order; batching overlapping roots is a possible separate follow-up.

InverseLocal already indexes consumers by source. SampleTranslation retains selector results
against the world revision and samples ancestor transforms live; ordinary fork/ancestor and
camera-specific operators follow their structural hierarchy. No additional whole-world transform
operator scan was found that needs the TransformParent fix.

## Spring contacts

SystemWorld synchronizes the existing shared CollidableBvh after primary pose/IK, then supplies
it to secondary motion before spring propagation and skinning. Enabled Collidable.static()
surfaces participate, including moved static surfaces; slide zones remain moving pose consumers.
This uses the engine's existing supported primitive contact frames, not rendered triangle meshes.

Each chain queries a conservative reachable bound once per pose stage. Endpoint spheres use the
chain's scaled hit_radius; contact half-space projection preserves segment length. Downward box
floor sweeps catch a tail crossing a thin platform between substeps. Four bounded projection
passes reconcile sphere/length/surface constraints. Tangent inertia uses the existing drag.

Spring collider target bindings are retained against topology/metadata/configuration revisions.
All world-space sphere positions and scale-adjusted radii are sampled before any chain is solved.
The shared sample is sorted along X; each chain selects only reachable foreign-avatar spheres.
The source avatar needs collider declarations, but does not need active spring chains. Contacts
are positional: they push spring endpoints, without applying reciprocal rigid-body impulses.

Existing explicit same-avatar collider lists remain authoritative. A chain does not start using
all its own avatar's body colliders simply because foreign-avatar contact was added. Imported
same-avatar static surfaces are excluded from world contact.

Both additions default on, with independent per-chain opt-outs:

```mms
SpringBone.from_root("[name='tail']")
    .world_collision(false)
    .avatar_collision(false)
```

The normal authored colliders list still works with either switch disabled. Global secondary-motion
pause/disable semantics are preserved. This remains an endpoint-sphere solver: sparse segments,
very fast lateral collider motion, mutually impossible constraints, and arbitrary pitched/sheared
contact frames are not a general rigid-body/continuous mesh collision solution.

## Avatar hands

Bisket and Rei(mu) already shared bisket_colliders(), with 0.09 m hand spheres. Lumae already had
0.075 m spheres. Both presets now use 0.11 m radii, before world scaling. Actual-model tests verify
that left/right J_Bip hand targets bind for all three assets. The larger radius is a first headset
tuning value; other body collider sizes are unchanged.

## Validation

- Transform/operator regression tests cover indexed source/ancestor lookup, retargeting, subtree
  exclusions, late selector resolution, metadata changes, reparenting/removal, and no cache rebuild
  across repeated ordinary pose writes. Existing parent/inverse/sampling propagation tests pass.
- Spring tests cover stable floor rest with preserved length, disabled surface release, a thin-floor
  crossing, world/foreign collision opt-outs, live foreign hand motion/scaling, collider edits/removal,
  unchanged self-collider selection, and real Bisket/Rei(mu)/Lumae hand bindings.
- MMS round-trip checks preserve both collision opt-outs.

All 84 targeted tests passed: 26 secondary-motion (including five new collision/asset checks),
18 operator, eight transform propagation, three collidable BVH, 16 world graph, 11 Sakura scene,
and two spring MMS round-trip tests. The release profile example also compiles and completes.

```sh
cargo test --lib secondary_motion_system
cargo test --lib transform_operator::tests
cargo test --lib transform_system::tests
cargo test --lib collidable_bvh
cargo test --lib world_graph_tests
cargo test --lib sakura_scene_tests
cargo test --lib roundtrip_spring
cargo run --release --example profile-sakura-cpu -- full 240 6
cargo run --release --example profile-sakura-cpu -- paused 240 6
```

## Six-avatar CPU result

Same CPU-only fixture as the [earlier comparison](sakura-scene-polish-bloom-and-can-physics.md):
six Lumae, 14,822 components, 595 visual instances, 240 warmup plus 240 sampled frames at
simulated 75 Hz. The new collision paths and enlarged hands are enabled in the final running case.
The before column uses the prior recorded run; it is not a new alternating-revision benchmark.

| Mean CPU stage | Before index, running | After index/contact, running | After, paused |
|---|---:|---:|---:|
| Update | 14.845 ms | 5.364 ms | 4.394 ms |
| Render preparation | 1.122 ms | 1.077 ms | 1.050 ms |
| Secondary motion and contact sampling | 0.260 ms | 0.301 ms | 0.0003 ms |
| Spring transform propagation | 9.506 ms | 0.523 ms | 0.0001 ms |
| Post-spring skin work | 0.108 ms | 0.087 ms | 0.001 ms |

The measured propagation cost drops about 18×. The running/paused total-update gap is now
about 0.97 ms in this fixture, compared with about 9.86 ms in the earlier comparison. The fixture
has the regular Sakura surface workload; correctness tests separately exercise actual spring
contacts and foreign hands. It does not establish FPS or a worst-case cost for clustered avatars.

Final logs: `/tmp/sakura-springs-indexed-full.log` and
`/tmp/sakura-springs-indexed-paused.log`.

## Headset smoke test

In Sakura, crouch with the tail above a roof/floor that has Collidable.static(), then move a hand
through another avatar's hair. Check support, jitter, enlarged hand reach, avatar scaling, and
pause/resume. The CPU harness excludes tracking, headset pacing, and GPU work.
