// assets/components/world_panel_content.mms
//
// Reusable World panel content factory for MMS-driven panel rendering.
//
// `world_panel_content(items)` returns only the rerenderable content subtree.
// This is the slice that Rust can replace independently later.
//
// v1 item contract: `items` is an array of display strings.
//
// Draft v2 contract:
// - each item should carry a stable key
// - each item should carry its display label
// - each item should carry the target component identity it represents
//   as a target_ref field inside a record / map item
// - the preferred first encoding for that target_ref is a canonical string
//   in `@uuid:...` form
// - selector strings are the fallback, not the preferred contract
// - item click handlers should be authored here in MMS and emit an
//   editor-facing intent such as `EDITOR_SELECT(item.target_ref)`
//
// That intent surface is still spec-only; no runtime support is added here yet.

let TEXT_SCALE = 0.08

fn world_panel_row(label, bg) {
    let row = T.position(0.0, 0.0, 0.1) {
        Raycastable.enabled()
        Style {
            margin_xy(0.25, 0.20)
            padding_xy(0.55, 0.45)
            background_color = bg
        }
        T.position(0.0, 0.0, 0.015).scale(TEXT_SCALE, TEXT_SCALE, TEXT_SCALE) {
            Text {
                label
                C.rgba(0.06, 0.09, 0.08, 1.0)
            }
        }
    }
    return row
}

export fn world_panel_content(items) {
    let root = T {
        name = "world_panel_content_root"

        T {
            name = "rows_mount"
            Style {
                width(100%)
            }

            for item in items {
                world_panel_row(item, [0.92, 0.97, 0.92, 1.0])
            }
        }
    }

    return root
}