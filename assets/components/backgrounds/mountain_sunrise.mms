import { f32, vec3 } from "types"
import { cartoon_grass } from "../materials/cartoon_grass.mms"

// A mountain sunrise on a clifftop: a procedural sky, a grass plateau
// around the play area that ends in rock cliffs, a hazy plain far below, a
// few low mountain ranges out on the plain in some directions (the sunrise
// side stays open), a sun that blooms, and the scene lights that go with
// them.
//
// The sky and the sun's disc are Background (they turn with your view but
// never come closer). Everything else is world geometry with parallax. The
// plateau's grass is solid (you can walk to the cliff edge and fall off,
// into the scene's fall_respawn); it leaves out `hole`, the area the scene
// grounds itself (the truck bed demo's destroyable tiles). The cliffs and
// mountains are CombineMesh batches with baked ambient occlusion, and
// neither collide nor take hits.
//
// Each peak is a cube turned onto a corner inside a non-uniformly scaled
// frame, which shears it into a leaning, spiky parallelepiped; cliff
// pillars are tilted boxes. A gold sun low on one side and a red
// counter-light from the other colour their faces; the mountain shader adds
// haze toward the horizon colour (complete at the far plane, so the plain
// meets the sky without an edge) and a gold rim near the sun.
//
// Azimuths are radians around the play area's centre, from +x toward +z
// (so -pi/2 is -z, ahead of the truck bed demo's spawn).

let SUN = { azimuth = -1.1 elevation = 0.06 radius = 0.04 }

// Ranges on the plain: margin beyond the plateau (m), spacing between peaks
// along the ring (m), peak height range above the plain (m; the plateau top
// is CLIFF metres up), base colour, and arcs [azimuth, half width] where the
// range stands. Peaks ease down to nothing at an arc's ends.
let CLIFF = 60.0
let LAYERS = [
    { margin = 70.0 spacing = 26.0 low = 48.0 high = 66.0 color = [0.86, 0.48, 0.58]
      arcs = [[1.75, 0.9]] },
    { margin = 125.0 spacing = 32.0 low = 58.0 high = 84.0 color = [0.90, 0.56, 0.64]
      arcs = [[2.3, 1.1], [0.95, 0.35]] },
    { margin = 180.0 spacing = 38.0 low = 66.0 high = 96.0 color = [0.94, 0.64, 0.70]
      arcs = [[1.4, 0.8], [3.0, 0.45]] }
]

