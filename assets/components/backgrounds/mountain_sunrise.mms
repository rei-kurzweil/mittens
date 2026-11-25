import { f32, vec3 } from "types"

// A mountain sunrise: a procedural sky, a lit background mountain range in
// four layers, a sun that blooms, and the scene lights that go with them.
//
// Background removes camera translation, so the range is a backdrop: it turns
// with your view but never comes closer. Positions here are relative to the
// eye, which sits at the origin; FLOOR_Y is the valley floor beneath it.
//
// Each peak is a cube turned onto a corner inside a non-uniformly scaled
// frame, which shears it into a leaning, spiky parallelepiped. A gold sun
// low on one side and a red counter-light from the other colour each ridge's
// two faces; the mountain shader adds haze toward the horizon colour and a
// gold rim on silhouettes near the sun.
//
// Every layer leaves a pass open at both ends of the valley the road runs
// along (+x and -x). The passes shift side to side from layer to layer, so
// the valley reads as a winding road, and on the +x end they bend toward the
// sun, which comes up in the farthest notch.
//
// Azimuths are radians from +x toward +z (so -pi/2 is straight ahead, -z).

let SUN = { azimuth = -1.1 elevation = 0.06 radius = 0.04 }
let FLOOR_Y = -4.0

// distance (m), angular step between peaks, peak top elevation range
// (radians above the eye), base colour, and passes [azimuth, half width,
// top elevation inside the pass].
let LAYERS = [
    { distance = 32.0 step = 0.20 low = 0.09 high = 0.20 color = [0.74, 0.40, 0.40]
      gaps = [[-0.45, 0.75, -0.01], [3.24, 0.45, -0.01]] },
    { distance = 45.0 step = 0.15 low = 0.12 high = 0.24 color = [0.80, 0.46, 0.44]
      gaps = [[-0.80, 0.42, -0.01], [2.84, 0.30, -0.01]] },
    { distance = 58.0 step = 0.12 low = 0.14 high = 0.27 color = [0.85, 0.53, 0.50]
      gaps = [[-1.00, 0.26, 0.0], [3.24, 0.20, 0.0]] },
    { distance = 74.0 step = 0.10 low = 0.16 high = 0.30 color = [0.90, 0.60, 0.56]
      gaps = [[-1.12, 0.12, 0.05]] }
]

let SKY = {
    zenith_color = [0.30, 0.22, 0.46], zenith_point = 0.85,
    upper_color = [0.95, 0.52, 0.62], upper_point = 0.22,
    horizon_color = [1.0, 0.62, 0.72], sun_side_power = 2.0,
    sun_horizon_color = [0.98, 0.34, 0.26], glow_strength = 1.0,
    glow_color = [1.0, 0.76, 0.38], glow_width = 1.0,
    sun_direction = [0.0, 0.0, -1.0], cloud_strength = 0.55,
    cloud_color = [0.86, 0.62, 0.84], below_point = -0.3,
    below_color = [0.42, 0.24, 0.34], cloud_seed = 0.8
}

let sky_definition = MaterialDefinition(MaterialBase())
    .fragment("assets/shaders/sunrise-sky.frag")
    .inputs({
        zenith_color: vec3, zenith_point: f32,
        upper_color: vec3, upper_point: f32,
        horizon_color: vec3, sun_side_power: f32,
        sun_horizon_color: vec3, glow_strength: f32,
        glow_color: vec3, glow_width: f32,
        sun_direction: vec3, cloud_strength: f32,
        cloud_color: vec3, below_point: f32,
        below_color: vec3, cloud_seed: f32
    })

let mountain_definition = MaterialDefinition(MaterialBase())
    .fragment("assets/shaders/sunrise-mountain.frag")
    .inputs({
        haze_color: vec3, haze_near: f32,
        haze_sun_color: vec3, haze_far: f32,
        rim_color: vec3, haze_strength: f32,
        sun_direction: vec3, rim_strength: f32,
        floor_y: f32, valley_mist: f32
    })

fn sun_direction() {
    let c = Math.cos(SUN.elevation)
    return [c * Math.cos(SUN.azimuth), Math.sin(SUN.elevation), c * Math.sin(SUN.azimuth)]
}

// Deterministic 0..1 noise per peak, so the range is the same every run.
fn noise(seed) {
    let x = Math.sin(seed * 12.9898 + 4.1414) * 43758.5453
    return x - Math.floor(x)
}

fn wrap(angle) {
    let a = angle
    if a > Math.pi { a = a - Math.tau }
    if a < -Math.pi { a = a + Math.tau }
    return a
}

// A peak's top elevation: random within the layer's range, eased down to
// the pass height inside a pass.
fn peak_elevation(layer, azimuth, seed) {
    let e = layer.low + (layer.high - layer.low) * noise(seed)
    for gap in layer.gaps {
        let off = Math.abs(wrap(azimuth - gap[0]))
        let open = Math.smoothstep(off, gap[1], gap[1] + 0.22)
        if gap[2] < e { e = gap[2] + (e - gap[2]) * open }
    }
    return e
}

