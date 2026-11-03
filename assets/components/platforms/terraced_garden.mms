// Floating garden prefab. Local origin is the center of the upper deck's floor.
// A 6x4 m upper deck slopes along -Z via a 3x5 m ramp, dropping 1.4 m
// onto a 7x5 m landing. Materials are custom and geometry is retained/batched.
import { f32 } from "types"
import { pool_tile_material } from "../materials/pool_tiles.mms"
import { anime_surface } from "../materials/anime_surface.mms"
import { sakura_tree_custom } from "../plants/sakura_tree.mms"

fn box_at(x, y, z, width, height, depth) {
    return T.position(x,y,z).scale(width,height,depth) { R.cube() {} }
}

fn rail_x(x, y, z, length, segments) {
    return T {
        for i in range(segments+1) {
            box_at(x+length*i/segments,y+0.5,z,0.055,1.0,0.055)
        }
        for height in [0.40,1.0] { box_at(x+length/2,y+height,z,length,0.055,0.055) }
    }
}

fn rail_z(x, y, z, length, segments) {
    return T {
        for i in range(segments+1) {
            box_at(x,y+0.5,z+length*i/segments,0.055,1.0,0.055)
        }
        for height in [0.40,1.0] { box_at(x,y+height,z+length/2,0.055,0.055,length) }
    }
}

export fn terraced_garden() {
    let tiles = pool_tile_material()
    let solid = anime_surface()
    let blossom_definition = MaterialDefinition(MaterialBase()).fragment("assets/shaders/sakura-blossom.frag")
        .inputs({ brightness: f32 })
    let blossoms = Material(blossom_definition).inputs({ brightness = 1.0 })
    return T {
        name = "terraced_garden"
        Raycastable.disabled()
        CombineMesh {
            name = "terraced_garden_tiles"
            tiles
            Texture.with_uri("assets/textures/pool/white-tiles-surface.png")
            TextureFiltering.linear()
            C.rgba(0.98,0.94,0.90,1)
            box_at(0,-0.11,0,6,0.22,4)
            // Rotate about the top of the ramp so it meets both level decks.
            T.position(0,-0.7,-4.5).rotation(-0.2730087,0,0) {
                box_at(0,-0.11,0,3,0.22,5.19230199)
            }
            box_at(0,-1.51,-9.5,7,0.22,5)
            C.rgba(0.96,0.87,0.84,1)
            for x in [-0.98,0.98] { box_at(x,-1.17,-10.1,0.14,0.46,2.1) }
            for z in [-11.08,-9.12] { box_at(0,-1.17,z,1.82,0.46,0.14) }
        }
        CombineMesh {
            name = "terraced_garden_railings"
            solid
            C.rgba(0.64,0.46,0.34,1)
            // Open the near edge for an approach, and the far edge for the ramp.
            for z in [-1.94,1.94] {
                rail_x(-2.94,0,z,1.44,3)
                rail_x(1.50,0,z,1.44,3)
            }
            for x in [-2.94,2.94] { rail_z(x,0,-1.94,3.88,8) }
            for x in [-1.44,1.44] {
                for i in range(11) { box_at(x,0.5-1.4*i/10,-2-5*i/10,0.055,1.0,0.055) }
                for height in [0.40,1.0] {
                    T.position(x,height-0.7,-4.5).rotation(-0.2730087,0,0) {
                        box_at(0,0,0,0.055,0.055,5.19230199)
                    }
                }
            }
            // Lower landing: leave only the ramp mouth open.
            rail_x(-3.44,-1.4,-11.94,6.88,14)
            rail_x(-3.44,-1.4,-7.06,1.94,4)
            rail_x(1.50,-1.4,-7.06,1.94,4)
            for x in [-3.44,3.44] { rail_z(x,-1.4,-11.94,4.88,10) }
        }
        T.position(0,-1.02,-10.1) {
            name = "terraced_garden_tree"
            T.scale(1.82,0.12,1.82) { R.cube() { solid C.rgba(0.20,0.135,0.105,1) } }
            T.position(0,0.07,0) { sakura_tree_custom(3,1.05,0.18,solid,blossoms) }
        }
    }
}