let SKY = {
    zenith_color = [0.30, 0.22, 0.46], zenith_point = 0.85,
    upper_color = [0.95, 0.52, 0.62], upper_point = 0.22,
    horizon_color = [1.0, 0.62, 0.72], sun_side_power = 2.0,
    sun_horizon_color = [0.98, 0.34, 0.26], glow_strength = 1.0,
    glow_color = [1.0, 0.76, 0.38], glow_width = 1.0,
    sun_direction = [0.0, 0.0, -1.0], cloud_strength = 0.55,
    cloud_color = [0.86, 0.62, 0.84], below_point = -0.6,
    below_color = [0.98, 0.58, 0.62], cloud_seed = 0.8
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

// How much of a ring stands at `azimuth`: 1 inside an arc, easing to 0
// over 0.3 rad past its ends.
fn arc_weight(layer, azimuth) {
    let weight = 0.0
    for arc in layer.arcs {
        let off = Math.abs(wrap(azimuth - arc[0]))
        let w = 1.0 - Math.smoothstep(off, arc[1], arc[1] + 0.3)
        if w > weight { weight = w }
    }
    return weight
}

fn ring_point(ring, index, count, seed) {
    let step = Math.tau / count
    let phi = -Math.pi + (index + (noise(seed + 0.1) - 0.5) * 0.4) * step
    let a = ring.a + (noise(seed + 0.3) - 0.5) * 6.0
    let b = ring.b + (noise(seed + 0.3) - 0.5) * 6.0
    let speed = Math.sqrt(a * a * Math.sin(phi) * Math.sin(phi) + b * b * Math.cos(phi) * Math.cos(phi))
    return {
        x = ring.center[0] + a * Math.cos(phi)
        z = ring.center[1] + b * Math.sin(phi)
        azimuth = Math.atan2(b * Math.sin(phi), a * Math.cos(phi))
        width = speed * step
    }
}

// Peaks are evenly spaced in the ellipse's parameter, not in distance, so
// each one's width follows the local ring speed: wider along the long sides.
fn peak(layer, ring, index, count, floor_y, material) {
    let seed = layer.margin * 7.0 + index
    let p = ring_point(ring, index, count, seed)
    let h = (layer.low + (layer.high - layer.low) * noise(seed)) * arc_weight(layer, p.azimuth)
    // The unit cube turned onto an edge reaches about 0.75 of its frame's
    // height above its centre, which sits on the plain.
    let height = h / 0.75
    let width = p.width * (1.3 + 0.6 * noise(seed + 0.7))
    let c = layer.color
    return T.position(p.x, floor_y, p.z)
        .rotation(0.0, Math.pi / 2.0 - p.azimuth + (noise(seed + 1.1) - 0.5) * 0.6, 0.0)
        .scale(width, height, width * 0.8) {
        T.rotation((noise(seed + 1.3) - 0.5) * 0.5, 0.35 + 0.55 * noise(seed + 1.7),
            0.785 + (noise(seed + 1.9) - 0.5) * 0.6) {
            R.cube() { material C.rgba(c[0], c[1], c[2], 1.0) }
        }
    }
}

// One range as one combined mesh. CombineMesh keeps the first peak's colour
// for the whole ring; the baked occlusion darkens where peaks overlap.
fn layer_peaks(layer, center, plateau, floor_y, material) {
    let ring = { center = center a = plateau[0] + layer.margin b = plateau[1] + layer.margin }
    let perimeter = Math.tau * Math.sqrt((ring.a * ring.a + ring.b * ring.b) / 2.0)
    let count = Math.floor(perimeter / layer.spacing)
    return CombineMesh {
        name = "mountain_ring_" + layer.margin
        AmbientOcclusion.texcoord0().proximity_radius(10.0).cavity_radius(5.0).strength(0.55)
        for index in range(count) {
            if arc_weight(layer, ring_point(ring, index, count, layer.margin * 7.0 + index).azimuth) > 0.25 {
                peak(layer, ring, index, count, floor_y, material)
            }
        }
    }
}

// One rough rock pillar of the cliff face, from below the plain up to just
// under the grass, leaning a little.
fn pillar(x, z, outward, seed, material) {
    let width = 9.0 + 6.0 * noise(seed)
    let depth = 6.0 + 5.0 * noise(seed + 0.4)
    let top = -0.3 - 1.6 * noise(seed + 0.8)
    let height = CLIFF + 4.0 + top
    let push = 1.0 + 3.0 * noise(seed + 1.2)
    return T.position(x + outward[0] * push, top - height * 0.5, z + outward[1] * push)
        .rotation((noise(seed + 1.6) - 0.5) * 0.16, (noise(seed + 2.0) - 0.5) * 0.5, (noise(seed + 2.4) - 0.5) * 0.16)
        .scale(width, height, depth) {
        R.cube() { material C.rgba(0.66, 0.42, 0.44, 1.0) }
    }
}

// Pillars every ~9 m along one side of the plateau, from `start` to
// `finish` ([x, z]), pushed out along `outward`.
fn cliff_side(start, finish, outward, salt, material) {
    let dx = finish[0] - start[0]
    let dz = finish[1] - start[1]
    let count = Math.ceil(Math.sqrt(dx * dx + dz * dz) / 9.0)
    return T {
        for index in range(count + 1) {
            pillar(start[0] + dx * index / count, start[1] + dz * index / count, outward, salt + index * 3.1, material)
        }
    }
}

// A solid grass slab between x0..x1 and z0..z1, its top at y = 0, with rock
// beneath down to the plain.
fn plateau_slab(x0, x1, z0, z1, grass, rock) {
    let w = x1 - x0
    let d = z1 - z0
    if w <= 0.0 || d <= 0.0 { return T {} }
    let cx = (x0 + x1) * 0.5
    let cz = (z0 + z1) * 0.5
    return T {
        name = "mountain_sunrise_plateau_slab"
        T.position(cx, 0.0, cz).rotation(-Math.pi / 2.0, 0.0, 0.0).scale(w, d, 1.0) {
            R.plane() {
                grass
                C.rgba(1.0, 1.0, 1.0, 1.0)
                Texture.with_uri("assets/textures/grass/cartoon-grass-tufts.png")
                TextureFiltering.linear()
            }
        }
        T.position(cx, -0.5, cz).scale(w, 1.0, d) {
            Zone.cube([0.5, 0.5, 0.5]) { Collidable.static().friction(0.85) {} }
        }
        T.position(cx, -(CLIFF + 0.04) * 0.5 - 0.02, cz).scale(w, CLIFF - 0.04, d) {
            R.cube() { rock C.rgba(0.58, 0.36, 0.40, 1.0) }
        }
    }
}

// options (all optional):
//   center [x, z]: the middle of the plateau and the ranges (default the
//     truck bed demo's range centre);
//   plateau [half x, half z]: the plateau's size (default 110 x 70);
//   hole [min_x, max_x, min_z, max_z]: an area of the plateau the scene
//     grounds itself (default the truck bed demo's tile range); null for none;
//   grass_albedo [r, g, b]: the grass colour (default pink);
//   sky: overrides for SKY fields; ambient [r, g, b];
//   sun_light, counter_light: [r, g, b, intensity].
// Returns { world, grass, sky_material, mountain_material, sun_light,
//           counter_light, ambient }. Add `world` to the scene once. The far ranges are up to
// about 300 m from the centre and the plain fades out at 400 m, so the
// camera's far plane should reach about 420 m.
export fn mountain_sunrise(options) {
    if options == null { options = {} }
    let center = [0.0, -7.0]
    if options["center"] != null { center = options.center }
    let plateau = [110.0, 70.0]
    if options["plateau"] != null { plateau = options.plateau }
    let hole = [-48.0, 48.0, -23.0, 7.0]
    if options["hole"] != null { hole = options.hole }
    if options["hole"] == null && options["center"] != null { hole = null }
    // Linear, like every albedo: the sRGB swapchain brightens it on output.
    // This displays as #e78eb3 where lit (the old [0.93, 0.66, 0.70] showed as
    // a greyish #e0c0c5, and grey in shade).
    let grass_albedo = [1.0, 0.34, 0.56]
    if options["grass_albedo"] != null { grass_albedo = options.grass_albedo }
    let floor_y = -CLIFF

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
        haze_color = sky_inputs.horizon_color, haze_near = 60.0,
        haze_sun_color = sky_inputs.sun_horizon_color, haze_far = 400.0,
        rim_color = sky_inputs.glow_color, haze_strength = 1.0,
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
    // The plateau's grass; the scene can reuse it for its own ground.
    let grass = cartoon_grass("far", { albedo = grass_albedo })
    let x0 = center[0] - plateau[0]
    let x1 = center[0] + plateau[0]
    let z0 = center[1] - plateau[1]
    let z1 = center[1] + plateau[1]
    // Without a hole, one slab; with one, four slabs around it.
    let h = hole
    if h == null { h = [center[0], center[0], center[1], center[1]] }
    let world = T {
        name = "mountain_sunrise"
        T {
            name = "mountain_sunrise_plateau"
            plateau_slab(x0, x1, z0, h[2], grass, mountain_material)
            plateau_slab(x0, x1, h[3], z1, grass, mountain_material)
            plateau_slab(x0, h[0], h[2], h[3], grass, mountain_material)
            plateau_slab(h[1], x1, h[2], h[3], grass, mountain_material)
        }
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
                // Rock under the hole, deep enough that a fall through it
                // still reaches the scene's fall_respawn first.
                if hole != null {
                    T.position((h[0] + h[1]) * 0.5, -(CLIFF + 30.0) * 0.5, (h[2] + h[3]) * 0.5)
                        .scale(h[1] - h[0], CLIFF - 30.0, h[3] - h[2]) {
                        R.cube() { mountain_material C.rgba(0.58, 0.36, 0.40, 1.0) }
                    }
                }
                CombineMesh {
                    name = "mountain_sunrise_cliffs"
                    AmbientOcclusion.texcoord0().proximity_radius(4.0).cavity_radius(2.0).strength(0.5)
                    cliff_side([x0, z0], [x1, z0], [0.0, -1.0], 1.0, mountain_material)
                    cliff_side([x0, z1], [x1, z1], [0.0, 1.0], 401.0, mountain_material)
                    cliff_side([x0, z0], [x0, z1], [-1.0, 0.0], 801.0, mountain_material)
                    cliff_side([x1, z0], [x1, z1], [1.0, 0.0], 1201.0, mountain_material)
                }
                // The plain, out past the far plane.
                T.position(center[0], floor_y - 0.25, center[1]).scale(1000.0, 0.5, 1000.0) {
                    R.cube() { mountain_material C.rgba(0.62, 0.40, 0.46, 1.0) }
                }
                for layer in LAYERS { layer_peaks(layer, center, plateau, floor_y, mountain_material) }
            }
        }
        ambient
        T.position(sun_from[0], sun_from[1], sun_from[2]) { sun_light }
        T.position(counter_from[0], counter_from[1], counter_from[2]) { counter_light }
    }
    return {
        world = world
        grass = grass
        sky_material = sky_material
        mountain_material = mountain_material
        sun_light = sun_light
        counter_light = counter_light
        ambient = ambient
    }
}
