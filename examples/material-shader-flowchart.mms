// Desktop material authoring -> renderer bindings -> shader stages.
// Run: cargo run --release -- load examples/material-shader-flowchart.mms
// Like the OpenXR flowchart: clickable cards, orthogonal wires, movable notes.
// Outlined cards exist today. Gold-filled cards describe proposed integration.
// Code snippets and proposed component names are displayed text, not live effects.

import { info_panel, info_panel_body } from "../assets/components/ui/info_panel.mms"
import { material_shader_notes } from "../assets/components/diagrams/material_shader_notes.mms"

RendererSettings { window_size(1920, 1200) }
BGC.rgba(0.009, 0.014, 0.025, 1.0)
RenderGraph.off()

let cyan = [0.24, 0.82, 0.95, 1.0]
let violet = [0.74, 0.60, 1.0, 1.0]
let green = [0.40, 0.90, 0.68, 1.0]
let gold = [1.0, 0.76, 0.38, 1.0]
let white = [0.91, 0.95, 1.0, 1.0]
let muted = [0.66, 0.74, 0.83, 1.0]
let dark = [0.018, 0.030, 0.049, 1.0]
let black = [0.025, 0.025, 0.035, 1.0]
let notes = material_shader_notes()
let detail_state = { selected_id = "material_base" body_mount = null }

fn words(value, size, tint, box_w, box_h) {
    return T {
        Style {
            display("block") width(box_w) height(box_h) font_size(size)
            word_wrap("normal") color(tint)
        }
        Text { value C.rgba(tint[0], tint[1], tint[2], tint[3]) }
    }
}

fn paragraph(value, size, tint) {
    return T {
        Style { display("block") width(100%) font_size(size) word_wrap("break-word") color(tint) }
        Text { value C.rgba(tint[0], tint[1], tint[2], tint[3]) }
    }
}

fn detail_section(label, value) {
    return T {
        Style { display("flex") flex_direction("column") width(100%) row_gap(0.25) margin_top(0.6) }
        paragraph(label, 0.76, cyan)
        paragraph(value, 0.96, white)
    }
}

fn detail_content() {
    let note = notes[detail_state.selected_id]
    return T {
        name = "material_shader_detail_content"
        Style { display("flex") flex_direction("column") width(100%) row_gap(0.35) padding(0.4) }
        paragraph(note.title, 1.30, white)
        paragraph(note.status, 0.84, gold)
        paragraph("Desktop / Vulkano architecture / 2026-10-31", 0.74, muted)
        detail_section("ROLE", note.detail)
        detail_section("CONTRACT / CURRENT LIMIT", note.contract)
        detail_section("EXAMPLE / DISPLAYED TEXT", note.example)
        detail_section("SOURCE", note.source)
    }
}

fn detail_body() {
    return info_panel_body({
        content = detail_content()
        body_background_color = dark
        body_text_color = white
    })
}

fn show_detail(id) {
    detail_state.selected_id = id
    let body = detail_state.body_mount.query("#accordion_body")
    if body {
        body.remove_subtree()
        detail_state.body_mount.attach(detail_body())
    }
    // A minimized panel reads selected_id when its factory restores the body.
}

fn card(id, tint) {
    let note = notes[id]
    let fill = dark
    let ink = white
    let accent = tint
    let caption = muted
    if note["planned"] {
        tint = gold
        fill = gold
        ink = black
        accent = black
        caption = black
    }
    let item = T {
        name = "material_card_" + id
        Option {}
        Raycastable.enabled() { interaction_priority(25.0) }
        Style { display("block") flex_shrink(0.0) width(25.0) height(12.0) padding(0.08) background_color(tint) }
        T {
            Style {
                display("flex") flex_direction("column") width(24.84) height(11.84)
                padding(0.72) row_gap(0.22) background_color(fill)
            }
            words(note.status, 0.63, accent, 23.4, 1.55)
            words(note.title, 1.18, ink, 23.4, 2.6)
            words(note.summary, 0.94, ink, 23.4, 4.0)
            words("CLICK FOR CONTRACT + SOURCE", 0.59, caption, 23.4, 0.85)
        }
    }
    on(item, "Click", fn(event) { show_detail(id) })
    return item
}

// Wires share the panel body's glyph-unit coordinate system. Layout places only
// cards/text; these unstyled siblings draw the orthogonal paths between them.
fn ink(x, y, width, height, tint) {
    return T.position(x + width / 2.0, -y - height / 2.0, 0.06).scale(width, height, 1.0) {
        R.square() { C.rgba(tint[0], tint[1], tint[2], tint[3]) }
    }
}

