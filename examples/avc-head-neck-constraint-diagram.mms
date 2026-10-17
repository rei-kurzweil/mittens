// Interactive proposal, not an implementation of the avatar constraint.
// Run: cargo run --release -- load examples/avc-head-neck-constraint-diagram.mms
// New value-component names are proposals. TOP.yaw_follow is existing context.
// Avatar snippets are displayed text. Only this diagram and its desktop camera run.
// Styled rows place cards; unstyled geometry draws orthogonal dataflow edges.

import { info_panel, info_panel_body } from "../assets/components/ui/info_panel.mms"
import { avc_head_neck_notes, avc_math_component_colors } from "../assets/components/diagrams/avc_head_neck_notes.mms"

RendererSettings { window_size(1920, 1200) }
BGC.rgba(0.009, 0.014, 0.025, 1.0)
AL.rgb(1.0, 1.0, 1.0)
// No blur/bloom passes: this text-heavy diagnostic scene needs no glow postprocess.

let cyan = [0.24, 0.82, 0.95, 1.0]
let gold = [1.0, 0.72, 0.30, 1.0]
let violet = [0.72, 0.54, 1.0, 1.0]
let green = [0.40, 0.90, 0.68, 1.0]
let muted = [0.60, 0.68, 0.78, 1.0]
let white = [0.91, 0.95, 1.0, 1.0]
let black = [0.0, 0.0, 0.0, 1.0]
let graph_text_scale = 2.6
let notes = avc_head_neck_notes()
let component_colors = avc_math_component_colors()
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
            if note["operators"] { component_badges(id, 49.9) }
            detail_section("IN THIS PROPOSAL", note.detail)
            detail_section("OWNERSHIP / EVALUATION", note.contract)
            detail_section("OPEN DESIGN / VALIDATION", note.open)
            if note.example { detail_section("FUTURE MMS OUTLINE / DISPLAYED TEXT", note.example) }
            detail_section("DESIGN DOC", note.reference)
        } else {
            detail_text("Select any proposal card", 1.05, white)
            detail_text("Each card is clickable. Input / output / how / why stays on the diagram; this panel explains the typed contracts, native execution model and open decisions.", 0.84, white)
            detail_section("01 / OPERATOR LIBRARY", "Proposed long names identify typed value components for scalar, Vec3 and quaternion operations. MathGraph binds them into a native execution plan. MMS constructs descriptions; the engine evaluates the values without per-update MMS callbacks.")
            detail_section("02 / CONSTRAINT COMPOSITION", "Read the main pose lane from left to right. Its cards name the value components they compose in matching catalog colors. Calibration/yaw feed it from above; accepted follow state returns underneath on the next update.")
            detail_section("03 / FUTURE MMS EXAMPLE", "Ordinary Input or InputXR with AVC will request the default behavior. These cards outline the later example; this scene does not spawn an avatar or run a solver.")
            detail_section("CARD APPEARANCE", "Solid bright backgrounds with black text identify proposed behavior or future examples. Outlined cards identify existing input/facility context. All cards remain clickable.")
            detail_section("STATUS / CONTROLS", "Visual proposal / 2026-10-17. Full component names are proposals; shortforms and connection syntax remain open. Drag a section title or its short left bar; use the arrow to unload / restore its content. Drag this panel by its title. WASD / R / F: navigate; right mouse: look. Collapsing retains the selected card.")
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

fn words(value, size, ink, box_w, box_h) {
    return T {
        Style { display("block") width(box_w) height(box_h) font_size(size) word_wrap("break-all") color(ink) }
        Text { value C.rgba(ink[0], ink[1], ink[2], ink[3]) }
    }
}

// Lower cards flow at word boundaries; omitting height lets layout measure
// their text, ports and containers. The compact catalog keeps its current sizing.
fn card_words(value, size, ink, box_w, box_h, auto_height) {
    let wrap_mode = "break-all"
    if auto_height { wrap_mode = "normal" }
    return T {
        Style {
            display("block") width(box_w) font_size(size)
            word_wrap(wrap_mode) color(ink)
            // Preserve the words in the long compound operator name when the
            // enlarged tag needs another line at the same width.
            if auto_height && value == "QuaternionFromDirections" {
                word_wrap_tokens(["From"])
            }
            if auto_height { margin_bottom(0.20) }
            else { height(box_h) }
        }
        Text { value C.rgba(ink[0], ink[1], ink[2], ink[3]) }
    }
}

fn flow_words(value, size, ink, box_w) {
    return card_words(value, size, ink, box_w, 0.0, true)
}

fn port(label, value, tint, fill, value_ink, box_w, text_scale, auto_height) {
    return T {
        Style {
            display("flex") flex_direction("column") width(box_w)
            padding_xy(0.35, 0.18) background_color(fill)
            if !auto_height { height(2.35 * text_scale) }
        }
        card_words(label, 0.44 * text_scale, tint, box_w - 0.70, 0.55 * text_scale, auto_height)
        card_words(value, 0.63 * text_scale, value_ink, box_w - 0.70, 1.40 * text_scale, auto_height)
    }
}

fn component_color(id, fallback) {
    let key = id
    if notes[id]["operators"] { key = notes[id].operators[0] }
    if component_colors[key] { return component_colors[key] }
    return fallback
}

// Composite stages contain several operations. Name each with exactly the same
// fill as its catalog card; the large stage uses its primary component's color.
fn component_badges(stage_id, box_w) {
    return scaled_component_badges(stage_id, box_w, 1.0)
}

fn scaled_component_badges(stage_id, box_w, text_scale) {
    let auto_height = text_scale > 1.0
    return T {
        name = stage_id + "_components"
        Style {
            display("flex") flex_direction("column") width(box_w)
            if !auto_height { row_gap(0.15 * text_scale) }
        }
        card_words("Uses", 0.43 * text_scale, muted, box_w, 0.8 * text_scale, auto_height)
        for operator_id in notes[stage_id].operators {
            let tint = component_colors[operator_id]
            let badge = T {
                name = "badge_" + stage_id + "_" + operator_id
                Option {}
                Raycastable.enabled() { interaction_priority(25.0) }
                Style {
                    display("flex") width(box_w) flex_shrink(0.0)
                    if auto_height { margin_bottom(0.15 * text_scale) }
                    else { height(1.2 * text_scale) }
                    align_items("center") padding_xy(0.12, 0.4) background_color(tint) color(black)
                }
                card_words(notes[operator_id].title, 0.62 * text_scale, black, box_w - 0.8, 0.9 * text_scale, auto_height)
            }
            on(badge, "Click", fn(event) { show_detail(operator_id) })
            badge
        }
    }
}

fn card(id, fallback_tint, box_w) {
    return sized_card(id, fallback_tint, box_w, 1.0, 15.5)
}

fn graph_card(id, fallback_tint, box_w) {
    return sized_card(id, fallback_tint, box_w, graph_text_scale, null)
}

fn example_card(id, fallback_tint, box_w) {
    return sized_card(id, fallback_tint, box_w, 1.0, null)
}

// A null height selects intrinsic content sizing and word-boundary wrapping.
// Width and font size stay explicit; the compact catalog still uses fixed boxes.
fn sized_card(id, fallback_tint, box_w, text_scale, card_h) {
    let auto_height = card_h == null
    let note = notes[id]
    let tint = component_color(id, fallback_tint)
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
            display("block") flex_shrink(0.0) width(box_w)
            if !auto_height { height(card_h) }
            padding(0.06) background_color(tint) color(card_ink)
        }
        T {
            Style {
                display("flex") flex_direction("column")
                width(box_w - 0.12) padding(0.60)
                // Use measured child margins for spacing in auto-height cards.
                if !auto_height { height(card_h - 0.12) }
                background_color(card_fill) color(card_ink)
            }
            card_words(note.kind, 0.50 * text_scale, kind_ink, box_w - 1.32, 0.8 + 1.4 * (text_scale - 1.0), auto_height)
            card_words(note.title, 0.96 * text_scale, card_ink, box_w - 1.32, 1.9 + 2.2 * (text_scale - 1.0), auto_height)
            port("INPUT", note.input, input_ink, input_fill, card_ink, box_w - 1.32, text_scale, auto_height)
            T { Style { display("block") width(box_w - 1.32) height(0.20) } }
            port("OUTPUT", note.output, output_ink, output_fill, card_ink, box_w - 1.32, text_scale, auto_height)
            T { Style { display("block") width(box_w - 1.32) height(0.25) } }
            card_words("HOW IT WORKS", 0.44 * text_scale, kind_ink, box_w - 1.32, 0.55 * text_scale, auto_height)
            card_words(note.how, 0.65 * text_scale, card_ink, box_w - 1.32, 1.65 + 3.85 * (text_scale - 1.0), auto_height)
            card_words("WHY USEFUL", 0.44 * text_scale, caption_ink, box_w - 1.32, 0.55 * text_scale, auto_height)
            card_words(note.why, 0.65 * text_scale, caption_ink, box_w - 1.32, 1.65 + 3.85 * (text_scale - 1.0), auto_height)
            card_words("CLICK FOR CONTRACTS / OPEN DESIGN", 0.43 * text_scale, kind_ink, box_w - 1.32, 0.60 * text_scale, auto_height)
        }
    }
    on(item, "Click", fn(event) { show_detail(id) })
    return item
}

