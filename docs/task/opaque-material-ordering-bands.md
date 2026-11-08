# Surface definition opaque ordering buckets

Status: implemented in `eb5cafc2`; tests, release build and fresh controlled
comparison passed. Broader vehicle/example adoption follows in a separate change.

## Ownership and API

`MaterialBase::Surface` defines the eligible rendering contract. Each
`MaterialDefinition` built from Surface chooses its own opaque bucket:
`near`, `middle` (default), or `far`. No three new bases are introduced.
Built-in Anime and Toon definitions use Surface too.

```mms
let truck = MaterialDefinition(MaterialBase())
    .opaque_bucket("near")
    .fragment("assets/shaders/lambert-toon.frag")
    .inputs({ /* the shader's typed fields */ })
let walls = MaterialDefinition.anime().opaque_bucket("far")
```

`Material(definition)` instances share the definition's bucket. There is no
per-instance override or new runtime MMS setter in this slice. Shader/schema
builders on built-in definitions remain read-only; bucket authoring is allowed.
Invalid bucket names and explicit buckets on planar-reflective/extracted-emissive
bases are rejected. Non-Surface definitions use the normal middle ordering
position without gaining this authoring option.

Plain `MaterialDefinition.anime()` retains its existing shared identity. A
bucketed built-in constructor owns an independent definition component and
registry metadata record, reusing built-in shader programs, schemas and defaults.
Setting its bucket cannot modify every other Anime material. Custom definition
registration retains its existing program identity rules; changing an existing
bucket does not compile another shader or create another pipeline.

## Meaning and sorting

The opaque sort key is stencil scope, bucket, then the existing material,
texture, mesh, filtering and material-instance grouping. It replaces the interim
custom-before-built-in classification. A built-in Near definition can therefore
precede a custom Far definition. Unassigned Surface definitions are Middle,
regardless of shader kind; other examples may need authored hints to reproduce
the interim custom-first policy's ordering.

Buckets are scheduling hints, not visual layers, distance tests or render passes.
They share normal depth testing/writes: a nearer Far surface still occludes a
more distant Near surface. Geometry should be marked for its typical role;
changes in camera position can make an authored hint inefficient.

Background retains its separate phase (including optional self-occluding depth),
whose depth is cleared before foreground rendering. Transparent, cutout,
overlay and background sorting ignore these buckets. Existing stencil entry,
source rendering and exit operations remain intact. As with any opaque reorder,
coplanar surfaces deserve visual review.

## Native Rust and invalidation

`OpaqueBucket::{Near, Middle, Far}` is the native enum. Authored native components
can use `MaterialDefinitionComponent::with_opaque_bucket(bucket)`; Surface scope
is validated during publication. Base/shader/schema identity follows the existing
registration contract.

For a registered definition, use
`VisualWorld::set_surface_opaque_bucket(definition_id, bucket)` to validate and
invalidate retained draw ordering when the value actually changes. It leaves
material uniform revisions and GPU program identities alone. Native component
changes published through `MaterialSystem` synchronize the metadata too; clearing
an authored bucket returns a Surface definition to Middle. Low-level registry
mutation alone does not notify the visual-world cache.

No per-frame camera sorting, per-draw allocation, extra render targets or
material-type dispatch is added to command submission. Retained commands are
rebuilt when ordering changes, then reused with normal camera/joint updates.

## Ship authoring

- Truck LambertToon definitions: Near. The generic factory defaults to Middle
  and retains one shared definition per requested bucket.
- Ship directional-fill avatar/face definition: Near (same angular fill/rim).
- Ship environment built-in Anime definition: Far.
- Ship washroom/passage tile definitions: Far; generic pool tiles default Middle.
- Other Surface definitions: Middle unless explicitly authored.

Interior/exterior truck material separation and automatic occupancy-driven
bucket changes remain deferred. No shader math or character shading defaults
change in this slice.

## Validation

- Five focused bucket tests: host syntax, Surface restriction, independent
  built-in definitions, default built-in sharing, ordering across shader kinds,
  batching/cache reuse, invalidation, clipping and unchanged non-opaque phases.
- 26 visual-world tests passed (including four focused ordering tests).
- Seven MaterialSystem tests passed; existing typed-material host test passed.
- `cargo check --lib` and release application build passed.
- Material runtime registration was extracted from oversized runtime_config.rs
  before expansion; definition authoring is focused in
  `src/scripting/component_bindings/material_definition.rs`.

