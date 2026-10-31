// info_panel.mms — reusable authored title/content accordion panel (=^･ω･^=)
//
// Required options:
//   root_name, width_gu, unit_scale, title, and content OR content_factory
//   title: string for the standard header, or a component used as the actual
//   title-bar root. Custom bars own their Style/layout; the minimize control
//   is appended after their authored children. Put movement markers on that root.
// Optional options:
//   icon, background_color (or title_background_color), toggle_background_color,
//   toggle_icon_color, toggle_icon_glow_intensity, body_background_color,
//   title_text_color, body_text_color, inherit_body_text_color,
//   transparent_background, body_padding
//   inherit_body_text_color: false omits the panel-level text color so content
//     with independent/nested layouts owns its colors (default true).
//   transparent_background: omit the body background geometry (default false).
//     The retained title/toggle keep their independently configurable colors.
//   body_padding: body inset in layout units (default 0.65).
// Panels use lighting-independent colors without bloom; a nearer authored
// material can override that choice.
//   content_factory: zero-argument function returning a fresh content subtree.
//     Called initially and on restore; minimizing deletes that subtree.
//
// The panel wraps `content` in its body internally. When the panel is restored
// after minimizing, the caller receives the forwarded
// AccordionRestoreRequested event and may attach a freshly built body to the
// event payload's accordion_body_mount using info_panel_body(options).
// With content_factory, the panel handles restoration itself. Do not also
// install a manual restore handler for that panel.

import { accordion, accordion_body } from "../internal/ui/accordion.mms"

let INFO_PANEL_TITLE_HEIGHT_GU = 3.5
let INFO_PANEL_TITLE_BACKGROUND = [0.93, 0.87, 0.71, 1.0]
let INFO_PANEL_TOGGLE_BACKGROUND = [0.88, 0.81, 0.66, 1.0]
let INFO_PANEL_BODY_BACKGROUND = [0.98, 0.95, 0.84, 1.0]
let INFO_PANEL_TITLE_TEXT = [0.0, 0.0, 0.0, 1.0]
let INFO_PANEL_BODY_TEXT = [0.0, 0.0, 0.0, 1.0]

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
// Required: content. Optional body styling matches info_panel(options).
export fn info_panel_body(options) {
    let body_background = INFO_PANEL_BODY_BACKGROUND
    if options["body_background_color"] { body_background = options.body_background_color }
    let body_text_color = INFO_PANEL_BODY_TEXT
    if options["body_text_color"] { body_text_color = options.body_text_color }
    let body_padding = 0.65
    if options["body_padding"] != null { body_padding = options.body_padding }
    return accordion_body(T {
        name = "info_panel_content"
        Style {
            display("flex")
            flex_direction("column")
            width(100%)
            padding(body_padding)
            row_gap(0.35)
            if options["inherit_body_text_color"] != false { color(body_text_color) }
            if !options["transparent_background"] {
                background_color(body_background)
                background_z(-0.01)
            }
        }
        options.content
    })
}

fn panel_with_title_children(options, title_children, custom_title_bar, title_bar_background, toggle_background, toggle_icon_color, toggle_icon_glow, body_background, body_text_color) {
    let panel = accordion({
        root_name = options.root_name
        unlit = true
        width_gu = options.width_gu
        unit_scale = options.unit_scale
        background_color = title_bar_background
        toggle_background_color = toggle_background
        toggle_icon_color = toggle_icon_color
        toggle_icon_glow_intensity = toggle_icon_glow
        children = title_children
        title_bar = custom_title_bar
        body = make_panel_body(options, body_background, body_text_color)
    })
    if options["content_factory"] {
        let body_mount = panel.query("#accordion_body_mount")
        on(panel, "DataEvent", fn(event) {
            if event == "AccordionRestoreRequested" {
                if !body_mount.query("#accordion_body") {
                    body_mount.attach(make_panel_body(options, body_background, body_text_color))
                }
            }
        })
    }
    return panel
}

fn make_panel_body(options, body_background, body_text_color) {
    let content = options["content"]
    let factory = options["content_factory"]
    if factory { content = factory() }
    return info_panel_body({
        content = content
        body_background_color = body_background
        body_text_color = body_text_color
        inherit_body_text_color = options["inherit_body_text_color"]
        transparent_background = options["transparent_background"]
        body_padding = options["body_padding"]
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

    let toggle_icon_color = [0.0, 0.0, 0.0, 1.0]
    let authored_toggle_icon_color = options["toggle_icon_color"]
    if authored_toggle_icon_color { toggle_icon_color = authored_toggle_icon_color }

    let toggle_icon_glow = 0.0
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

    if !is_string(options.title) {
        return panel_with_title_children(
            options,
            [],
            options.title,
            title_bar_background,
            toggle_background,
            toggle_icon_color,
            toggle_icon_glow,
            body_background,
            body_text_color,
        )
    }

    let title = info_panel_title(options.title, title_text_color)
    let icon = options["icon"]
    if icon {
        return panel_with_title_children(
            options,
            [info_panel_icon(icon), title],
            null,
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
        null,
        title_bar_background,
        toggle_background,
        toggle_icon_color,
        toggle_icon_glow,
        body_background,
        body_text_color,
    )
}
