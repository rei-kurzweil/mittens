// Body solver -> upper and lower body solvers: who decides, who writes, each frame.
// Run: cargo run --release -- load examples/body-solver-flowchart.mms
// Desktop only. WASD + right-drag to fly; click a card for its contract and source.
// Outlined cards exist today. Gold-filled cards are proposed next steps.

import { info_panel, info_panel_body } from "../assets/components/ui/info_panel.mms"
import { body_solver_notes } from "../assets/components/diagrams/body_solver_notes.mms"

RendererSettings { window_size(1920, 1200) }
BGC.rgba(0.020, 0.018, 0.032, 1.0)
RenderGraph.off()

// Pastel set: lavender decides, mint writes, sky shares, peach observes.
let lavender = [0.80, 0.70, 1.0, 1.0]
let mint = [0.62, 0.95, 0.80, 1.0]
let sky = [0.62, 0.84, 1.0, 1.0]
let peach = [1.0, 0.78, 0.66, 1.0]
let rose = [1.0, 0.70, 0.82, 1.0]
let gold = [1.0, 0.82, 0.48, 1.0]
let white = [0.95, 0.94, 1.0, 1.0]
let muted = [0.72, 0.70, 0.82, 1.0]
let dark = [0.040, 0.034, 0.062, 1.0]
let black = [0.03, 0.025, 0.04, 1.0]
let notes = body_solver_notes()
let detail_state = { selected_id = "body_solver" body_mount = null }

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
        paragraph(label, 0.76, lavender)
        paragraph(value, 0.96, white)
    }
}

