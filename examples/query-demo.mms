// query-demo.mms
// Four demo rows, stacked in a LayoutRoot. Each row is a styled container
// holding a styled button (clickable) and a styled output cell with a target
// Text. Click a button → its target Text mutates via set_text().
//
// Forms exercised:
//   1. query("#name")               -- single result by name
//   2. "selector" -> method(args)   -- method-shorthand desugar
//   3. "selector" -> fn(t) { ... }  -- callback handler
//   4. comp.query_all("text")       -- scoped subtree query, multiple results

RendererSettings {
    window_size(1280, 720)
}

BGC {
    C.rgba(0.9, 0.9, 0.9, 1.0)
}

AL.rgb(0.55, 0.55, 0.60)

T.position(3.0, 5.0, 4.0) {
    DL {}
}

I.speed(3.0) {
    InputTransformMode.forward_z() {
        fps_rotation()
        roll_axis_y()
    }

    T.position(0.0, 0.5, 6.0) {
        C3D {}
        Pointer {}
    }
}

// ── Layout root ──────────────────────────────────────────────────────────────
// 80 gu wide × 60 gu tall, scale 0.06 → ~4.8 × 3.6 world units.
// Default body display is block → demo rows stack vertically.
let layout_root = null; 

T.position(-2.4, 1.8, 0.0).scale(0.06, 0.06, 0.06) {
    layout_root = LayoutRoot {
        available_width(80.0)
        available_height(60.0)

        // ── Demo A: query("#target_a") ────────────────────────────────────────
        T.position(0, 0, 0.05) {
            name = "row_a"
            Style.display("flex")
                    .flex_direction("row")
                    .gap(1.0)
                    .padding(1.0)
                    .background_color([0.10, 0.30, 0.80, 1.0]) {}

            T.position(0, 0, 0.05) {
                name = "btn_a"
                Raycastable.enabled()
                Style.padding_xy(2.0, 1.0)
                        .background_color([0.50, 0.60, 0.90, 1.0]) {}
                Text { "query('#target_a')" }
            }

            T.position(0, 0, 0.05) {
                Style.padding_xy(4.0, 2.0)
                        .background_color([0.60, 0.70, 0.95, 1.0]) {}
                Text { "(unclicked)" name = "target_a" }
            }
        }
        
        // ── Demo B: '#target_b' -> set_text(..) ──────────────────────────────
        T.position(0, 0, 0.05) {
            name = "row_b"
            Style.display("flex")
                    .flex_direction("row")
                    .gap(1.0)
                    .padding(1.0)
                    .background_color([0.10, 0.70, 0.30, 1.0]) {}

            T.position(0, 0, 0.05) {
                name = "btn_b"
                Raycastable.enabled()
                Style.padding_xy(2.0, 1.0)
                        .background_color([0.50, 0.85, 0.60, 1.0]) {}
                Text { "'#target_b' -> set_text(..)" }
            }

            T.position(0, 0, 0.05) {
                Style.padding_xy(4.0, 2.0)
                        .background_color([0.65, 0.90, 0.75, 1.0]) {}
                Text { "(unclicked)" name = "target_b" }
            }
        }

        // ── Demo C: '#target_c' -> fn(t) {..} ────────────────────────────────
        T.position(0, 0, 0.05) {
            name = "row_c"
            Style.display("flex")
                    .flex_direction("row")
                    .gap(1.0)
                    .padding(1.0)
                    .background_color([0.90, 0.45, 0.10, 1.0]) {}

            T.position(0, 0, 0.05) {
                name = "btn_c"
                Raycastable.enabled()
                Style.padding_xy(2.0, 1.0)
                        .background_color([0.95, 0.70, 0.50, 1.0]) {}
                Text { "'#target_c' -> fn(t) {..}" }
            }

            T.position(0, 0, 0.05) {
                Style.padding_xy(4.0, 2.0)
                        .background_color([1.00, 0.85, 0.75, 1.0]) {}
                Text { "(unclicked)" name = "target_c" }
            }
        }

        // ── Demo D: row_d.query_all("text")  (hits label + target) ───────────
        T.position(0, 0, 0.05) {
            name = "row_d"
            Style.display("flex")
                    .flex_direction("row")
                    .gap(1.0)
                    .padding(1.0)
                    .background_color([0.80, 0.20, 0.70, 1.0]) {}

            T.position(0, 0, 0.05) {
                name = "btn_d"
                Raycastable.enabled()
                Style.padding_xy(2.0, 1.0)
                        .background_color([0.90, 0.60, 0.85, 1.0]) {}
                Text { "row_d.query_all('text')" }
            }

            T.position(0, 0, 0.05) {
                Style.padding_xy(4.0, 2.0)
                        .background_color([0.95, 0.75, 0.90, 1.0]) {}
                Text { "(unclicked)" name = "target_d" }
            }
        }
    }

    layout_root
}

// ── click handlers — one per query form ───────────────────────────────────────
if (layout_root) {
// let btn_a = layout_root.query("#btn_a")
// let btn_b = layout_root.query("#btn_b")
// let btn_c = layout_root.query("#btn_c")
// let btn_d = layout_root.query("#btn_d")
print("layout_root exists")


if btn_a {
    print("btn_a")
    print(btn_a)
}
}

// on(btn_a, "Click", fn(event) {
//      let t = layout_root.query("#target_a")
// 
//      if t {
//          t.set_text("query() hit me!")
//      }
// })


// on(btn_b, "Click", fn(event) {
//     "#target_b" -> set_text("-> shorthand!")
// })

// on(btn_c, "Click", fn(event) {
//     "#target_c" -> fn(t) {
//         if t { t.set_text("-> callback!") }
//     }
// })

// on(btn_d, "Click", fn(event) {
//     // Descendant combinator — every `text` under `#row_d` (label + target).
//     for t in query_all("#row_d text") {
//         t.set_text("query_all hit")
//     }
// })
