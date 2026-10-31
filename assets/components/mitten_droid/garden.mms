// One floating ceramic terrace, one raised Sakura bed, and a low perimeter rail.
// The two shells share metre-scale scenery while keeping their own UI sizing.
import { f32 } from "types"
import { pool_tile_material } from "../materials/pool_tiles.mms"
import { anime_surface } from "../materials/anime_surface.mms"
import { sakura_tree_custom } from "../plants/sakura_tree.mms"

fn box_part(position, dimensions, color) {
    return T.position(position[0],position[1],position[2]).scale(dimensions[0],dimensions[1],dimensions[2]) {
        R.cube() { C.rgba(color[0],color[1],color[2],1) }
    }
}

export fn shell_garden(ui_right, floor_y, ui_z, spawn_z) {
    let tiles = pool_tile_material()
    let solid = anime_surface()
    let blossom_definition = MaterialDefinition(MaterialBase()).fragment("assets/shaders/sakura-blossom.frag")
        .inputs({ brightness: f32 })
    let blossoms = Material(blossom_definition).inputs({ brightness = 1.0 })
    let left_edge = -ui_right - 1.0
    let right_edge = ui_right + 2.8
    let front_edge = -1.4
    let back_edge = spawn_z - ui_z + 1.6
    let x_span = right_edge-left_edge
    let z_span = back_edge-front_edge
    let bed_x = ui_right+1.2
    let bed_z = 0.8
    let cream = [0.98,0.94,0.90]
    let pink = [0.96,0.87,0.84]
    return T.position(0,floor_y,ui_z) {
        name = "shell_floating_garden"
        Raycastable.disabled()
        // One retained ceramic batch: world-space tile mapping stays consistent
        // across the floor slab and all four sides of the raised planter.
        CombineMesh {
            name = "shell_terrace_tiles"
            tiles
            Texture.with_uri("assets/textures/pool/white-tiles-surface.png")
            TextureFiltering.linear()
            box_part([(left_edge+right_edge)/2,-0.11,(front_edge+back_edge)/2],[x_span,0.22,z_span],cream)
            box_part([bed_x-0.83,0.23,bed_z],[0.14,0.46,1.9],pink)
            box_part([bed_x+0.83,0.23,bed_z],[0.14,0.46,1.9],pink)
            box_part([bed_x,0.23,bed_z-0.88],[1.52,0.46,0.14],pink)
            box_part([bed_x,0.23,bed_z+0.88],[1.52,0.46,0.14],pink)
        }
        T.position(bed_x,0.37,bed_z) {
            name = "shell_flower_bed"
            T.scale(1.52,0.12,1.62) { R.cube() { solid C.rgba(0.20,0.135,0.105,1) } }
            T.position(0,0.07,0) {
                name = "shell_sakura_tree"
                sakura_tree_custom(3,0.85,0.15,solid,blossoms)
            }
            CombineMesh {
                name = "shell_bed_flowers"
                blossoms
                Texture.with_uri("assets/textures/cherry-blossoms/sakura-blossom-256.png")
                TextureFiltering.linear()
                for x in [-0.56,0.56] {
                    for z in [-0.58,-0.18,0.25,0.61] {
                        T.position(x,0.16,z).rotation(-0.25,x,0).scale(0.18,0.18,0.18) { R.square() {} }
                    }
                }
            }
        }
        CombineMesh {
            name = "shell_low_railing"
            solid
            C.rgba(0.64,0.46,0.34,1)
            for z in [front_edge+0.10,back_edge-0.10] {
                for index in range(9) {
                    T.position(left_edge+0.1+(x_span-0.2)*index/8,0,z) { R.cylinder(0.024,0.62,8) {} }
                }
                for y in [0.25,0.62] {
                    T.position(left_edge+0.1,y,z).rotation(0,0,-1.57079633) { R.cylinder(0.025,x_span-0.2,10) {} }
                }
            }
            for x in [left_edge+0.10,right_edge-0.10] {
                for index in range(1,6) {
                    T.position(x,0,front_edge+0.1+(z_span-0.2)*index/6) { R.cylinder(0.024,0.62,8) {} }
                }
                for y in [0.25,0.62] {
                    T.position(x,y,front_edge+0.1).rotation(1.57079633,0,0) { R.cylinder(0.025,z_span-0.2,10) {} }
                }
            }
        }
    }
}
