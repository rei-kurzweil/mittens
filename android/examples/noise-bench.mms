// XR2 GPU benchmark: baked noise texture (RGBA8 / ASTC) versus procedural
// Perlin noise. Build the APK with `--features noise-bench`. Results print to
// logcat as `stdio [mms] {...}` lines; see docs/task/gpu-benchmarks.md.
//
// One background sphere covers every pixel once. Cases are swapped by
// attaching/detaching their sphere; only one is ever attached. Each window
// warms up, then brackets Benchmark.start/stop (GPU time of both eye passes).
// The installer writes perlin-noise-benchmark-mode<N>.frag variants (N = shader
// NOISE_MODE) from assets/shaders/perlin-noise-benchmark.frag.
import { f32, vec3 } from "types"

// Fixed for comparability with earlier Focus 3 profiles; foveation off so
// gl_FragCoord-based sampling is 1:1 everywhere.
RendererSettings { desired_xr_resolution([1712, 1712]) }
GraphicsSettings.foveation.quality("off")
BGC.rgba(0.0, 0.0, 0.0, 1.0)

let settle_seconds = 10.0
let warmup_seconds = 3.0
let measure_seconds = 6.0
let rounds = 4

fn noise_case(label, mode, texture) {
    let definition = MaterialDefinition(MaterialBase())
        .fragment("assets/shaders/perlin-noise-benchmark-mode" + mode + ".frag")
        .inputs({ tint: vec3, base_cell_px: f32, texture_px: f32 })
    let node = BG {
        name = "noise_case_" + label
        T.scale(80.0, 80.0, 80.0) {
            R.sphere() {
                Material(definition).inputs({ tint = [1.0, 1.0, 1.0], base_cell_px = 128.0, texture_px = 1024.0 })
                Texture.with_uri(texture)
                TextureFiltering.linear()
            }
        }
    }
    return { label = label node = node }
}

let png = "assets/images/noise/perlin-1024.png"
// Every case binds a texture so descriptor setup is identical.
// Tables keyed by index string, as in smoke.mms (no array push/mutation).
let cases = {}
cases["0"] = noise_case("flat", 0, png)
cases["1"] = noise_case("texture-rgba8", 1, png)
cases["2"] = noise_case("texture-astc4x4", 1, "assets/textures/noise/perlin-1024-astc4x4.astc")
cases["3"] = noise_case("texture-astc8x8", 1, "assets/textures/noise/perlin-1024-astc8x8.astc")
cases["4"] = noise_case("perlin-1-octave", 2, png)
cases["5"] = noise_case("perlin-fbm6", 3, png)
let case_count = 6

let stage = T { name = "noise_bench_stage" }
stage

let state = {
    round = 0 slot = 0 phase = "settle" elapsed = 0.0 active = null done = false
}

// Rotate each round and reverse on odd rounds so no case always follows another.
fn case_at(round, slot) {
    let index = (slot + round) % case_count
    if round % 2 == 1 { index = (case_count - 1 - slot + round) % case_count }
    return cases["" + index]
}

fn begin_case() {
    let current = case_at(state.round, state.slot)
    stage.attach(current.node)
    state.active = current
    state.phase = "warmup"
    state.elapsed = 0.0
}

print("noise-bench start: " + case_count + " cases x " + rounds + " rounds, " + warmup_seconds + "s warmup, " + measure_seconds + "s windows")

on_global("FrameTick", fn(event) {
    if state.done { return }
    state.elapsed = state.elapsed + event.dt_sec
    if state.phase == "settle" {
        if state.elapsed >= settle_seconds { begin_case() }
    } else if state.phase == "warmup" {
        if state.elapsed >= warmup_seconds {
            Benchmark.start(state.active.label)
            state.phase = "measure"
            state.elapsed = 0.0
        }
    } else if state.phase == "measure" {
        if state.elapsed >= measure_seconds {
            let result = Benchmark.stop(state.active.label)
            print("noise-bench result " + JSON.stringify({ round = state.round case = state.active.label result = result }))
            state.active.node.detach()
            state.slot = state.slot + 1
            if state.slot >= case_count {
                state.slot = 0
                state.round = state.round + 1
            }
            if state.round >= rounds {
                state.done = true
                print("noise-bench done")
            } else {
                begin_case()
            }
        }
    }
})
