// assets/components/world_panel_content.mms
//
// Reusable World panel content factory for MMS-driven panel rendering.
//
// `world_panel_content(items)` returns only the rerenderable content subtree.
// This is the slice that Rust can replace independently later.
//
// v1 item contract: `items` is an array of display strings.
//
// v1 interaction contract:
// - Rust attaches signal handlers after rendering
// - rows must therefore get easy query names under `world_panel_content_root`
// - until items carry stable keys, row names are derived from render order:
//   `item_0`, `item_1`, ...
//
// Draft v2 contract remains records/maps passed from Rust with stable keys and
// target refs, but that is spec work only for now.

let TEXT_SCALE = 0.08

fn world_panel_row(row_name, label, bg) {
    let row = T {
        name = row_name
        Raycastable.enabled()
        Style {
            display("block")
            width(100%)
            margin_xy(0.25, 0.20)
            padding_xy(0.55, 0.45)
            font_size(1)
            background_color = bg
        }
        T.position(0.0, 0.0, 0.0) {
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
        Style {
            display("block")
            width(100%)
        }

        T {
            name = "rows_mount"
            Style {
                display("block")
                width(100%)
            }

            let idx = 0
            for item in items {
                let row_name = "item_" + idx
                world_panel_row(row_name, item, [0.92, 0.97, 0.92, 1.0])
                idx = idx + 1
            }
        }
    }

    return root
}