// Retain a draggable header and minimize control; construct the entire expensive
// body (including wires) afresh. No detached live component tree is cached.
fn section(id, title, description, content_factory, section_w, section_h, tint, group_x, group_y, decorations_factory) {
    let text_scale = 1.0
    if id == "avc_constraint_composition" { text_scale = graph_text_scale }
    let panel = info_panel({
        root_name = id
        width_gu = section_w
        unit_scale = 1.0
        transparent_background = true
        // The nested card layout supplies its own colors. A shell-level color
        // can register after that layout and overwrite its rendered glyphs.
        inherit_body_text_color = false
        body_padding = 0.0
        toggle_icon_glow_intensity = 0.0
        title = T {
            name = id + "_handle"
            Draggable.target("../../#" + id).plane("camera")
            Raycastable.enabled() { interaction_priority(35.0) }
            Style {
                display("flex") flex_direction("row") align_items("center")
                width(100%) height(3.5) background_color([0.018, 0.030, 0.049, 1.0])
            }
            // A layout boundary keeps the decorative handle at its authored
            // left position instead of centering its visual bounds in the bar.
            LayoutRoot {
                available_width(2.0) unit_scale(1.0)
                T.position(-2.1, -1.75, 0.08).scale(2.0, 3.5, 1.0) {
                    R.square() { C.rgba(tint[0], tint[1], tint[2], tint[3]) }
                }
            }
            T {
                Style { display("flex") width(0.0) flex_grow(1.0) height(3.5) align_items("center") padding_xy(0.3, 1.5) font_size(text_scale) }
                Text { title C.rgba(tint[0], tint[1], tint[2], tint[3]) }
            }
        }
        content_factory = fn() {
            return T {
                name = id + "_content"
                Style { display("block") width(section_w) height(section_h) }
                LayoutRoot {
                    name = id + "_layout"
                    available_width(section_w) available_height(section_h) unit_scale(1.0)
                    T {
                        Style {
                            display("flex") flex_direction("column")
                            width(section_w) height(section_h) padding(1.5)
                        }
                        words("INPUT / OUTPUT / HOW / WHY  —  CLICK A CARD FOR DETAILS", 0.70 * text_scale, tint, section_w - 3.0, 1.5 * text_scale)
                        words(description, 0.65 * text_scale, muted, section_w - 3.0, 1.4 * text_scale)
                        T { Style { display("block") width(section_w - 3.0) height(0.6 * text_scale) } }
                        content_factory()
                    }
                }
                decorations_factory()
            }
        }
    })
    return T.position(group_x, group_y, 0.0).scale(0.10, 0.10, 0.10) { panel }
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

fn empty_decorations() { return T {} }

fn operator_catalog_content() {
    return T {
        Style { display("flex") flex_direction("column") row_gap(5.0) width(173.0) height(77.0) }
        T {
            Style { display("flex") flex_direction("row") column_gap(7.0) width(173.0) height(15.5) }
            card("math_graph", violet, 38.0)
            card("value_source", gold, 38.0)
            card("quat_relative", cyan, 38.0)
            card("quat_pitch", cyan, 38.0)
        }
        T {
            Style { display("flex") flex_direction("row") column_gap(7.0) width(173.0) height(15.5) }
            card("scalar_arithmetic", cyan, 38.0)
            card("scalar_clamp", cyan, 38.0)
            card("quat_axis_angle", cyan, 38.0)
            card("quat_compose", cyan, 38.0)
        }
        T {
            Style { display("flex") flex_direction("row") column_gap(7.0) width(173.0) height(15.5) }
            card("vector_rotate", cyan, 38.0)
            card("vector_arithmetic", cyan, 38.0)
            card("vector_project", cyan, 38.0)
            card("vector_measure", cyan, 38.0)
        }
        T {
            Style { display("flex") flex_direction("row") column_gap(7.0) width(173.0) height(15.5) }
            card("quat_directions", green, 38.0)
            card("op_follow", violet, 38.0)
            card("channel_output", green, 38.0)
            card("op_yaw", cyan, 38.0)
        }
    }
}

fn constraint_composition_content() {
    return T {
        Style { display("flex") flex_direction("column") width(263.0) }
        T {
            Style { display("flex") flex_direction("column") width(263.0) }
            T {
                name = "avc_graph_support_row"
                Style { display("flex") flex_direction("row") align_items("start") column_gap(7.0) width(263.0) height(48.0) margin_bottom(6.0) }
                T {
                    Style { display("flex") flex_direction("column") width(38.0) flex_shrink(0.0) }
                    flow_words("BOUND INPUTS / SHARED VALUES", 0.70 * graph_text_scale, muted, 38.0)
                    flow_words("Rest geometry and existing body yaw\nfeed typed inputs in the graph below.\n\nNo MMS AST or callback is evaluated\nfor an ordinary numeric update.", 0.65 * graph_text_scale, muted, 38.0)
                }
                graph_card("rig_bind", gold, 38.0)
                graph_card("used_yaw", cyan, 38.0)
            }
            T {
                name = "avc_graph_pose_row"
                Style { display("flex") flex_direction("row") align_items("start") column_gap(7.0) width(263.0) height(54.0) margin_bottom(6.0) }
                graph_card("pose_inputs", muted, 38.0)
                graph_card("used_math_rotation", cyan, 38.0)
                graph_card("used_math_measure", cyan, 38.0)
                graph_card("used_follow", violet, 38.0)
                graph_card("used_math_closure", green, 38.0)
                graph_card("used_math_publish", green, 38.0)
            }
            T {
                name = "avc_graph_state_row"
                Style { display("flex") flex_direction("row") align_items("start") column_gap(7.0) width(263.0) height(48.0) }
                for stage_id in ["pose_inputs", "used_math_rotation", "used_math_measure", "used_follow", "used_math_closure"] {
                    T {
                        Style { display("block") width(38.0) flex_shrink(0.0) }
                        scaled_component_badges(stage_id, 38.0, graph_text_scale)
                    }
                }
                graph_card("accepted_anchor", violet, 38.0)
            }
        }
        T { Style { display("block") width(263.0) height(8.0) } }
        flow_words("Numeric components return typed values. TransformChannelOutput bridges the accepted pose into the transform system; only then is next follow state committed.", 0.66 * graph_text_scale, muted, 263.0)
    }
}

fn constraint_wires() {
    // The parent body is styled. A private layout boundary protects the
    // authored connector coordinates from visual-content auto-alignment.
    return LayoutRoot {
        available_width(266.0) unit_scale(1.0)
        T.position(0.0, 0.0, 0.08) {
            name = "avc_constraint_wires"
            // Reserved routing lanes are separate from intrinsic card heights.
            // Coordinates follow the word-wrapped card bounds, not fixed card sizes.
            stroke([[20.5, 64.6], [20.5, 61.6], [64.7, 61.6]], cyan)
            wire([[66.3, 61.6], [110.5, 61.6], [110.5, 54.216]], cyan)
            wire([[65.5, 56.322], [65.5, 64.6]], gold)
            wire([[121.5, 54.216], [121.5, 62.6], [78.5, 62.6], [78.5, 64.6]], cyan)
            wire([[39.5, 84.6], [46.5, 84.6]], cyan)
            wire([[84.5, 84.6], [91.5, 84.6]], cyan)
            wire([[129.5, 84.6], [136.5, 84.6]], violet)
            wire([[174.5, 84.6], [181.5, 84.6]], green)
            wire([[219.5, 84.6], [226.5, 84.6]], green)
            wire([[245.5, 109.854], [245.5, 124.6]], green)
            wire([[226.5, 147.656], [223.0, 147.656], [223.0, 174.6], [178.0, 174.6], [178.0, 120.6], [155.5, 120.6], [155.5, 106.474]], violet)
            T.position(136.5, -177.1, 0.03) {
                LayoutRoot {
                    available_width(88.0) unit_scale(1.0)
                    flow_words("ACCEPTED ANCHOR / NEXT UPDATE", 0.62 * graph_text_scale, violet, 88.0)
                }
            }
        }
    }
}

fn future_example_content() {
    return T {
        Style { display("flex") flex_direction("row") column_gap(5.0) width(169.0) height(14.0) align_items("start") }
        example_card("scene_desktop", gold, 53.0)
        example_card("scene_xr", gold, 53.0)
        example_card("scene_checks", gold, 53.0)
    }
}

// These three groups are independently draggable; no outer layout repositions
// them after a drag. The middle lane keeps its connectors in its own local frame.
T {
    name = "avc_constraint_proposal"
    Selection { name = "avc_diagram_selection" }
    T.position(-13.3, 11.40, 0.0).scale(0.10, 0.10, 0.10) {
        LayoutRoot {
            available_width(266.0) available_height(6.0) unit_scale(1.0)
            T {
                Style { display("flex") flex_direction("column") width(266.0) height(6.0) }
                words("AVC / TYPED VALUE OPERATORS", 1.55, white, 266.0, 2.3)
                words("INTERACTIVE PROPOSAL / 2026-10-17     MMS constructs the graph; the engine evaluates retained typed operations.", 0.75, muted, 266.0, 1.7)
                words("SOLID / BLACK TEXT: proposed. OUTLINED: existing context. Named operator tags match catalog fills.
Drag a title / left bar; arrow unloads / restores content. Wires: CYAN pose, VIOLET state, GOLD calibration, GREEN accepted outputs.", 0.68, muted, 266.0, 2.0)
            }
        }
    }
    section("avc_operator_library", "01 / PROPOSED VALUE OPERATOR COMPONENTS", "Proposed long names; shortforms are undecided. Scalar / Vec3 / Quaternion ports form a small native graph, with input and transform-output boundaries.", operator_catalog_content, 176.0, 84.5, cyan, -13.3, 10.8, empty_decorations)
    section("avc_constraint_composition", "02 / AVC-GENERATED VALUE GRAPH", "Read the pose flow left to right. Click a stage for the named components it composes. Calibration/yaw feed it from above; accepted state returns underneath.", constraint_composition_content, 266.0, 188.0, green, -13.3, 1.85, constraint_wires)
    section("avc_future_example", "03 / FUTURE MMS EXAMPLE", "Ordinary Input or InputXR with AVC requests automatic graph construction. These displayed outlines are not running avatars.", future_example_content, 172.0, 21.0, gold, -13.3, -17.9, empty_decorations)
}

let details_panel = info_panel({
    root_name = "avc_diagram_details"
    width_gu = 52.0
    unit_scale = 0.075
    title = T {
        name = "avc_diagram_details_title_bar"
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
T.position(6.0, 10.65, 1.0) {
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
    T.position(0.0, 1.4, 18.0) { C3D { Pointer {} } }
}
