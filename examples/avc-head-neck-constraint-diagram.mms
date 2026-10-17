// Interactive proposal, not an implementation of the avatar constraint.
// Run: cargo run --release -- load examples/avc-head-neck-constraint-diagram.mms
// All operator names except TOP.yaw_follow describe proposed capabilities.
// Avatar snippets are displayed text. Only this diagram and its desktop camera run.
// Styled rows place cards; unstyled geometry draws orthogonal dataflow edges.

import { info_panel, info_panel_body } from "../assets/components/ui/info_panel.mms"
import { avc_head_neck_notes } from "../assets/components/diagrams/avc_head_neck_notes.mms"

RendererSettings { window_size(1920, 1200) }
BGC.rgba(0.009, 0.014, 0.025, 1.0)
AL.rgb(1.0, 1.0, 1.0)
RenderGraph {
    EmissivePass { BlurPass { radius_ndc(0.008) half_res(true) } }
    Bloom { intensity(0.30) radius_ndc(0.008) emissive_scale(1.0) half_res(true) }
}

let cyan = [0.24, 0.82, 0.95, 1.0]
let gold = [1.0, 0.72, 0.30, 1.0]
let violet = [0.72, 0.54, 1.0, 1.0]
let green = [0.40, 0.90, 0.68, 1.0]
let muted = [0.60, 0.68, 0.78, 1.0]
let white = [0.91, 0.95, 1.0, 1.0]
let black = [0.0, 0.0, 0.0, 1.0]
let notes = avc_head_neck_notes()
let detail_state = { selected_id = null body_mount = null }

fn detail_text(value, size, tint) {
    return T {
        Style { display("block") width(100%) font_size(size) word_wrap("break-word") }
        Text { value C.rgba(tint[0], tint[1], tint[2], tint[3]) }
    }
}

fn detail_section(label, value) {
    return T {
        Style { display("flex") flex_direction("column") width(100%) row_gap(0.25) margin_top(0.5) }
        detail_text(label, 0.62, cyan)
        detail_text(value, 0.78, white)
    }
}

fn detail_content(id) {
    let note = null
    if id != null { note = notes[id] }
    return T {
        name = "avc_diagram_detail_content"
        Style { display("flex") flex_direction("column") width(100%) row_gap(0.3) padding(0.4) }
        if note {
            detail_text(note.title, 1.02, white)
            detail_text(note.kind, 0.70, gold)
            detail_text(note.role, 0.68, muted)
            detail_section("IN THIS PROPOSAL", note.detail)
            detail_section("OWNERSHIP / EVALUATION", note.contract)
            detail_section("OPEN DESIGN / VALIDATION", note.open)
            if note.example { detail_section("FUTURE MMS OUTLINE / DISPLAYED TEXT", note.example) }
            detail_section("DESIGN DOC", note.reference)
        } else {
            detail_text("Select any proposal card", 1.05, white)
            detail_text("All 15 cards are clickable. Their short input / output / how / why stays on the diagram; this panel explains the contracts and open decisions.", 0.84, white)
            detail_section("01 / OPERATOR LIBRARY", "One existing yaw-follow facility, a proposed typed math/patch facility, and a proposed bounded translation follower. Math primitives are value operations, not separate ECS components.")
            detail_section("02 / CONSTRAINT COMPOSITION", "Read the main pose lane from left to right. Rig calibration and yaw feed it from above. The accepted anchor returns below the lane through a violet next-update loop.")
            detail_section("03 / FUTURE MMS EXAMPLE", "Ordinary Input or InputXR with AVC will request the default behavior. These cards outline the later example; this scene does not spawn an avatar or run a solver.")
            detail_section("CARD APPEARANCE", "Solid bright backgrounds with black text identify proposed behavior or future examples. Outlined cards identify existing input/facility context. All cards remain clickable.")
            detail_section("STATUS / CONTROLS", "Visual proposal / 2026-10-16. Exact new TOP syntax and pose-limit projection remain open. WASD / R / F: navigate; right mouse: look. Drag the panel title; collapse and restore it while retaining your selection.")
        }
    }
}

fn detail_body() {
    return info_panel_body({
        content = detail_content(detail_state.selected_id)
        body_background_color = [0.018, 0.026, 0.042, 1.0]
        body_text_color = white
    })
}

fn show_detail(id) {
    detail_state.selected_id = id
    let current_body = detail_state.body_mount.query("#accordion_body")
    if current_body {
        current_body.remove_subtree()
        detail_state.body_mount.attach(detail_body())
    }
}

