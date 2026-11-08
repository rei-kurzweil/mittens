# LambertToon truck material experiment

Status: implemented as an opt-in material definition; awaiting visual review.

## Existing scene materials

Ship architecture normally inherits `ship_environment_material()` from
`assets/components/materials/ship_anime_materials.mms`: built-in Anime with rim
strength 0.90. Walls/floors and most cafeteria/shop geometry use it. Washroom and
passage surfaces override it with `pool_tiles.mms`; emissive objects, labels,
windows and individual props also have their own overrides. Character presets
use the separate directional-fill Anime definition, rim strength 0.42.

Built-in Anime gates diffuse lighting with N·L > 0, then uses distance/cone
attenuation and light intensity to drive its smooth shade ramp. It does not
multiply eligible light by the cosine. Pool tile diffuse follows that approach
but also has angle-dependent specular. Legacy Toon does use cosine weighting,
but its quantized shading differs from the Anime ramp and is not used here.

## New definition

`assets/components/materials/lambert_toon.mms` shares one custom definition backed
by `assets/shaders/lambert-toon.frag`. Its diffuse input is:

```
sum(max(dot(normal, light_direction), 0) * intensity * attenuation)
```

Attenuation includes distance falloff and the spotlight cone, rather than
multiplication by raw distance. Ramp, albedo/alpha handling and camera-facing rim
match Anime. Rim is computed once, independent of scene lights. There is no
matcap or specular term in this experiment. As with the current Anime shader,
light color and ambient do not tint the ramp.

`pickup_lambert_toon.mms` preserves the previous pickup preset's controls,
including rim strength 1.0 (~2.38× character rim). Both imported-model and
procedural-prototype pickups use it. Architecture, characters, global Anime and
tiles remain unchanged; the previous `pickup_anime.mms` remains available.
The definition is compiled only when used, through existing custom material APIs.

## Scope and follow-up

- [x] Compile GLSL to SPIR-V and load it through the real ship rendering fixture.
- [ ] Review the truck visually in XR, especially grazing-light panels.
- [ ] After Blender material assignments exist, separate exterior and interior
      finishes; decide whether exterior specular is worth its measured cost.
      Keep the interior cheaper. No windshield is introduced here.

## Measurement

Use the ignored local ship variant: Mitten player, Miku vendor, one cafeteria
pickup; bridge/barracks/washroom remain loaded, orchard absent, mirror off.
1920×1920 per eye, MSAA 4x, bloom/emission enabled, GTX 1080, 240 warmup and 480
sample frames per walking/driving run. All normal callbacks remain enabled.
These offscreen stereo captures exclude OpenXR/compositor/ALVR costs.


### 2026-11-07 results

All times are milliseconds. CPU is the scene-update stage, not all CPU work;
elapsed includes other CPU stages and GPU completion waiting.

| Motion/run | Truck finish | CPU update | GPU graphics | Elapsed mean | Elapsed p95 |
|---|---|---:|---:|---:|---:|
| walking, r1 | Anime | 5.678 | 11.221 | 18.238 | 20.371 |
| walking, r1 | LambertToon | 5.745 | 11.141 | 18.203 | 20.358 |
| driving, r1 | Anime | 6.844 | 13.131 | 21.345 | 23.144 |
| driving, r1 | LambertToon | 6.850 | 14.800 | 23.039 | 24.710 |
| driving, r2 | Anime | 6.872 | 13.419 | 21.677 | 23.670 |
| driving, r2 | LambertToon | 6.734 | 15.317 | 23.417 | 25.366 |

R1 ran Anime then LambertToon; R2 reverses driving order (LambertToon then
Anime) and enables diagnostic GPU pass timestamps on both. Do not directly
compare R1/R2 as though query boundaries were free. R2 opaque time rises
11.733 → 13.612 ms; other pass means remain approximately unchanged. The
1.7–1.9 ms driving GPU regression is reproducible here; walking is effectively
unchanged. This is a visual experiment, not a performance optimization.
The comparison changes both the lighting calculation and built-in versus custom
pipeline: it does not establish that the extra cosine multiply alone costs this
much. A follow-up control would compare binary-gate and cosine variants through
the same custom pipeline before attributing the cause.

All 480 camera matrices, player/truck poses, mount/power/mirror states and per-frame
draw counts match within each pair. Lambert has two additional material/definition
components, no extra rendered geometry. Both retain two cached graphics commands
per frame with zero rebuilds or steady-state material allocations. The shader
compiles with glslangValidator and the full ship fixture loads/renders it.
There is an existing asset-scan `cannot read 'types'` warning in both baselines
and candidates; no new material compilation/pipeline errors were reported.

