// accordion.mms — draggable/grabbable title, removable-body panel shell (=^･ω･^=)

import { accordion_down_arrow_icon } from "../../icons.mms"

// Options:
//   root_name        public name of the inner draggable panel transform
//   width_gu         panel width in layout units
//   unit_scale       required scale for the private LayoutRoot
//   background_color title-bar background RGBA
//   toggle_background_color optional minimize-button background RGBA
//   toggle_icon_color optional minimize-arrow RGBA
//   toggle_icon_glow_intensity optional minimize-arrow emissive strength
//   toggle_icon      optional component replacing the disclosure glyph
//   initial_expanded optional boolean, defaults to true
//   toggle_decoration optional fixed component behind the rotating glyph
//   material         optional self-contained panel material, retained on grab
//   children         ordered retained title-bar component objects
//   title_bar        optional component replacing the standard title bar;
//                    owns its layout/markers, receives the minimize control last
//   body             one component rooted at #accordion_body
//   unlit            use a shared, non-blooming UI material
//
// The returned object is the layout-owned outer slot. The default minimize
// toggle is always inserted before caller children. Accordion events originate
// on the inner panel root and therefore bubble to handlers on the named slot.

let ACCORDION_TITLE_HEIGHT_GU = 3.5
let ACCORDION_TOGGLE_WIDTH_GU = 4.0
let ACCORDION_BODY_GAP_GU = 0.6
let ACCORDION_TOGGLE_TRANSITION_BEATS = 0.6

export fn accordion_body(content) {
    return T {
        name = "accordion_body"
        Style {
            display("flex")
            flex_direction("column")
            width(100%)
            margin_xy(0.0, ACCORDION_BODY_GAP_GU / 2.0)
        }
        content
    }
}

export fn accordion(options) {
    let state = { expanded = options["initial_expanded"] != false }
    let root_name = options.root_name
    let width_gu = options.width_gu
    let accordion_unit_scale = options.unit_scale
    let background_rgba = options.background_color
    let toggle_background_rgba = [0.12, 0.34, 0.66, 1.0]
    let authored_toggle_background = options["toggle_background_color"]
    if authored_toggle_background {
        toggle_background_rgba = authored_toggle_background
    }
    let toggle_icon_rgba = [0.72, 0.90, 1.0, 1.0]
    let authored_toggle_icon_color = options["toggle_icon_color"]
    if authored_toggle_icon_color { toggle_icon_rgba = authored_toggle_icon_color }
    let toggle_icon_glow = 1.8
    let authored_toggle_icon_glow = options["toggle_icon_glow_intensity"]
    if authored_toggle_icon_glow != null { toggle_icon_glow = authored_toggle_icon_glow }
    let title_children = options.children

    let glyph = options["toggle_icon"]
    if glyph == null { glyph = accordion_down_arrow_icon(toggle_icon_rgba, toggle_icon_glow) }
    let toggle_icon = T.position(
        ACCORDION_TOGGLE_WIDTH_GU / 2.0 * accordion_unit_scale,
        -ACCORDION_TITLE_HEIGHT_GU / 2.0 * accordion_unit_scale,
        0.02,
    ).scale(1.62 * accordion_unit_scale, 1.62 * accordion_unit_scale, 1.0).rotation(0.0, 0.0, 0.0) {
        name = "accordion_toggle_icon"
        Transition {
            duration_beats(ACCORDION_TOGGLE_TRANSITION_BEATS)
            ease_out_cubic()
            capture_from_current(true)
            replace_same_target()
        }
        glyph
    }

    let toggle = T {
        name = "accordion_toggle"
        Grabbable.off()
        Raycastable.enabled() { interaction_priority(110.0) }
        Style {
            display("flex")
            width(ACCORDION_TOGGLE_WIDTH_GU)
            height(ACCORDION_TITLE_HEIGHT_GU)
            align_items("center")
            justify_content("center")
            background_color(toggle_background_rgba)
            background_z(-0.015)
            color([0.97, 0.99, 1.0, 1.0])
        }
        T.position(ACCORDION_TOGGLE_WIDTH_GU/2.0*accordion_unit_scale,
            -ACCORDION_TITLE_HEIGHT_GU/2.0*accordion_unit_scale,-0.035)
            .scale(1.62*accordion_unit_scale,1.62*accordion_unit_scale,1.62*accordion_unit_scale) {
            options["toggle_decoration"]
        }
        toggle_icon
    }

    let body_mount = T {
        name = "accordion_body_mount"
        Style {
            display("flex")
            flex_direction("column")
            width(100%)
        }
        if state.expanded { options.body }
    }

    let drag_target = "../../#" + root_name
    let title_bar = options["title_bar"]
    if title_bar != null {
        title_bar.attach(toggle)
    } else {
        title_bar = T {
            name = "title_bar"
            Draggable.target(drag_target)
            Grabbable.parent()
            Raycastable.enabled()
            Style {
                display("flex")
                flex_direction("row")
                align_items("center")
                width(100%)
                height(ACCORDION_TITLE_HEIGHT_GU)
                background_color(background_rgba)
                background_z(-0.01)
            }
            toggle
            for child in title_children { child }
        }
    }
    let panel_root = T {
        name = root_name
        if options["material"] != null { options.material }
        if options["unlit"] { Unlit {} }
        Option {}
        Raycastable.enabled() { interaction_priority(100.0) }

        LayoutRoot {
            available_width(width_gu)
            unit_scale(accordion_unit_scale)

            title_bar
            body_mount
        }
    }
    let layout_slot = T {
        name = "accordion_layout_slot"
        Style { width(width_gu) }
        panel_root
    }

    fn set_expanded(value) {
        if value == state.expanded { return }
        state.expanded = value
        let current_body = body_mount.query("#accordion_body")
        let angle = 0.0
        if !value {
            angle = -1.570796
            if current_body { current_body.remove_subtree() }
        }
        toggle_icon.update_transform(
            [ACCORDION_TOGGLE_WIDTH_GU/2.0*accordion_unit_scale,-ACCORDION_TITLE_HEIGHT_GU/2.0*accordion_unit_scale,0.02],
            [0,0,angle],[1.62*accordion_unit_scale,1.62*accordion_unit_scale,1.0])
        if value { emit_data(panel_root,"AccordionRestoreRequested",body_mount) }
        else { emit_data(panel_root,"AccordionMinimized",body_mount) }
    }
    panel_root.set_expanded = set_expanded
    panel_root.is_expanded = fn() { return state.expanded }
    layout_slot.set_expanded = set_expanded
    layout_slot.is_expanded = fn() { return state.expanded }
    if !state.expanded {
        toggle_icon.update_transform(
            [ACCORDION_TOGGLE_WIDTH_GU/2.0*accordion_unit_scale,-ACCORDION_TITLE_HEIGHT_GU/2.0*accordion_unit_scale,0.02],
            [0,0,-1.570796],[1.62*accordion_unit_scale,1.62*accordion_unit_scale,1.0])
    }
    on(toggle,"Click",fn(event) { set_expanded(!state.expanded) })

    return layout_slot
}