fn words(value, size, color, box_w, box_h) {
    return T {
        Style { display("block") width(box_w) height(box_h) font_size(size) word_wrap("break-all") }
        Text { value C.rgba(color[0], color[1], color[2], color[3]) }
    }
}

fn port(label, value, tint, fill, value_ink, box_w) {
    return T {
        Style {
            display("flex") flex_direction("column")
            width(box_w) height(2.35) padding_xy(0.35, 0.18)
            background_color(fill)
        }
        words(label, 0.44, tint, box_w - 0.70, 0.55)
        words(value, 0.63, value_ink, box_w - 0.70, 1.40)
    }
}

fn card(id, tint, box_w) {
    let note = notes[id]
    let card_fill = [0.018, 0.030, 0.049, 1.0]
    let card_ink = white
    let kind_ink = tint
    let caption_ink = muted
    let input_ink = cyan
    let output_ink = green
    let input_fill = [0.025, 0.063, 0.080, 1.0]
    let output_fill = [0.024, 0.070, 0.056, 1.0]
    if note.proposed {
        card_fill = tint
        card_ink = black
        kind_ink = black
        caption_ink = black
        input_ink = black
        output_ink = black
        input_fill = tint
        output_fill = tint
    }
    let item = T {
        name = id
        Option {}
        Raycastable.enabled() { interaction_priority(25.0) }
        Style {
            display("block") flex_shrink(0.0) width(box_w) height(15.5)
            padding(0.06) background_color(tint)
        }
        T {
            Style {
                display("flex") flex_direction("column")
                width(box_w - 0.12) height(15.38) padding(0.60)
                background_color(card_fill)
            }
            words(note.kind, 0.50, kind_ink, box_w - 1.32, 0.8)
            words(note.title, 0.96, card_ink, box_w - 1.32, 1.9)
            port("INPUT", note.input, input_ink, input_fill, card_ink, box_w - 1.32)
            T { Style { display("block") width(box_w - 1.32) height(0.20) } }
            port("OUTPUT", note.output, output_ink, output_fill, card_ink, box_w - 1.32)
            T { Style { display("block") width(box_w - 1.32) height(0.25) } }
            words("HOW IT WORKS", 0.44, kind_ink, box_w - 1.32, 0.55)
            words(note.how, 0.65, card_ink, box_w - 1.32, 1.65)
            words("WHY USEFUL", 0.44, caption_ink, box_w - 1.32, 0.55)
            words(note.why, 0.65, caption_ink, box_w - 1.32, 1.65)
            words("CLICK FOR CONTRACTS / OPEN DESIGN", 0.43, kind_ink, box_w - 1.32, 0.60)
        }
    }
    on(item, "Click", fn(event) { show_detail(id) })
    return item
}

// Content starts at (1.5, 5.0) in each section's glyph-unit coordinate frame.
fn section(id, title, description, content, section_w, section_h, tint) {
    return T {
        name = id
        Style {
            display("block") width(section_w) height(section_h) flex_shrink(0.0)
            padding(0.06) background_color([tint[0] * 0.28, tint[1] * 0.28, tint[2] * 0.28, 1.0])
        }
        T {
            Style {
                display("flex") flex_direction("column")
                width(section_w - 0.12) height(section_h - 0.12) padding(1.44)
                background_color([0.012, 0.021, 0.036, 1.0])
            }
            words(title, 1.02, tint, section_w - 3.0, 1.5)
            words(description, 0.65, muted, section_w - 3.0, 1.4)
            T { Style { display("block") width(section_w - 3.0) height(0.6) } }
            content
        }
    }
}

fn ink(x, y, line_w, line_h, color) {
    return T.position(x + line_w / 2.0, -y - line_h / 2.0, 0.03).scale(line_w, line_h, 1.0) {
        R.square() {
            C.rgba(color[0], color[1], color[2], color[3])
            Emissive.on() { intensity(1.1) }
        }
    }
}

fn stroke(points, color) {
    return T {
        for i in range(len(points) - 1) {
            let a = points[i]
            let b = points[i + 1]
            if a[0] == b[0] {
                let top = a[1]
                let height = b[1] - a[1]
                if height < 0.0 { top = b[1] height = -height }
                ink(a[0] - 0.04, top, 0.08, height, color)
            } else {
                let left = a[0]
                let width = b[0] - a[0]
                if width < 0.0 { left = b[0] width = -width }
                ink(left, a[1] - 0.04, width, 0.08, color)
            }
        }
    }
}