fn stroke(points, tint) {
    return T {
        for i in range(len(points) - 1) {
            let a = points[i]
            let b = points[i + 1]
            if a[0] == b[0] {
                let top = a[1]
                let height = b[1] - a[1]
                if height < 0.0 { top = b[1] height = -height }
                ink(a[0] - 0.045, top, 0.09, height, tint)
            } else {
                let left = a[0]
                let width = b[0] - a[0]
                if width < 0.0 { left = b[0] width = -width }
                ink(left, a[1] - 0.045, width, 0.09, tint)
            }
        }
    }
}

fn wire(points, tint) {
    return T {
        let tip = points[len(points) - 1]
        let previous = points[len(points) - 2]
        let dx = 0.0
        let dy = 1.0
        let angle = 0.0
        if tip[0] == previous[0] {
            if tip[1] > previous[1] { dy = -1.0 angle = Math.pi }
        } else {
            dy = 0.0
            dx = 1.0
            angle = -Math.pi / 2.0
            if tip[0] < previous[0] { dx = -1.0 angle = Math.pi / 2.0 }
        }
        let head = 0.62
        let neck = [tip[0] - dx * head * 2.0 / 3.0, tip[1] + dy * head * 2.0 / 3.0]
        for i in range(len(points) - 2) { stroke([points[i], points[i + 1]], tint) }
        stroke([previous, neck], tint)
        T.position(neck[0], -neck[1], 0.06).rotation(0.0, 0.0, angle).scale(0.80, head / 0.8660254, 1.0) {
            name = "material_flow_arrowhead"
            R.triangle() { C.rgba(tint[0], tint[1], tint[2], tint[3]) }
        }
    }
}

fn row(children) {
    return T {
        Style { display("flex") flex_direction("row") column_gap(4.0) width(112.0) height(12.0) flex_shrink(0.0) }
        for child in children { child }
    }
}

fn spacer(gap_height) {
    return T { Style { display("block") width(112.0) height(gap_height) flex_shrink(0.0) } }
}

fn authoring_body() {
    return T {
        Style { display("block") width(112.0) height(19.0) }
        Unlit {}
        Selection {}
        LayoutRoot {
            available_width(112.0) available_height(19.0) unit_scale(1.0)
            T {
                Style { display("flex") flex_direction("column") width(112.0) height(19.0) }
                words("One shared appearance can be attached to several independent mesh instances.", 0.94, muted, 112.0, 3.5)
                row([card("material_base", cyan), card("material_definition", cyan), card("material_instance", violet), card("renderable", green)])
                spacer(1.1)
                words("BASE -> DEFINITION -> VALUES -> CONSUMER     Each arrow is a reference / use relationship.", 0.75, muted, 112.0, 2.4)
            }
        }
        wire([[25.0, 9.5], [29.0, 9.5]], cyan)
        wire([[54.0, 9.5], [58.0, 9.5]], violet)
        wire([[83.0, 9.5], [87.0, 9.5]], green)
    }
}

fn current_body() {
    return T {
        Style { display("block") width(112.0) height(39.0) }
        Unlit {}
        Selection {}
        LayoutRoot {
            available_width(112.0) available_height(39.0) unit_scale(1.0)
            T {
                Style { display("flex") flex_direction("column") width(112.0) height(39.0) }
                words("CUSTOM MATERIAL TODAY / typed values feed the fragment stage; the engine owns the vertex stage.", 0.94, muted, 112.0, 3.5)
                row([card("schema_values", violet), card("reflection", violet), card("descriptors", violet), card("custom_pipeline", cyan)])
                spacer(7.0)
                row([card("mesh", green), card("surface_vertex", cyan), card("fragment", violet), card("output", green)])
                spacer(1.1)
                words("Lower lane: geometry -> vertex -> rasterized surface inputs -> fragment -> attachments. The top-right card selects the shader pair.", 0.75, muted, 112.0, 3.4)
            }
        }
        wire([[25.0, 9.5], [29.0, 9.5]], violet)
        wire([[54.0, 9.5], [58.0, 9.5]], violet)
        wire([[70.5, 15.5], [70.5, 22.5]], violet)
        // Gap at the descriptor crossing: these paths are not joined.
        stroke([[99.5, 15.5], [99.5, 19.0], [71.3, 19.0]], cyan)
        wire([[69.7, 19.0], [41.5, 19.0], [41.5, 22.5]], cyan)
        wire([[25.0, 28.5], [29.0, 28.5]], green)
        wire([[54.0, 28.5], [58.0, 28.5]], cyan)
        wire([[83.0, 28.5], [87.0, 28.5]], green)
    }
}

