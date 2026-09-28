// Renderable primitive showcase and desktop fixture for multi-layer Bloom.
//
// It deliberately uses the existing one-layer Bloom configuration until
// Bloom.layers(n) is implemented. The implementation task changes this to
// `layers(3)` for the wide-glow comparison while retaining `layers(1)` as the
// compatibility/control mode.
//
// The grid contains every shape exported by assets/components/primitives.mms.
// Strong, coloured emissive surfaces against black make the tight and wide
// portions of bloom easy to inspect without depending on scene lighting.

RendererSettings { window_size(1440, 960) }
BGC.rgba(0.0, 0.0, 0.0, 1.0)
AL.rgb(0.08, 0.08, 0.08)

RenderGraph {
    EmissivePass { BlurPass { radius_ndc(0.045) half_res(true) } }
    Bloom { intensity(1.0) radius_ndc(0.045) emissive_scale(1.25) half_res(true) }
}

fn palette_color(index) {
    let palette = [
        [0.94, 0.32, 0.42, 1.0],
        [0.96, 0.55, 0.22, 1.0],
        [0.92, 0.82, 0.25, 1.0],
        [0.38, 0.86, 0.48, 1.0],
        [0.26, 0.76, 0.90, 1.0],
        [0.34, 0.48, 0.94, 1.0],
        [0.62, 0.38, 0.94, 1.0],
        [0.88, 0.34, 0.78, 1.0],
    ]
    return palette[index % 8]
}

fn glow_shape(shape_index, color, emissive_intensity) {
    // Emissive on this container is inherited by the contained renderable.
    // The eighteen cases mirror assets/components/primitives.mms exactly.
    return T.scale(0.78, 0.78, 0.78) {
        Emissive.on() { intensity(emissive_intensity) }

        if shape_index == 0 {
            R.cube() { C.rgba(color[0], color[1], color[2], color[3]) }
        } else if shape_index == 1 {
            R.sphere() { C.rgba(color[0], color[1], color[2], color[3]) }
        } else if shape_index == 2 {
            R.plane() { C.rgba(color[0], color[1], color[2], color[3]) }
        } else if shape_index == 3 {
            R.triangle() { C.rgba(color[0], color[1], color[2], color[3]) }
        } else if shape_index == 4 {
            R.square() { C.rgba(color[0], color[1], color[2], color[3]) }
        } else if shape_index == 5 {
            R.wireframe_square(0.10) { C.rgba(color[0], color[1], color[2], color[3]) }
        } else if shape_index == 6 {
            R.annulus_2d() { C.rgba(color[0], color[1], color[2], color[3]) }
        } else if shape_index == 7 {
            R.circle_2d() { C.rgba(color[0], color[1], color[2], color[3]) }
        } else if shape_index == 8 {
            R.cone() { C.rgba(color[0], color[1], color[2], color[3]) }
        } else if shape_index == 9 {
            R.polygon("gallery/hexagon/v1", [[0.0, 0.5], [0.43, 0.25], [0.43, -0.25], [0.0, -0.5], [-0.43, -0.25], [-0.43, 0.25]]) { C.rgba(color[0], color[1], color[2], color[3]) }
        } else if shape_index == 10 {
            R.wireframe_box() { C.rgba(color[0], color[1], color[2], color[3]) }
        } else if shape_index == 11 {
            R.wireframe_sphere() { C.rgba(color[0], color[1], color[2], color[3]) }
        } else if shape_index == 12 {
            R.wireframe_icosahedron() { C.rgba(color[0], color[1], color[2], color[3]) }
        } else if shape_index == 13 {
            R.tetrahedron() { C.rgba(color[0], color[1], color[2], color[3]) }
        } else if shape_index == 14 {
            R.icosahedron() { C.rgba(color[0], color[1], color[2], color[3]) }
        } else if shape_index == 15 {
            R.star() { C.rgba(color[0], color[1], color[2], color[3]) }
        } else if shape_index == 16 {
            R.heart() { C.rgba(color[0], color[1], color[2], color[3]) }
        } else {
            R.partial_annulus_2d() { C.rgba(color[0], color[1], color[2], color[3]) }
        }
    }
}

fn glow_cell(row, column) {
    let index = row * 12 + column
    let color = palette_color(index)
    let emissive_intensity = 1.5 + (index % 5) * 0.25

    return T {
        Style {
            display("inline-block")
            width(3.0)
            height(3.0)
        }
        glow_shape(index % 18, color, emissive_intensity)
    }
}

fn glow_row(row) {
    return T {
        Style { display("block") width(36.0) height(3.0) }
        for column in range(12) {
            glow_cell(row, column)
        }
    }
}

// LayoutRoot supplies the requested 12 x 12 grid. Its position centers the
// 36-gu composition in front of the fixed desktop camera.
T.position(-5.4, 5.4, -4.0) {
    LayoutRoot {
        name = "multi_layer_bloom_primitive_grid"
        available_width(36.0)
        available_height(36.0)
        unit_scale(0.30)

        for row in range(12) {
            glow_row(row)
        }
    }
}

// Deliberately no fps_rotation(): WASD/RF moves in local +Z-forward mode,
// while Q/E uses the local Z roll axis. The initial view faces the grid along
// the Camera3D's local -Z direction.
Input.speed(3.0) {
    InputTransformMode.forward_z() { roll_axis_z() }
    T.position(0.0, 0.0, 9.0) {
        C3D { Pointer {} }
    }
}