fn wire(points, color) {
    return T {
        let tip = points[len(points) - 1]
        let prev = points[len(points) - 2]
        let direction_x = 0.0
        let direction_y = 1.0
        let angle = 0.0
        if tip[0] == prev[0] {
            if tip[1] > prev[1] { direction_y = -1.0 angle = 3.14159265 }
        } else {
            direction_y = 0.0
            direction_x = 1.0
            angle = -1.57079633
            if tip[0] < prev[0] { direction_x = -1.0 angle = 1.57079633 }
        }
        let head_length = 0.60
        let tip_offset = head_length * 2.0 / 3.0
        let neck = [tip[0] - direction_x * tip_offset, tip[1] + direction_y * tip_offset]
        for i in range(len(points) - 2) { stroke([points[i], points[i + 1]], color) }
        stroke([prev, neck], color)
        T.position(neck[0], -neck[1], 0.03).rotation(0.0, 0.0, angle).scale(0.76, head_length / 0.8660254, 1.0) {
            name = "avc_diagram_arrowhead"
            R.triangle() {
                C.rgba(color[0], color[1], color[2], color[3])
                Emissive.on() { intensity(1.1) }
            }
        }
    }
}

// Section tops: library 6, composition 34, future example 107.5.
// Composition main lane: x=1.5 + 45*n, y=26.5; six cards 38x15.5.
// Rig and yaw sit above; accepted state sits below publication with a return wire.
T.position(-13.3, 6.70, 0.0).scale(0.10, 0.10, 0.10) {
    LayoutRoot {
        name = "avc_constraint_proposal"
        available_width(266.0) available_height(133.5) unit_scale(1.0)
        Selection { name = "avc_diagram_selection" }
        T {
            Style { display("flex") flex_direction("column") width(266.0) height(133.5) }
            words("AVC / HEAD, NECK AND BODY FOLLOW", 1.55, white, 266.0, 2.3)
            words("INTERACTIVE PROPOSAL / 2026-10-16     Click any card for contracts, geometry and unresolved API decisions.", 0.75, muted, 266.0, 1.7)
            words("SOLID / BLACK TEXT: proposed behavior or future example.    OUTLINED: existing facility / input context.\nCYAN: pure math / pose flow    VIOLET: retained state    GOLD: calibration / authored shell    GREEN: accepted pose", 0.68, muted, 266.0, 2.0)
            section("avc_operator_library", "01 / OPERATOR FACILITIES", "Three facilities, not a component for every arithmetic step. These cards are independent library entries; section 02 shows their composition.",
                T {
                    Style { display("flex") flex_direction("row") column_gap(5.0) width(169.0) height(15.5) }
                    card("op_yaw", cyan, 53.0)
                    card("op_math", cyan, 53.0)
                    card("op_follow", violet, 53.0)
                }, 172.0, 23.0, cyan)
            T { Style { display("block") width(266.0) height(5.0) } }
            section("avc_constraint_composition", "02 / AVC-GENERATED CONSTRAINT", "Read the pose flow left to right. Rig calibration and yaw feed it from above; the accepted anchor returns underneath on the next update.",
                T {
                    Style { display("flex") flex_direction("column") width(263.0) height(62.0) }
                    T {
                        Style { display("flex") flex_direction("column") row_gap(6.0) width(263.0) height(58.5) }
                        T {
                            Style { display("flex") flex_direction("row") column_gap(7.0) width(263.0) height(15.5) }
                            T {
                                Style { display("flex") flex_direction("column") width(38.0) height(15.5) flex_shrink(0.0) }
                                words("SUPPORTING VALUES", 0.70, muted, 38.0, 1.5)
                                words("Rig calibration and current body yaw\nfeed the main pose lane below.\n\nShared context travels with its result;\nno duplicate filters are implied.", 0.65, muted, 38.0, 5.0)
                            }
                            card("rig_bind", gold, 38.0)
                            card("used_yaw", cyan, 38.0)
                        }
                        T {
                            Style { display("flex") flex_direction("row") column_gap(7.0) width(263.0) height(15.5) }
                            card("pose_inputs", muted, 38.0)
                            card("used_math_rotation", cyan, 38.0)
                            card("used_math_measure", cyan, 38.0)
                            card("used_follow", violet, 38.0)
                            card("used_math_closure", green, 38.0)
                            card("used_math_publish", green, 38.0)
                        }
                        T {
                            Style { display("flex") flex_direction("row") width(263.0) height(15.5) }
                            T { Style { display("block") width(225.0) height(15.5) flex_shrink(0.0) } }
                            card("accepted_anchor", violet, 38.0)
                        }
                    }
                    T { Style { display("block") width(263.0) height(0.7) } }
                    words("Shared range: rotation-only movement fits; positional excess can trigger follow. Reach limits still apply. Commit state only after a connected pose is accepted.", 0.66, muted, 263.0, 2.0)
                }, 266.0, 68.5, green)
            T { Style { display("block") width(266.0) height(5.0) } }
            section("avc_future_example", "03 / FUTURE MMS EXAMPLE", "Author ordinary Input or InputXR with AVC. The future default wires section 02 automatically; these displayed snippets are not running avatars.",
                T {
                    Style { display("flex") flex_direction("row") column_gap(5.0) width(169.0) height(15.5) }
                    card("scene_desktop", gold, 53.0)
                    card("scene_xr", gold, 53.0)
                    card("scene_checks", gold, 53.0)
                }, 172.0, 24.0, gold)
            words("This scene runs only the diagram. No new TOP constructors, avatar solver, model or XR session are executed.   WASD / R / F: navigate; right mouse: look.", 0.66, muted, 266.0, 2.0)
        }
    }
    T.position(0.0, -34.0, 0.08) {
        // Source orientation branches upward to yaw. A small crossing gap
        // distinguishes it from the vertical gold calibration connection.
        stroke([[20.5, 26.5], [20.5, 23.0], [64.7, 23.0]], cyan)
        wire([[66.3, 23.0], [110.5, 23.0], [110.5, 20.5]], cyan)
        wire([[65.5, 20.5], [65.5, 26.5]], gold)
        wire([[121.5, 20.5], [121.5, 24.0], [78.5, 24.0], [78.5, 26.5]], cyan)
        // The entire current-update pose flow is one left-to-right lane.
        wire([[39.5, 34.25], [46.5, 34.25]], cyan)
        wire([[84.5, 34.25], [91.5, 34.25]], cyan)
        wire([[129.5, 34.25], [136.5, 34.25]], violet)
        wire([[174.5, 34.25], [181.5, 34.25]], green)
        wire([[219.5, 34.25], [226.5, 34.25]], green)
        // Commit below publication, then return only on the next update.
        wire([[245.5, 42.0], [245.5, 48.0]], green)
        wire([[226.5, 55.75], [155.5, 55.75], [155.5, 42.0]], violet)
        T.position(176.0, -52.5, 0.03) { words("ACCEPTED ANCHOR / NEXT UPDATE", 0.62, violet, 48.0, 1.0) }
    }
}

