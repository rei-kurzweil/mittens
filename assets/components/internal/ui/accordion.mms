// accordion.mms — retained-title, removable-body editor shell (=^･ω･^=)

import { accordion_down_arrow_icon } from "../../icons.mms"

// Options:
//   root_name        public name of the inner draggable panel transform
//   width_gu         panel width in layout units
//   unit_scale       required scale for the private LayoutRoot
//   background_color title-bar background RGBA
//   toggle_background_color optional minimize-button background RGBA
//   toggle_icon_color optional minimize-arrow RGBA
//   toggle_icon_glow_intensity optional minimize-arrow emissive strength
//   children         ordered retained title-bar component objects
//   body             one component rooted at #accordion_body
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
        accordion_down_arrow_icon(toggle_icon_rgba, toggle_icon_glow)
    }

    let toggle = T {
        name = "accordion_toggle"
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
        toggle_icon
    }

    let body_mount = T {
        name = "accordion_body_mount"
        Style {
            display("flex")
            flex_direction("column")
            width(100%)
        }
        options.body
    }

    let drag_target = "../../#" + root_name
    let panel_root = T {
        name = root_name
        Option {}
        Raycastable.enabled() { interaction_priority(100.0) }

        LayoutRoot {
            available_width(width_gu)
            unit_scale(accordion_unit_scale)

            T {
                name = "title_bar"
                Draggable.target(drag_target)
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

            body_mount
        }
    }
    let layout_slot = T {
        name = "accordion_layout_slot"
        Style { width(width_gu) }
        panel_root
    }

    on(toggle, "Click", fn(event) {
        let current_body = body_mount.query("#accordion_body")
        if current_body {
            current_body.remove_subtree()
            toggle_icon.update_transform(
                [ACCORDION_TOGGLE_WIDTH_GU / 2.0 * accordion_unit_scale, -ACCORDION_TITLE_HEIGHT_GU / 2.0 * accordion_unit_scale, 0.02],
                [0.0, 0.0, -1.570796],
                [1.62 * accordion_unit_scale, 1.62 * accordion_unit_scale, 1.0],
            )
            emit_data(panel_root, "AccordionMinimized", body_mount)
        } else {
            toggle_icon.update_transform(
                [ACCORDION_TOGGLE_WIDTH_GU / 2.0 * accordion_unit_scale, -ACCORDION_TITLE_HEIGHT_GU / 2.0 * accordion_unit_scale, 0.02],
                [0.0, 0.0, 0.0],
                [1.62 * accordion_unit_scale, 1.62 * accordion_unit_scale, 1.0],
            )
            emit_data(panel_root, "AccordionRestoreRequested", body_mount)
        }
    })

    return layout_slot
}
