import { grid_surface_material } from "../assets/components/materials/grid_surface.mms"

RendererSettings { window_size(1000, 640) }
BGC.rgba(0.025, 0.03, 0.045, 1.0)
RenderGraph.off()

let local_grid = grid_surface_material()
local_grid.set_input("local_extent", [3.0, 3.0])
let world_grid = grid_surface_material()
world_grid.set_input("spacing", 0.35)
world_grid.set_input("world_space", 1.0)

T.position(-1.7, 0.0, 0.0).scale(3.0, 3.0, 1.0) {
    R.plane() { local_grid C.rgba(0.4, 0.8, 1.0, 1.0) }
}
T.position(1.7, 0.0, 0.0).rotation(0.0, 0.0, 0.15).scale(3.0, 3.0, 1.0) {
    R.plane() { world_grid C.rgba(1.0, 0.65, 0.45, 1.0) }
}
I.speed(2.0) { T.position(0.0, 0.0, 7.0) { C3D {} } }
