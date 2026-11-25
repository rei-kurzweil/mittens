import { f32, vec3 } from "types"

// A mountain sunrise: a procedural sky, a lit mountain range in four rings
// around a play area, a sun that blooms, and the scene lights that go with
// them.
//
// The sky and the sun's disc are Background (they turn with your view but
// never come closer). The mountains are ordinary world geometry, so they have
// parallax as you move and drive: each ring is an ellipse around the play
// area, merged into one mesh (CombineMesh) with baked ambient occlusion in
// the creases where peaks overlap. They neither collide nor take hits.
//
// Each peak is a cube turned onto a corner inside a non-uniformly scaled
// frame, which shears it into a leaning, spiky parallelepiped. A gold sun
// low on one side and a red counter-light from the other colour each ridge's
// two faces; the mountain shader adds haze toward the horizon colour and a
// gold rim on silhouettes near the sun.
//
// Every ring leaves a pass open at both ends of the valley the road runs
// along (+x and -x). The passes shift side to side from ring to ring, so the
// valley reads as a winding road, and on the +x end they bend toward the
// sun, which comes up in the farthest notch.
//
// Azimuths are radians around the play area's centre, from +x toward +z
// (so -pi/2 is -z).

let SUN = { azimuth = -1.1 elevation = 0.06 radius = 0.04 }