Raw ignored captures: `examples/local/benchmarks/cafeteria-1920-{walking,driving}-truck-lambert-{baseline,candidate}-r1-2026-11-07.*`
and driving-only `r2` files. An ignored
`truck-lambert-material-experiment-2026-11-07.manifest.json` records sources,
shader hashes and executable provenance. No model or restricted local example is
included in the tracked report.


### Same-custom-pipeline control (2026-11-07)

To isolate the diffuse calculation, reran driving with exactly the same custom
material definition, field layout, shader URI and truck imports. Only the
accumulation changed:

```glsl
// Binary-gate control:
if (ndotl > 0.0) {
    facing_amount += amount;
}
// Angle + distance/cone attenuation:
facing_amount += max(ndotl, 0.0) * amount;
```

Both use `amount = max(intensity, 0) * attenuation`. No extra runtime switch or
new material component was added. Each independent process compiles its shader
before warmup. The sequence is binary → Lambert → Lambert → binary, followed by
a fresh built-in Anime reference. All runs use GPU pass timestamps, the same
1920/eye MSAA 4x bloom/emission setup, 240 warmup and 480 sample frames.

| Order | Calculation/path | CPU update ms | GPU graphics ms | GPU opaque ms | Elapsed mean ms | Elapsed p95 ms |
|---|---|---:|---:|---:|---:|---:|
| 1 | Custom binary gate | 6.786 | 16.298 | 14.605 | 24.488 | 26.531 |
| 2 | Custom Lambert | 6.740 | 16.039 | 14.331 | 24.138 | 26.354 |
| 3 | Custom Lambert | 6.665 | 15.341 | 13.625 | 23.357 | 24.718 |
| 4 | Custom binary gate | 6.759 | 15.493 | 13.774 | 23.629 | 25.235 |
| 5 | Built-in Anime reference | 6.848 | 12.326 | 10.655 | 20.521 | 22.417 |

The two-run GPU mean is **15.895 ms binary** and **15.690 ms Lambert**. The
0.205 ms difference is smaller than the observed within-variant run spread
(0.805 ms binary, 0.698 ms Lambert); these runs provide no evidence that cosine
weighting is slower. This does not prove identical cost on every GPU/scene.
The custom binary gate also retains the slowdown relative to built-in Anime,
whose fresh reference averages **12.326 ms GPU**. Therefore the original
built-in/custom comparison must not be presented as a cost of replacing the if
statement with a multiply, or as evidence of branch prediction overhead.

The custom path/shader configuration is the remaining investigation target;
its exact cause is unproven. Draw ordering/overdraw, pipeline state and compiled
shader differences are hypotheses, not measured explanations. Background desktop
and SteamVR processes remained present (initial GPU utilization was 2%); raw
process state is recorded, so these are offscreen application comparisons rather
than isolated-hardware or live headset frame-rate guarantees.

All four custom runs have identical component counts, draw counts and 480 camera
and motion states. The built-in reference matches camera/motion/draws and has two
fewer material/definition components. Graphics commands remain cached (two hits
per frame, zero rebuilds). Only the known asset-scan types warning is present;
there are no new material compilation errors. Lambert source and truck imports
are restored for visual testing after the diagnostic swaps.

- [x] Compare binary and cosine calculations through the same custom pipeline.
- [ ] Explain the remaining custom-versus-built-in difference using matched
      shader/state inspection and draw-order/overdraw measurements before
      changing the engine or claiming a fix.

Ignored raw captures:
`examples/local/benchmarks/cafeteria-1920-driving-toon-same-custom-{binary,lambert}-r{1,2,3,4}-2026-11-07.*`
(binary r1/r4, Lambert r2/r3), plus
`cafeteria-1920-driving-toon-fresh-builtin-r1-2026-11-07.*`.
`toon-same-custom-pipeline-2026-11-07.manifest.json` retains both shader sources,
shader/executable hashes, order, process snapshot and measurements; diagnostic
scripts are copied alongside it. Restricted models remain excluded from Git.

### Follow-up: opaque ordering

The [clean custom-material GPU investigation](custom-material-gpu-profile.md)
finds that material-ID sorting moves the truck later, increasing fragment work.
Moving custom Anime early drops GPU time 15.129 → 12.845 ms; moving built-in Anime
late raises it 13.240 → 14.670 ms. This substantially explains the earlier
apparent shader penalty. A production ordering remedy and strict same-frame
shader-cost microbenchmark remain separate follow-ups. Initial ordering captures
were discarded from the comparison after the user reported accidental GPU use;
only fresh `r2-clean` data supports this conclusion.