fn peak(layer, index, material) {
    let azimuth = -Math.pi + (index + noise(layer.distance + index) * 0.5) * layer.step
    let seed = layer.distance * 7.0 + index
    let d = layer.distance + (noise(seed + 0.3) - 0.5) * 4.0
    // Elevations are from the eye (the origin), so the top's height is too.
    let top = peak_elevation(layer, azimuth, seed) * d
    // The unit cube turned onto an edge reaches about 0.75 of its frame's
    // height above its centre, which sits on the floor.
    let height = (top - FLOOR_Y) / 0.75
    if height < 0.5 { height = 0.5 }
    let width = d * layer.step * (1.3 + 0.6 * noise(seed + 0.7))
    let tint = 0.9 + 0.2 * noise(seed + 0.9)
    let c = layer.color
    return T.position(d * Math.cos(azimuth), FLOOR_Y, d * Math.sin(azimuth))
        .rotation(0.0, Math.pi / 2.0 - azimuth + (noise(seed + 1.1) - 0.5) * 0.6, 0.0)
        .scale(width, height, width * 0.8) {
        T.rotation((noise(seed + 1.3) - 0.5) * 0.5, 0.35 + 0.55 * noise(seed + 1.7),
            0.785 + (noise(seed + 1.9) - 0.5) * 0.6) {
            R.cube() { material C.rgba(c[0] * tint, c[1] * tint, c[2] * tint, 1.0) }
        }
    }
}

fn layer_peaks(layer, material) {
    let count = Math.floor(Math.tau / layer.step)
    return T {
        name = "mountain_layer_" + layer.distance
        for index in range(count) { peak(layer, index, material) }
    }
}

// options (all optional): sky (overrides for SKY fields), ambient, sun_light,
// counter_light ([r, g, b, intensity]).
// Returns { world, sky_material, mountain_material, sun_light, counter_light,
//           ambient }. Add `world` to the scene once.
export fn mountain_sunrise(options) {
    if options == null { options = {} }
    let sun = sun_direction()
    let sky_inputs = {}
    for field in ["zenith_color", "zenith_point", "upper_color", "upper_point",
        "horizon_color", "sun_side_power", "sun_horizon_color", "glow_strength",
        "glow_color", "glow_width", "cloud_strength", "cloud_color",
        "below_point", "below_color", "cloud_seed"] {
        sky_inputs[field] = SKY[field]
        if options["sky"] != null && options["sky"][field] != null { sky_inputs[field] = options.sky[field] }
    }
    sky_inputs.sun_direction = sun
    let sky_material = Material(sky_definition).inputs(sky_inputs)
    let mountain_material = Material(mountain_definition).inputs({
        haze_color = sky_inputs.horizon_color, haze_near = 30.0,
        haze_sun_color = sky_inputs.sun_horizon_color, haze_far = 95.0,
        rim_color = sky_inputs.glow_color, haze_strength = 0.75,
        sun_direction = sun, rim_strength = 0.6,
        floor_y = FLOOR_Y, valley_mist = 0.55
    })

    let ambient_rgb = [0.30, 0.18, 0.26]
    if options["ambient"] != null { ambient_rgb = options.ambient }
    let sun_rgbi = [1.0, 0.70, 0.36, 1.5]
    if options["sun_light"] != null { sun_rgbi = options.sun_light }
    let counter_rgbi = [0.95, 0.30, 0.38, 0.7]
    if options["counter_light"] != null { counter_rgbi = options.counter_light }
    let ambient = AL.rgb(ambient_rgb[0], ambient_rgb[1], ambient_rgb[2]) { name = "mountain_sunrise_ambient" }
    let sun_light = DL.color(sun_rgbi[0], sun_rgbi[1], sun_rgbi[2]).intensity(sun_rgbi[3]) {
        name = "mountain_sunrise_sun"
    }
    let counter_light = DL.color(counter_rgbi[0], counter_rgbi[1], counter_rgbi[2]).intensity(counter_rgbi[3]) {
        name = "mountain_sunrise_counter"
    }
    // The sun light comes from just above the sun; the counter-light from the
    // opposite side of the sky, higher up.
    let sun_from = [sun[0], sun[1] + 0.18, sun[2]]
    let counter_from = [-sun[0], 0.55, -sun[2]]

    let sun_distance = 88.0
    let sun_size = 2.0 * Math.sin(SUN.radius) * sun_distance
    let world = T {
        name = "mountain_sunrise"
        Raycastable.disabled() {
            Selectable.off()
            BG {
                name = "mountain_sunrise_sky"
                // Radius 80 m stays inside the default 100 m far plane.
                T.scale(160.0, 160.0, 160.0) {
                    R.sphere() { name = "mountain_sunrise_sky_surface" sky_material }
                }
            }
            BG.occlusion_and_lighting() {
                name = "mountain_sunrise_range"
                // The valley floor, out to the far plane.
                T.position(0.0, FLOOR_Y - 0.25, 0.0).scale(180.0, 0.5, 180.0) {
                    R.cube() { mountain_material C.rgba(0.46, 0.26, 0.34, 1.0) }
                }
                for layer in LAYERS { layer_peaks(layer, mountain_material) }
                // Behind the farthest ridge, so its notch hides the sun's
                // lower edge. Emissive, so it blooms.
                T.position(sun[0] * sun_distance, sun[1] * sun_distance, sun[2] * sun_distance)
                    .scale(sun_size, sun_size, sun_size) {
                    R.sphere() {
                        name = "mountain_sunrise_sun_disc"
                        Material(MaterialDefinition.toon())
                        C.rgba(1.0, 0.86, 0.55, 1.0)
                        EM.on().intensity(2.2)
                    }
                }
            }
        }
        ambient
        T.position(sun_from[0], sun_from[1], sun_from[2]) { sun_light }
        T.position(counter_from[0], counter_from[1], counter_from[2]) { counter_light }
    }
    return {
        world = world
        sky_material = sky_material
        mountain_material = mountain_material
        sun_light = sun_light
        counter_light = counter_light
        ambient = ambient
    }
}
