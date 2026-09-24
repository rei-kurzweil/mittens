// info_panel.mms — reusable authored title/content accordion panel (=^･ω･^=)
//
// Required options:
//   root_name, width_gu, unit_scale, title, content
// Optional options:
//   icon, background_color (or title_background_color), toggle_background_color,
//   toggle_icon_color, toggle_icon_glow_intensity, body_background_color,
//   title_text_color, body_text_color
//
// The panel wraps `content` in its body internally. When the panel is restored
// after minimizing, the caller receives the forwarded
// AccordionRestoreRequested event and may attach a freshly built body to the
// event payload's accordion_body_mount using info_panel_body(options).

import { accordion, accordion_body } from "../internal/ui/accordion.mms"

let INFO_PANEL_TITLE_HEIGHT_GU = 3.5
let INFO_PANEL_TITLE_BACKGROUND = [0.12, 0.12, 0.14, 0.98]
let INFO_PANEL_TOGGLE_BACKGROUND = [0.24, 0.24, 0.27, 1.0]
let INFO_PANEL_BODY_BACKGROUND = [0.075, 0.075, 0.09, 0.96]
let INFO_PANEL_TITLE_TEXT = [0.96, 0.96, 0.98, 1.0]
let INFO_PANEL_BODY_TEXT = [0.96, 0.96, 0.98, 1.0]

fn info_panel_title(title, text_color) {
    return T {
        name = "info_panel_title"
        Style {
            display("flex")
            width(0.0)
            flex_grow(1.0)
            height(INFO_PANEL_TITLE_HEIGHT_GU)
            align_items("center")
            padding_xy(0.9, 0.3)
            color(text_color)
        }
        T.position(0.0, 0.0, 0.02) { Text { title } }
    }
}

fn info_panel_icon(icon) {
    return T {
        name = "info_panel_icon"
        Style {
            display("flex")
            width(INFO_PANEL_TITLE_HEIGHT_GU)
            height(INFO_PANEL_TITLE_HEIGHT_GU)
            align_items("center")
            justify_content("center")
        }
        T.position(0.0, 0.0, 0.02) { icon }
    }
}

// Rebuild the same padded/styled body after AccordionRestoreRequested.
// Required: content. Optional: body_background_color and body_text_color.
export fn info_panel_body(options) {
    let body_background = INFO_PANEL_BODY_BACKGROUND
    if options["body_background_color"] { body_background = options.body_background_color }
    let body_text_color = INFO_PANEL_BODY_TEXT
    if options["body_text_color"] { body_text_color = options.body_text_color }
    return accordion_body(T {
        name = "info_panel_content"
        Style {
            display("flex")
            flex_direction("column")
            width(100%)
            padding(0.65)
            row_gap(0.35)
            color(body_text_color)
            background_color(body_background)
            background_z(-0.01)
        }
        options.content
    })
}

fn panel_with_title_children(options, title_children, title_bar_background, toggle_background, toggle_icon_color, toggle_icon_glow, body_background, body_text_color) {
    let body = info_panel_body({
        content = options.content
        body_background_color = body_background
        body_text_color = body_text_color
    })
    return accordion({
        root_name = options.root_name
        width_gu = options.width_gu
        unit_scale = options.unit_scale
        background_color = title_bar_background
        toggle_background_color = toggle_background
        toggle_icon_color = toggle_icon_color
        toggle_icon_glow_intensity = toggle_icon_glow
        children = title_children
        body = body
    })
}

// The returned accordion root emits AccordionMinimized and
// AccordionRestoreRequested. The latter carries accordion_body_mount as its
// payload so the caller can reload dynamic content.
export fn info_panel(options) {
    let title_bar_background = INFO_PANEL_TITLE_BACKGROUND
    let authored_background_color = options["background_color"]
    if authored_background_color { title_bar_background = authored_background_color }
    let authored_title_background_color = options["title_background_color"]
    if authored_title_background_color { title_bar_background = authored_title_background_color }

    let toggle_background = INFO_PANEL_TOGGLE_BACKGROUND
    let authored_toggle_background_color = options["toggle_background_color"]
    if authored_toggle_background_color { toggle_background = authored_toggle_background_color }

    let toggle_icon_color = [0.72, 0.90, 1.0, 1.0]
    let authored_toggle_icon_color = options["toggle_icon_color"]
    if authored_toggle_icon_color { toggle_icon_color = authored_toggle_icon_color }

    let toggle_icon_glow = 1.8
    let authored_toggle_icon_glow = options["toggle_icon_glow_intensity"]
    if authored_toggle_icon_glow != null { toggle_icon_glow = authored_toggle_icon_glow }

    let body_background = INFO_PANEL_BODY_BACKGROUND
    let authored_body_background_color = options["body_background_color"]
    if authored_body_background_color { body_background = authored_body_background_color }

    let title_text_color = INFO_PANEL_TITLE_TEXT
    let authored_title_text_color = options["title_text_color"]
    if authored_title_text_color { title_text_color = authored_title_text_color }

    let body_text_color = INFO_PANEL_BODY_TEXT
    let authored_body_text_color = options["body_text_color"]
    if authored_body_text_color { body_text_color = authored_body_text_color }

    let title = info_panel_title(options.title, title_text_color)
    let icon = options["icon"]
    if icon {
        return panel_with_title_children(
            options,
            [info_panel_icon(icon), title],
            title_bar_background,
            toggle_background,
            toggle_icon_color,
            toggle_icon_glow,
            body_background,
            body_text_color,
        )
    }
    return panel_with_title_children(
        options,
        [title],
        title_bar_background,
        toggle_background,
        toggle_icon_color,
        toggle_icon_glow,
        body_background,
        body_text_color,
    )
}