let details_panel = info_panel({
    root_name = "avc_diagram_details"
    width_gu = 52.0
    unit_scale = 0.075
    title = T {
        name = "avc_diagram_details_title_bar"
        Grabbable.parent()
        Draggable.target("../../#avc_diagram_details")
        Raycastable.enabled()
        Style {
            display("flex") flex_direction("row") align_items("center")
            width(100%) height(3.5)
            background_color([0.075, 0.16, 0.22, 1.0])
        }
        T {
            Style {
                display("flex") width(0.0) flex_grow(1.0) height(3.5)
                align_items("center") padding_xy(0.9, 0.3) font_size(0.9)
            }
            Text { "AVC constraint / proposal notes" C.rgba(0.91, 0.95, 1.0, 1.0) }
        }
    }
    toggle_icon_glow_intensity = 0.25
    body_background_color = [0.018, 0.026, 0.042, 1.0]
    body_text_color = white
    content = detail_content(null)
})
T.position(5.8, 6.15, 1.0) {
    name = "avc_diagram_details_placement"
    details_panel
}
detail_state.body_mount = details_panel.query("#accordion_body_mount")
on(details_panel, "DataEvent", fn(event) {
    if event == "AccordionRestoreRequested" {
        detail_state.body_mount.attach(detail_body())
    }
})

// Desktop camera only. Use W to approach individual sections for comfortable text.
Input.speed(3.0) {
    InputTransformMode.forward_z() { fps_rotation() roll_axis_y() }
    T.position(0.0, 0.0, 15.5) { C3D { Pointer {} } }
}
