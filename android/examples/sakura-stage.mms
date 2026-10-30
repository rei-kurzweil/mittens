// Native Android rooftop lighting demo. No controllers, avatars or post effects.
import { f32, vec3 } from "types"
import { studio_stage } from "../../assets/components/studio_stage.mms"
import { pool_building, ceramic_box } from "../../assets/components/buildings/pool_building.mms"
import { pool_tile_material } from "../../assets/components/materials/pool_tiles.mms"
import { anime_surface } from "../../assets/components/materials/anime_surface.mms"
import { matcap_metal } from "../../assets/components/materials/matcap_metal.mms"

BGC.rgba(0.23, 0.32, 0.62, 1.0)
AL.rgb(0.08, 0.025, 0.035)
let sky_definition = MaterialDefinition(MaterialBase())
    .fragment("android/shaders/sakura-sky.frag")
    .inputs({ low_color: vec3, low_point: f32, high_color: vec3, high_point: f32,
        sun_direction: vec3, sun_angular_radius: f32, sun_color: vec3, sun_strength: f32 })
BG {
    name = "sakura_sky"
    T.scale(80.0, 80.0, 80.0) {
        R.sphere() { Material(sky_definition).inputs({
            low_color = [1.0, 0.72, 0.66], low_point = -0.12,
            high_color = [0.23, 0.32, 0.62], high_point = 0.82,
            sun_direction = [-0.35, 0.30, -0.88], sun_angular_radius = 0.055,
            sun_color = [1.0, 0.84, 0.60], sun_strength = 1.0
        }) }
    }
}
// Directional light positions are surface-to-light directions, not origins.
T.position(0.0, 1.0, 0.0) { DL.color(0.22, 0.40, 1.0).intensity(0.8) }

let tiles = pool_tile_material()
let stage_material = anime_surface()
let chrome = matcap_metal()
let roof_height = 25.6
let white = [0.94, 0.96, 0.97]
let pink = [0.88, 0.65, 0.73]
let blue = [0.64, 0.78, 0.86]

// Place the authored rooftop spawn at LOCAL eye height without enabling the
// engine's separate XR driver. Physical headset motion remains one-to-one.
T.position(18.0, -(roof_height + 1.6), -3.0) {
    name = "android_sakura_world"
    pool_building("sakura_stage_tower", [-18.0, 0.0, 0.0], 8, 1.0, true, tiles, pink, true, false)
    // Keep exactly one cube per surrounding building in this first mobile scene.
    let neighbors = [
        { position = [-18.0, 0.0, -25.0] floors = 8 color = white },
        { position = [18.0, 0.0, 0.0] floors = 8 color = blue },
        { position = [-18.0, 0.0, 25.0] floors = 8 color = white },
        { position = [18.0, 0.0, -25.0] floors = 8 color = blue },
        { position = [18.0, 0.0, 25.0] floors = 8 color = blue },
        { position = [-39.0, 0.0, -10.0] floors = 2 color = blue },
        { position = [39.0, 0.0, 13.0] floors = 3 color = pink },
        { position = [-18.0, 0.0, 47.0] floors = 2 color = white },
    ]
    let index = 0
    for building in neighbors {
        let height = building.floors * 3.2
        T.position(building.position[0], height / 2.0 - 0.22, building.position[2])
            .scale(14.0, height, 12.0) {
            name = "sakura_neighbor_cube_" + index
            R.cube() { tiles C.rgba(building.color[0], building.color[1], building.color[2], 1.0)
                Texture.with_uri("assets/textures/pool/white-tiles-surface.png") }
        }
        index = index + 1
    }
    T.position(-18.0, roof_height + 0.15, -2.0).scale(0.36, 0.36, 0.36) {
        stage_material
        studio_stage("sakura_rooftop_sound_stage")
    }
    // White stage lights; no shadow maps or extra emission pass.
    for x in [-22.0, -14.0] {
        T.position(x, roof_height + 3.1, -1.2) {
            PL.color(1.0, 1.0, 1.0).intensity(2.2).distance(10.0)
        }
    }
    // A static matcap prop checks the same opaque custom texture interface.
    T.position(-21.4, roof_height + 0.8, -1.7).scale(0.6, 0.6, 0.6) {
        R.sphere() { chrome Texture.with_uri("assets/textures/pool/chrome-matcap.png") }
    }
}
