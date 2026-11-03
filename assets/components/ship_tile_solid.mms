import { pool_tile_material } from "./materials/pool_tiles.mms"

// Shared ceramic finish and texture, with metre-based shader projection.
let tiles = pool_tile_material()
export fn ship_tile_solid(position, size, color) {
    return T.position(position[0], position[1], position[2]).scale(size[0], size[1], size[2]) {
        R.cube() {
            C.rgba(1.0, 1.0, 1.0, 1.0)
            tiles
            Texture.with_uri("assets/textures/pool/white-tiles-surface.png")
            TextureFiltering.linear()
            Raycastable.enabled()
        }
        Zone.cube([0.5, 0.5, 0.5]) { Collidable.static().friction(0.8) {} }
    }
}
