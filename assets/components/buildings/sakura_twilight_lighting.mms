import { f32 } from "types"
import { kawaii_twilight_sky_with_material } from "../backgrounds/kawaii-twilight-sky.mms"
import { kawaii_twilight_sky_material } from "../materials/kawaii-twilight-sky.mms"
import { sky_cloud_banks } from "../backgrounds/clouds.mms"
import { settings_slider, settings_slider_row } from "../panels/xr_settings_panel.mms"

let EVENING_SKY = {
    low_color = [0.93, 0.80, 0.66], low_point = 0.0,
    middle_color = [0.68, 0.56, 0.84], middle_point = 0.24,
    high_color = [0.22, 0.17, 0.36], high_point = 0.95,
    cloud_color = [0.86, 0.76, 0.90], cloud_strength = 0.35,
    moon_direction = [-0.35, 0.34, -0.87], moon_radius = 0.055,
    moon_color = [0.98, 0.94, 0.90], moon_strength = 1.2,
    glow_color = [0.80, 0.66, 0.92], glow_strength = 0.8
}
let MORNING_SKY = {
    low_color = [0.99, 0.84, 0.70],
    middle_color = [0.55, 0.76, 0.92],
    high_color = [0.19, 0.43, 0.78],
    cloud_color = [1.0, 0.96, 0.88], cloud_strength = 0.53,
    moon_strength = 0.0, glow_strength = 0.0
}
let SAMPLE_TYPE = { revision: f32, owner: f32, value: f32 }
let CHANNEL_NAME = "arena.sky.time"

fn blend(a, b, t) { return a + (b - a) * t }
fn blend_rgb(a, b, t) {
    return [blend(a[0], b[0], t), blend(a[1], b[1], t), blend(a[2], b[2], t)]
}
fn light_color(light, evening, morning, t) {
    let rgb = blend_rgb(evening, morning, t)
    light.set_rgb(rgb[0], rgb[1], rgb[2])
}
fn time_text(value) {
    if value < 0.005 { return "Evening" }
    if value > 0.995 { return "Morning" }
    return "" + Math.round(value * 100.0) + "%"
}

