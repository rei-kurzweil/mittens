// Asset-only preview: airflow-controlled fire, barrel puffs and impact puffs.
// Run from the repository root:
// cargo run --release -- load assets/scenes/smoke-materials.mms
import { smoke } from "../components/smoke.mms"
import { fire } from "../components/fire.mms"
import { smoke_material } from "../components/materials/smoke.mms"
import { ui_unlit_material } from "../components/materials/ui_unlit.mms"

RendererSettings { window_size(1280, 800) }
RenderGraph.off()
XR.off()
BGC.rgba(0.045, 0.055, 0.08, 1.0)
T.position(0.0, 1.15, 0.75) { C3D {} }

let shared_smoke = smoke_material()
let plain = ui_unlit_material()
let wisp = fire({
    name = "preview_fire" airflow = 0.1 burn_rate = 0.15
    smoke_options = {
        name = "preview_wisp" material = shared_smoke capacity = 18
        size = 0.026 lifetime = 2.2 interval = 0.13 growth = 3.1
        velocity = [0.0, 0.13, 0.0] spread = 0.005 wobble = 0.022
        fade_in = 0.045 fade_out = 1.7 max_opacity = 0.78
    }
})
let burning = wisp.material
let barrel = smoke({
    name = "preview_barrel" material = shared_smoke capacity = 8
    size = 0.07 lifetime = 1.1 growth = 2.7
    velocity = [0.025, 0.19, 0.0] spread = 0.045 wobble = 0.028
    step_rate = 8.0 fade_in = 0.04 fade_out = 0.9 seed = 11
})
let impact = smoke({
    name = "preview_impact" material = shared_smoke capacity = 12
    size = 0.10 lifetime = 1.5 growth = 3.0
    velocity = [0.0, 0.20, 0.0] spread = 0.13 wobble = 0.035 seed = 23
})
wisp.root
barrel.root
impact.root

let prop = T.position(-0.42, 0.92, -1.0).rotation(0.0, 0.0, -1.57079633) {
    R.cylinder(0.012, 0.22, 16) {
        burning
        C.rgba(0.88, 0.85, 0.72, 1.0)
    }
    wisp.source
}
prop
T.position(0.0, 0.90, -1.0).rotation(1.57079633, 0.0, 0.0) {
    R.cylinder(0.025, 0.16, 12) { plain C.rgba(0.25, 0.29, 0.37, 1.0) }
}
T.position(0.0, 0.90, -0.84) { barrel.source }
T.position(0.0, 0.77, -1.05).scale(1.5, 0.025, 0.7) {
    R.cube() { plain C.rgba(0.13, 0.16, 0.22, 1.0) }
}

let state = { seconds = 0.0 last_burst = -1.0 last_airflow = -1.0 }
on_global("FrameTick", fn(event) {
    state.seconds = state.seconds + event.dt_sec
    let cycle = state.seconds % 10.0
    prop.update_transform([-0.42 + 0.06 * Math.sin(state.seconds * 1.2), 0.92, -1.0])
    if cycle < event.dt_sec {
        wisp.set_burn_progress(0.0)
        wisp.smoke.reset_source()
    }
    let airflow = 0.1
    if cycle >= 2.0 && cycle < 4.0 { airflow = 0.9 }
    if airflow != state.last_airflow {
        state.last_airflow = airflow
        wisp.set_airflow(airflow)
    }
    if cycle < 7.0 { wisp.ignite() } else { wisp.extinguish() }

    let burst_index = Math.floor(state.seconds / 3.0)
    if burst_index != state.last_burst {
        state.last_burst = burst_index
        barrel.burst(3)
        impact.burst_at([0.45, 0.80, -1.0], 5)
    }
    wisp.update(event.dt_sec)
    wisp.source.update_transform([0.0, wisp.stats().burn_progress * 0.22, 0.0])
    barrel.update(event.dt_sec)
    impact.update(event.dt_sec)
})
