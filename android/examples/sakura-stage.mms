// Native Android rooftop lighting demo. No controllers, avatars or post effects.
import { f32, vec3 } from "types"
import { studio_stage } from "../../assets/components/studio_stage.mms"
import { pool_building, ceramic_box } from "../../assets/components/buildings/pool_building.mms"
import { pool_tile_material } from "../../assets/components/materials/pool_tiles.mms"
import { anime_surface } from "../../assets/components/materials/anime_surface.mms"
import { matcap_metal } from "../../assets/components/materials/matcap_metal.mms"
import { ui_unlit_material } from "../../assets/components/materials/ui_unlit.mms"

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
let ui_material = ui_unlit_material()
let roof_height = 25.6
let white = [0.94, 0.96, 0.97]
let pink = [0.88, 0.65, 0.73]
let blue = [0.64, 0.78, 0.86]

// Avatar-free player: a sliding capsule and velocity live on the locomotion
// root. The host supplies head and hand poses from its single XR session.
Gravity {
    let player_velocity = Velocity {
        T {
            name = "sakura_mobile_player"
            T.position(0.0, -0.80, 0.0) {
                Zone.capsule_y(0.25, 0.55) {
                    Collidable.slide().movement_target("/[name='sakura_mobile_player']")
                }
            }
            InputXR.on() {
                InputXRGamepad { locomotion() speed(1.4) }
                T { CXR {} }
                XRHand.new(true, "Left", "Aim") {
                    T.position(0.0, -20.0, 0.0) {
                        Pointer { name = "sakura_mobile_left_pointer" }
                        T.position(0.0, 0.0, -5.0).scale(0.002, 0.002, 10.0) {
                            R.cube() { ui_material C.rgba(0.1, 1.0, 1.0, 1.0) }
                        }
                    }
                }
                XRHand.new(true, "Right", "Aim") {
                    T.position(0.0, -20.0, 0.0) {
                        Pointer { name = "sakura_mobile_right_pointer" }
                        T.position(0.0, 0.0, -5.0).scale(0.002, 0.002, 10.0) {
                            R.cube() { ui_material C.rgba(0.1, 1.0, 1.0, 1.0) }
                        }
                    }
                }
            }
        }
    }
    player_velocity
    on_global("XrButtonDown", fn(event) {
        if event.control == "ButtonB" && player_velocity.grounded() {
            player_velocity.translate_world([0.0, 4.5, 0.0])
        }
    })
}

// Place the authored rooftop spawn at LOCAL eye height without enabling the
// engine's separate XR driver. Physical headset motion remains one-to-one.
T.position(18.0, -(roof_height + 1.6), -3.0) {
    name = "android_sakura_world"
    pool_building("sakura_stage_tower", [-18.0, 0.0, 0.0], 8, 1.0, true, tiles, pink, true, false)
    let editor_ui_anchor = T.position(-18.65, roof_height + 2.4, 0.8) {
        name = "sakura_mobile_editor_ui"
        EditorUI { panels([{ panel = "settings" }]) ui_material Opacity.opacity(0.998) }
    }
    editor_ui_anchor
    let editor_icons_ready = false
    on_global("FrameTick", fn(event) {
        if !editor_icons_ready && editor_ui_anchor.query("#editor_settings_panel_root") != null {
            // The shared accordion icon is authored as emissive. This app's
            // unlit UI shader already displays it without a bloom pass.
            for emission in editor_ui_anchor.query_all("emissive") { emission.off() }
            editor_icons_ready = true
        }
    })
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
    // Sakura conversion's rooftop pistol table, with static collision.
    T.position(-20.3, roof_height, 0.2) {
        name = "sakura_pistol_table"
        ceramic_box([0.0, 0.90, 0.0], [2.8, 0.12, 1.15], white, tiles)
        T.position(0.0, 0.974, 0.0).scale(2.64, 0.025, 1.0) {
            R.cube() { stage_material C.rgba(0.91, 0.72, 0.78, 1.0) }
        }
        for x in [-1.18, 1.18] {
            for z in [-0.43, 0.43] {
                T.position(x, 0.42, z).scale(0.10, 0.84, 0.10) {
                    R.cube() { stage_material C.rgba(0.30, 0.32, 0.40, 1.0) }
                }
            }
        }
    }
    for x in [-21.15, -20.30, -19.45] {
        T.position(x + 0.05, roof_height + 1.015, 0.2).rotation_quat(0.5, -0.5, -0.5, 0.5) {
            T.position(0.0, 0.0, 0.05).rotation(0.0, 3.14159265, 0.0).scale(0.085, 0.085, 0.085) {
                let pistol = GLTF.new("assets/models/sakura-machine-pistol.glb") { stage_material }
                // The source model includes a glowing detail. The current
                // mobile material slice is opaque, so keep its color but turn
                // off the imported emissive modifier until emission is supported.
                on(pistol, "GLTFInitialized", fn(event) {
                    for emission in event.gltf.query_all("emissive") { emission.off() }
                })
                pistol
            }
        }
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
Editor {}