The local
fixture uses Mitten + Miku, one cafeteria truck, no orchard; other indoor rooms
remain loaded and the mirror stays off. MSAA 4x, bloom/emission, 1920×1920 per eye,
240 warmup/480 measured frames, two repetitions per walking/driving case, with
identical GPU query instrumentation. It is not a live XR/compositor measurement.


## Fresh controlled baseline and bucket comparison (2026-11-08)

The user reported possible workstation interference during the first bucket
captures. Those four files are preserved but explicitly superseded; none of their
timings support the following comparison. A fresh eight-capture sequence brackets
bucket runs with baseline runs: baseline walking/driving, bucket driving/walking,
bucket walking/driving, baseline driving/walking. Each policy/motion has two
captures of 240 warmup and 480 measured frames.

Every capture uses the **same executable, current scene, shaders and definition
metadata**. Test-only `SHIP_PROFILE_OPAQUE_POLICY=custom_first` reproduces the
previous opaque sort key, ignoring the authored buckets. `buckets` uses the new
key. The control is evaluated only during retained-stream rebuilds, affects the
whole opaque stream and its extraction order, and compiles out of normal builds.
It is not an engine API or a live runtime switch. `SHIP_PROFILE_TRUCK_ORDER=native`
keeps the separate per-body diagnostic intervention disabled in both policies.

The baseline therefore isolates ordering policy with the new metadata plumbing
held fixed, rather than benchmarking every implementation difference against an
old executable. It must not be described as a complete end-to-end binary comparison
with commit 34dd94f4. Independent scene loads still have startup/order variation.

960-frame pooled means and p95, in milliseconds (p95 is not averaged):

| Motion | CPU update baseline → buckets | GPU graphics baseline → buckets | Elapsed mean baseline → buckets | Elapsed p95 baseline → buckets |
|---|---:|---:|---:|---:|
| walking | 5.687 → 5.840 | 10.132 → 9.812 | 17.097 → 16.984 | 18.500 → 19.675 |
| driving | 6.670 → 6.792 | 11.270 → 11.335 | 19.238 → 19.461 | 20.568 → 21.003 |

Individual captures:

| Policy | Motion | Repeat | CPU update ms | GPU graphics ms | Opaque fragments M | Elapsed mean ms | Elapsed p95 ms |
|---|---|---|---:|---:|---:|---:|---:|
| custom_first | walking | 1 | 5.752 | 9.823 | 15.579 | 16.870 | 18.130 |
| custom_first | driving | 1 | 6.639 | 11.287 | 18.684 | 19.230 | 20.362 |
| buckets | driving | 1 | 6.709 | 11.460 | 18.862 | 19.500 | 20.683 |
| buckets | walking | 1 | 5.649 | 9.628 | 15.133 | 16.546 | 17.665 |
| buckets | walking | 2 | 6.031 | 9.995 | 15.885 | 17.423 | 21.413 |
| buckets | driving | 2 | 6.874 | 11.209 | 18.224 | 19.422 | 22.084 |
| custom_first | driving | 2 | 6.701 | 11.253 | 18.246 | 19.247 | 20.614 |
| custom_first | walking | 2 | 5.621 | 10.441 | 16.277 | 17.324 | 18.829 |

Driving GPU time is effectively unchanged in this comparison (about 0.06 ms
higher with buckets); walking is modestly lower. These two repetitions do not
establish a further driving speedup over the interim custom-first policy. The
value of the change is explicit definition-level ordering, independent of
custom/built-in shader identity, while retaining roughly the previous performance.
CPU means vary too; do not attribute those small differences to the sort key
without further profiling, especially since steady-state commands remain cached.

All 480 per-pair camera matrices, player/truck poses, mount/power/mirror states,
draw counts, component counts and input vertex counts match. Both policies retain
two graphics command-cache hits per frame with zero rebuilds and no steady-state
material allocations. No new material compilation errors occurred. Geometry
fingerprints/material IDs need not be identical across independent scene loads;
matching draw counts alone is not a perfect same-work shader microbenchmark.

These are local one-truck/no-orchard offscreen captures with diagnostic queries,
not live OpenXR/ALVR/compositor or 2144-per-eye timing guarantees. Frame budget
claims should use the headset smoke test too.

Raw ignored files:
`examples/local/benchmarks/cafeteria-1920-{walking,driving}-surface-policy-{custom_first,buckets}-r{1,2}-clean-2026-11-08.*`.
`surface-bucket-policy-clean-2026-11-08.manifest.json` preserves order, initial GPU
state, hashes, aggregate counters and the runner. Superseded initial captures use
`surface-opaque-buckets` and are excluded from this evidence.
