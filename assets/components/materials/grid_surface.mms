import { f32, vec2 } from "types"

// One Surface definition shared by independently configured grid instances.
// local_extent is the displayed plane's width/height in local grid metres.
let definition = MaterialDefinition(MaterialBase())
    .fragment("assets/shaders/grid-surface.frag")
    .inputs({
        spacing: f32
        world_space: f32
        local_extent: vec2
        minor_width: f32
        major_width: f32
        axis_width: f32
        fade_start: f32
        fade_end: f32
    })

export fn grid_surface_material() {
    return Material(definition).inputs({
        spacing = 0.25
        world_space = 0.0
        local_extent = [10.0, 10.0]
        minor_width = 1.0
        major_width = 1.8
        axis_width = 2.0
        fade_start = 32.0
        fade_end = 96.0
    })
}