export fn sakura_twilight_lighting(rank, pedestal_material, implicit_clouds) {
    let sky_material = kawaii_twilight_sky_material(EVENING_SKY)
    let cloud_banks = null
    let cloud_root = null
    let evening_cloud = [0.62, 0.49, 0.70]
    if implicit_clouds {
        sky_material.set_input("cloud_strength", 0.0)
        cloud_banks = sky_cloud_banks([
            [-40.0, 10.0, -45.0, 3.8, 0.45, 5],
            [12.0, 11.0, -55.0, 3.1, -0.20, 5],
            [43.0, 18.0, -33.0, 3.6, -0.92, 3],
            [34.0, 13.0, 42.0, 4.1, 0.65, 5],
            [-43.0, 17.0, 24.0, 3.5, -1.05, 3],
        ], evening_cloud)
        cloud_root = cloud_banks.root
    }
    let ambient = AL.rgb(0.26, 0.24, 0.32) { name = "sakura_twilight_ambient" }
    let key = DL.color(1.0, 0.86, 0.70).intensity(1.3) { name = "sakura_twilight_key" }
    let fill = DL.color(0.70, 0.66, 1.0).intensity(0.4) { name = "sakura_twilight_fill" }
    let key_direction = T.position(-0.70, 0.35, -0.60) { key }
    let scene_lights = T {
        Raycastable.disabled() {
            Selectable.off()
            kawaii_twilight_sky_with_material(sky_material, cloud_root)
        }
        ambient
        key_direction
        T.position(0.5, 0.8, 0.3) { fill }
    }

    let track_width_gu = 10.5
    let slider = settings_slider("sakura_twilight_time_slider", 0.0, 1.0, 0.01, 0.0, track_width_gu)
    let readout = Text { name = "sakura_twilight_time_readout" "Evening" }
    let state = { value = 0.0 revision = 0 owner = 0 outgoing = null incoming = null repeats = 0 }
    fn apply(value) {
        if state.value == value { return }
        state.value = value
        for field in ["low_color", "middle_color", "high_color", "cloud_color"] {
            sky_material.set_input(field, blend_rgb(EVENING_SKY[field], MORNING_SKY[field], value))
        }
        for field in ["moon_strength", "glow_strength"] {
            sky_material.set_input(field, blend(EVENING_SKY[field], MORNING_SKY[field], value))
        }
        if implicit_clouds {
            cloud_banks.set_tint(blend_rgb(evening_cloud, [0.85, 0.87, 0.95], value))
        } else {
            sky_material.set_input("cloud_strength", blend(EVENING_SKY.cloud_strength, MORNING_SKY.cloud_strength, value))
        }
        light_color(ambient, [0.26, 0.24, 0.32], [0.54, 0.58, 0.66], value)
        light_color(key, [1.0, 0.86, 0.70], [1.0, 0.93, 0.78], value)
        light_color(fill, [0.70, 0.66, 1.0], [0.77, 0.86, 1.0], value)
        key.set_intensity(blend(1.3, 1.8, value))
        fill.set_intensity(blend(0.4, 0.56, value))
        key_direction.update_transform(blend_rgb([-0.70, 0.35, -0.60], [0.78, 0.50, -0.32], value))
        readout.set_text(time_text(value))
    }
    on(slider, "SliderChanged", fn(event) {
        if event.value == state.value { return }
        state.revision = state.revision + 1
        state.owner = rank
        state.repeats = 5
        apply(event.value)
    })
    let pedestal = T {
        name = "sakura_twilight_lighting_pedestal"
        T.position(0.0, 0.09, 0.0).scale(0.92, 0.18, 0.72) {
            R.cube() { pedestal_material C.rgba(0.29, 0.26, 0.36, 1.0) }
            Zone.cube([0.5, 0.5, 0.5]) { Collidable.static() {} }
        }
        T.position(0.0, 0.67, 0.0).scale(0.27, 1.04, 0.30) {
            R.cube() { pedestal_material C.rgba(0.49, 0.43, 0.57, 1.0) }
            Zone.cube([0.5, 0.5, 0.5]) { Collidable.static() {} }
        }
        T.position(-1.28, 1.46, 0.22).rotation(-0.20, 0.0, 0.0) {
            LayoutRoot {
                name = "sakura_twilight_lighting_panel"
                available_width(32.0)
                available_height(4.5)
                unit_scale(0.08)
                T {
                    Style {
                        display("flex") flex_direction("column") width(100%) padding(0.6)
                        color([0.0, 0.0, 0.0, 1.0])
                        background_color([0.98, 0.95, 0.84, 1.0])
                    }
                    settings_slider_row("Time of day", slider, readout, track_width_gu)
                }
            }
        }
    }
    fn connect(socket, group, address) {
        let outgoing = socket.publish(CHANNEL_NAME).to(address).type(SAMPLE_TYPE)
        let incoming = socket.subscribe(CHANNEL_NAME).from(address).type(SAMPLE_TYPE)
        group.attach(outgoing)
        group.attach(incoming)
        state.outgoing = outgoing
        state.incoming = incoming
        if state.revision > 0 { state.repeats = 5 }
        on(incoming, "UdpSample", fn(event) {
            if state.incoming != incoming { return }
            let sample = event.value
            if !(sample.value >= 0.0 && sample.value <= 1.0) { return }
            if sample.owner != 1 && sample.owner != 2 { return }
            if sample.revision < 1 || sample.revision != Math.floor(sample.revision) { return }
            // Higher revision wins. For simultaneous edits the host wins the tie.
            if sample.revision < state.revision { return }
            if sample.revision == state.revision && sample.owner >= state.owner { return }
            state.revision = sample.revision
            state.owner = sample.owner
            state.repeats = 0
            apply(sample.value)
            slider.sync_value(sample.value)
        })
    }
    fn disconnect() {
        state.outgoing = null
        state.incoming = null
        state.repeats = 0
    }
    fn send() {
        if !state.outgoing || state.repeats <= 0 || !state.outgoing.ready() { return }
        state.outgoing.write({ revision = state.revision owner = state.owner value = state.value })
        state.repeats = state.repeats - 1
    }
    return { world = scene_lights pedestal = pedestal connect = connect disconnect = disconnect send = send }
}