fn detail_content() {
    let note = notes[detail_state.selected_id]
    return T {
        name = "body_solver_detail_content"
        Style { display("flex") flex_direction("column") width(100%) row_gap(0.35) padding(0.4) }
        paragraph(note.title, 1.30, white)
        paragraph(note.status, 0.84, peach)
        paragraph("Avatar IK / body solver / 2026-11-16", 0.74, muted)
        detail_section("ROLE", note.detail)
        detail_section("CONTRACT", note.contract)
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
}

fn card(id, tint) {
    let note = notes[id]
    let fill = dark
    let ink_tint = white
    let accent = tint
    let caption = muted
    if note["planned"] {
        tint = gold
        fill = gold
        ink_tint = black
        accent = black
        caption = black
    }
    let item = T {
        name = "body_card_" + id
        Option {}
        Raycastable.enabled() { interaction_priority(25.0) }
        Style { display("block") flex_shrink(0.0) width(25.0) height(12.0) padding(0.08) background_color(tint) }
        T {
            Style {
                display("flex") flex_direction("column") width(24.84) height(11.84)
                padding(0.72) row_gap(0.22) background_color(fill)
            }
            words(note.status, 0.63, accent, 23.4, 1.55)
            words(note.title, 1.18, ink_tint, 23.4, 2.6)
            words(note.summary, 0.94, ink_tint, 23.4, 4.0)
            words("CLICK FOR CONTRACT + SOURCE", 0.59, caption, 23.4, 0.85)
        }
    }
    on(item, "Click", fn(event) { show_detail(id) })
    return item
}

// Wires share the panel body's glyph-unit coordinates: x right, y down.
// Cards sit at x = 0, 29, 58, 87 (25 wide); rows center on y = 9.5 and 28.5.
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
            name = "body_flow_arrowhead"
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

fn frame_body() {
    return T {
        Style { display("block") width(112.0) height(39.0) }
        Unlit {}
        Selection {}
        LayoutRoot {
            available_width(112.0) available_height(39.0) unit_scale(1.0)
            T {
                Style { display("flex") flex_direction("column") width(112.0) height(39.0) }
                words("Top lane: decide, then write the torso, then place the body under the head. Bottom lane: legs, feedback, and the outward event.", 0.94, muted, 112.0, 3.5)
                row([card("avc_settings", sky), card("body_solver", lavender), card("upper_body", mint), card("head_neck", sky)])
                spacer(7.0)
                row([card("feedback", rose), card("lower_body", mint), card("policies", mint), card("posture_event", peach)])
                spacer(1.1)
                words("Lavender decides / mint writes bones / sky reads or configures / rose is last frame's state / peach is observable from MMS.", 0.75, muted, 112.0, 3.4)
            }
        }
        wire([[25.0, 9.5], [29.0, 9.5]], sky)
        wire([[54.0, 9.5], [58.0, 9.5]], lavender)
        wire([[83.0, 9.5], [87.0, 9.5]], mint)
        // Mode, weight, and floor down to the lower body.
        wire([[41.5, 15.5], [41.5, 22.5]], lavender)
        wire([[54.0, 28.5], [58.0, 28.5]], mint)
        // Last frame's support heights and Limited status back up.
        wire([[29.0, 28.5], [25.0, 28.5]], rose)
        wire([[12.5, 22.5], [12.5, 19.0], [33.0, 19.0], [33.0, 15.5]], rose)
        // Switches go out as PostureChanged.
        wire([[50.0, 15.5], [50.0, 19.5], [99.5, 19.5], [99.5, 22.5]], peach)
    }
}

fn lower_body_body() {
    return T {
        Style { display("block") width(112.0) height(39.0) }
        Unlit {}
        Selection {}
        LayoutRoot {
            available_width(112.0) available_height(39.0) unit_scale(1.0)
            T {
                Style { display("flex") flex_direction("column") width(112.0) height(39.0) }
                words("One system picks exactly one leg policy per avatar. Policies return goals; they never see the body mode.", 0.94, muted, 112.0, 3.5)
                row([card("lower_system", mint), card("support", sky), card("leg", sky), card("next_steps", gold)])
                spacer(7.0)
                row([card("crouch", lavender), card("walking", lavender), card("airborne", lavender), card("kneel", lavender)])
                spacer(1.1)
                words("Vertical: crouch rest goals, walking gait, airborne. Horizontal (weight >= 0.5): kneel. support.rs and leg.rs serve every policy.", 0.75, muted, 112.0, 3.4)
            }
        }
        wire([[29.0, 9.5], [25.0, 9.5]], sky)
        stroke([[12.5, 15.5], [12.5, 19.0], [99.5, 19.0]], mint)
        wire([[12.5, 19.0], [12.5, 22.5]], mint)
        wire([[41.5, 19.0], [41.5, 22.5]], mint)
        wire([[70.5, 19.0], [70.5, 22.5]], mint)
        wire([[99.5, 19.0], [99.5, 22.5]], mint)
    }
}

fn body_solver_inside() {
    return T {
        Style { display("block") width(112.0) height(19.0) }
        Unlit {}
        Selection {}
        LayoutRoot {
            available_width(112.0) available_height(19.0) unit_scale(1.0)
            T {
                Style { display("flex") flex_direction("column") width(112.0) height(19.0) }
                words("Inside the body solver, in order each frame.", 0.94, muted, 112.0, 3.5)
                row([card("mode_selection", lavender), card("blend", lavender), card("pitch_solve", lavender), card("yaw", lavender)])
                spacer(1.1)
                words("Mode -> eased weight -> torso pitch for the upper body -> yaw deadband on the body operator.", 0.75, muted, 112.0, 2.4)
            }
        }
        wire([[25.0, 9.5], [29.0, 9.5]], lavender)
        wire([[54.0, 9.5], [58.0, 9.5]], lavender)
        wire([[83.0, 9.5], [87.0, 9.5]], lavender)
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
            width(100%) height(3.5) background_color([0.13, 0.10, 0.20, 1.0])
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
            toggle_background_color = [0.13, 0.10, 0.20, 1.0]
            toggle_icon_color = white
            body_background_color = [0.028, 0.024, 0.045, 1.0]
            transparent_background = true
            body_text_color = white
            inherit_body_text_color = false
            body_padding = 0.0
            content_factory = factory
        })
    }
}

diagram_panel("body_frame_diagram", "01 / EACH FRAME - who decides, who writes", 6.5, fn() { return frame_body() })
diagram_panel("body_lower_diagram", "02 / LOWER BODY - one policy per avatar", 2.0, fn() { return lower_body_body() })
diagram_panel("body_inside_diagram", "03 / INSIDE THE BODY SOLVER", -2.5, fn() { return body_solver_inside() })

let details_panel = info_panel({
    root_name = "body_solver_details"
    width_gu = 42.0
    unit_scale = 0.095
    title = panel_title("body_solver_details", "Body solver details")
    toggle_icon_glow_intensity = 0.0
    toggle_background_color = [0.13, 0.10, 0.20, 1.0]
    toggle_icon_color = white
    body_background_color = dark
    body_text_color = white
    content_factory = fn() { return detail_content() }
})
T.position(3.0, 6.5, 0.8) { details_panel }
detail_state.body_mount = details_panel.query("#accordion_body_mount")

// Desktop only: no headset or model assets needed.
Input.speed(3.0) {
    InputTransformMode.forward_z() { fps_rotation() roll_axis_y() }
    T.position(-0.7, 1.4, 14.0) { C3D { Pointer {} } }
}