// margin beyond the play area (m), spacing between peaks along the ring (m),
// peak height range above the floor (m), base colour, and passes
// [azimuth, half width, peak height inside the pass].
let LAYERS = [
    { margin = 16.0 spacing = 13.0 low = 7.0 high = 15.0 color = [0.74, 0.40, 0.40]
      gaps = [[-0.45, 0.75, 1.5], [3.24, 0.45, 1.5]] },
    { margin = 30.0 spacing = 16.0 low = 13.0 high = 24.0 color = [0.80, 0.46, 0.44]
      gaps = [[-0.80, 0.42, 2.0], [2.84, 0.30, 2.0]] },
    { margin = 46.0 spacing = 20.0 low = 20.0 high = 34.0 color = [0.85, 0.53, 0.50]
      gaps = [[-1.00, 0.26, 3.0], [3.24, 0.20, 3.0]] },
    { margin = 66.0 spacing = 24.0 low = 30.0 high = 46.0 color = [0.90, 0.60, 0.56]
      gaps = [[-1.12, 0.12, 6.0]] }
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

// A peak's height: random within the ring's range, eased down to the pass
// height inside a pass.
fn peak_height(layer, azimuth, seed) {
    let h = layer.low + (layer.high - layer.low) * noise(seed)
    for gap in layer.gaps {
        let off = Math.abs(wrap(azimuth - gap[0]))
        let open = Math.smoothstep(off, gap[1], gap[1] + 0.22)
        if gap[2] < h { h = gap[2] + (h - gap[2]) * open }
    }
    return h
}

// Peaks are evenly spaced in the ellipse's parameter, not in distance, so
// each one's width follows the local ring speed: wider along the long sides.
fn peak(layer, ring, index, count, floor_y, material) {
    let seed = layer.margin * 7.0 + index
    let step = Math.tau / count
    let phi = -Math.pi + (index + (noise(seed + 0.1) - 0.5) * 0.4) * step
    let a = ring.a + (noise(seed + 0.3) - 0.5) * 4.0
    let b = ring.b + (noise(seed + 0.3) - 0.5) * 4.0
    let x = ring.center[0] + a * Math.cos(phi)
    let z = ring.center[1] + b * Math.sin(phi)
    let azimuth = Math.atan2(b * Math.sin(phi), a * Math.cos(phi))
    let speed = Math.sqrt(a * a * Math.sin(phi) * Math.sin(phi) + b * b * Math.cos(phi) * Math.cos(phi))
    // The unit cube turned onto an edge reaches about 0.75 of its frame's
    // height above its centre, which sits on the floor.
    let height = peak_height(layer, azimuth, seed) / 0.75
    let width = speed * step * (1.3 + 0.6 * noise(seed + 0.7))
    let c = layer.color
    return T.position(x, floor_y, z)
        .rotation(0.0, Math.pi / 2.0 - azimuth + (noise(seed + 1.1) - 0.5) * 0.6, 0.0)
        .scale(width, height, width * 0.8) {
        T.rotation((noise(seed + 1.3) - 0.5) * 0.5, 0.35 + 0.55 * noise(seed + 1.7),
            0.785 + (noise(seed + 1.9) - 0.5) * 0.6) {
            R.cube() { material C.rgba(c[0], c[1], c[2], 1.0) }
        }
    }
}

// One ring as one combined mesh. CombineMesh keeps the first peak's colour
// for the whole ring; the baked occlusion darkens where peaks overlap.
fn layer_peaks(layer, center, extent, floor_y, material) {
    let ring = { center = center a = extent[0] + layer.margin b = extent[1] + layer.margin }
    let perimeter = Math.tau * Math.sqrt((ring.a * ring.a + ring.b * ring.b) / 2.0)
    let count = Math.floor(perimeter / layer.spacing)
    return CombineMesh {
        name = "mountain_ring_" + layer.margin
        AmbientOcclusion.texcoord0().proximity_radius(6.0).cavity_radius(3.0).strength(0.55)
        for index in range(count) { peak(layer, ring, index, count, floor_y, material) }
    }
}

// options (all optional):
//   center [x, z] and extent [half x, half z]: the play area the rings
//     surround (default the truck bed demo's range);
//   floor_y: the valley floor's top, just under the play area's ground;
//   sky: overrides for SKY fields; ambient [r, g, b];
//   sun_light, counter_light: [r, g, b, intensity].
// Returns { world, sky_material, mountain_material, sun_light, counter_light,
//           ambient }. Add `world` to the scene once. Rings reach about 66 m
// beyond the play area, so the camera's far plane must reach past them.
export fn mountain_sunrise(options) {
    if options == null { options = {} }
    let center = [0.0, -7.0]
    if options["center"] != null { center = options.center }
    let extent = [48.0, 15.5]
    if options["extent"] != null { extent = options.extent }
    let floor_y = -0.35
    if options["floor_y"] != null { floor_y = options.floor_y }

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
        haze_color = sky_inputs.horizon_color, haze_near = 40.0,
        haze_sun_color = sky_inputs.sun_horizon_color, haze_far = 220.0,
        rim_color = sky_inputs.glow_color, haze_strength = 0.7,
        sun_direction = sun, rim_strength = 0.6,
        floor_y = floor_y, valley_mist = 0.5
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

    // Background, so at any distance inside the sky; the world mountains
    // always cover it.
    let sun_distance = 70.0
    let sun_size = 2.0 * Math.sin(SUN.radius) * sun_distance
    let span = [2.0 * (extent[0] + 80.0), 2.0 * (extent[1] + 80.0)]
    let world = T {
        name = "mountain_sunrise"
        Raycastable.disabled() {
            Selectable.off()
            BG {
                name = "mountain_sunrise_sky"
                // Radius 80 m stays inside a 100 m far plane.
                T.scale(160.0, 160.0, 160.0) {
                    R.sphere() { name = "mountain_sunrise_sky_surface" sky_material }
                }
            }
            BG.occlusion_and_lighting() {
                name = "mountain_sunrise_sun_layer"
                // Emissive, so it blooms.
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
            T {
                name = "mountain_sunrise_range"
                // The valley floor out to the farthest ring, just under the
                // play area's ground.
                T.position(center[0], floor_y - 0.25, center[1]).scale(span[0], 0.5, span[1]) {
                    R.cube() { mountain_material C.rgba(0.46, 0.26, 0.34, 1.0) }
                }
                for layer in LAYERS { layer_peaks(layer, center, extent, floor_y, mountain_material) }
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