fn deformation_body() {
    return T {
        Style { display("block") width(112.0) height(39.0) }
        Unlit {}
        Selection {}
        LayoutRoot {
            available_width(112.0) available_height(39.0) unit_scale(1.0)
            T {
                Style { display("flex") flex_direction("column") width(112.0) height(39.0) }
                words("OUTLINED: existing compute / built-in cache reader. GOLD FILLED: proposed custom pairing and touch deformation.", 0.94, muted, 112.0, 3.5)
                row([card("deformation_inputs", green), card("compute", green), card("cached_vertex", cyan), card("custom_bridge", gold)])
                spacer(7.0)
                row([card("collidable", green), card("squishable", gold), card("squish_compute", gold), T {
                    Style { display("flex") flex_direction("column") width(25.0) height(12.0) padding(0.8) }
                    words("GEOMETRY CAPABILITY", 0.76, gold, 23.4, 2.1)
                    words("Squishable would add a compute step.\n\nThe Material can keep the same fragment shader and shared values.", 0.96, white, 23.4, 8.0)
                }])
                spacer(1.1)
                words("The gold return feeds a proposed operation after skinning and before cache output. No Squishable component or custom cache bridge exists yet.", 0.75, muted, 112.0, 3.4)
            }
        }
        wire([[25.0, 9.5], [29.0, 9.5]], green)
        wire([[54.0, 9.5], [58.0, 9.5]], green)
        wire([[83.0, 9.5], [87.0, 9.5]], gold)
        wire([[25.0, 28.5], [29.0, 28.5]], gold)
        wire([[54.0, 28.5], [58.0, 28.5]], gold)
        wire([[70.5, 22.5], [70.5, 19.0], [41.5, 19.0], [41.5, 15.5]], gold)
    }
}

fn panel_title(id, label) {
    return T {
        name = id + "_title_bar"
        Grabbable.parent()
        Draggable.target("../../#" + id)
        Raycastable.enabled()
        Style {
            display("flex") flex_direction("row") align_items("center")
            width(100%) height(3.5) background_color([0.075, 0.16, 0.22, 1.0])
        }
        T {
            Style { display("flex") width(0.0) flex_grow(1.0) height(3.5) align_items("center") padding_xy(0.9, 0.3) font_size(1.10) }
            Text { label C.rgba(white[0], white[1], white[2], 1.0) }
        }
    }
}

fn diagram_panel(id, label, height_position, factory) {
    return T.position(-8.4, height_position, 0.0).scale(0.095, 0.095, 0.095) {
        info_panel({
            root_name = id
            width_gu = 112.0
            unit_scale = 1.0
            title = panel_title(id, label)
            toggle_icon_glow_intensity = 0.0
            toggle_background_color = [0.075, 0.16, 0.22, 1.0]
            toggle_icon_color = white
            body_background_color = [0.012, 0.021, 0.036, 1.0]
            transparent_background = true
            body_text_color = white
            inherit_body_text_color = false
            body_padding = 0.0
            content_factory = factory
        })
    }
}

diagram_panel("material_authoring_diagram", "01 / MMS AUTHORING - bases, definitions, values, renderables", 6.5, fn() { return authoring_body() })
diagram_panel("material_current_diagram", "02 / CURRENT CUSTOM PATH - descriptors and shader stages", 3.9, fn() { return current_body() })
diagram_panel("material_deformation_diagram", "03 / DEFORMATION - existing foundation + proposed squishing", -0.55, fn() { return deformation_body() })

let details_panel = info_panel({
    root_name = "material_shader_details"
    width_gu = 42.0
    unit_scale = 0.095
    title = panel_title("material_shader_details", "Material / shader details")
    toggle_icon_glow_intensity = 0.0
    toggle_background_color = [0.075, 0.16, 0.22, 1.0]
    toggle_icon_color = white
    body_background_color = dark
    body_text_color = white
    content_factory = fn() { return detail_content() }
})
T.position(3.0, 6.5, 0.8) { details_panel }
detail_state.body_mount = details_panel.query("#accordion_body_mount")

// Desktop only: this explanatory scene needs neither a headset nor model assets.
Input.speed(3.0) {
    InputTransformMode.forward_z() { fps_rotation() roll_axis_y() }
    T.position(-0.7, 0.8, 12.0) { C3D { Pointer {} } }
}
