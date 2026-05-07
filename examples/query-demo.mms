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
    window_size(900, 720)
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

    T.position(0.0, -0.5, 3.0) {
        C3D {}
        Pointer {}
    }
}

BG {
    T.position(0, -10, 0).scale(10000, 1, 10000) {
        R.cube() { C.rgba(0.6, 0.6, 0.6, 1.0) }
    }
}

// ── Layout root ──────────────────────────────────────────────────────────────
// 80 gu wide × 60 gu tall, scale 0.06 → ~4.8 × 3.6 world units.
// Default body display is block → demo rows stack vertically.


let layout_root = null; 
let red = 1.0
let green = 0.2
let blue = 0.2


T.position(-1.2, 0, 0.0).scale(0.06, 0.06, 0.06) {
    layout_root = LayoutRoot {
        available_width(35.0)
        available_height(80.0)

        // ── Demo A: query("#target_a") ────────────────────────────────────────
        T.position(0, 0, 0.05) {
            name = "row_a"
            Style.display("flex")
                    .flex_direction("row")
                    .gap(1.0)
                    .padding(1.0)
                    .background_color([red, green, blue, 1.0]) {}

            T.position(0, 0, 0.05) {
                name = "btn_a"
                Style.padding_xy(2.0, 4.0)
                        .background_color([red * 0.75, green * 0.75, blue * 0.75, 1.0]) {}
                Text {
                    Raycastable.enabled()
                    "query('#target_a')"
                }
            }

            T.position(0, 0, 0.05) {
                Style.padding_xy(4.0, 4.0)
                        .background_color([red * 0.5, green * 0.5, blue * 0.5, 1.0]) {}
                Text { "(unclicked)" name = "target_a" }
            }
        }
        
        green = 0.9
        red   = 1.0
        blue  = 0.2

        // ── Demo B: '#target_b' -> set_text(..) ──────────────────────────────
        T.position(0, 0, 0.05) {
            name = "row_b"
            Style.display("flex")
                    .flex_direction("row")
                    .gap(1.0)
                    .padding(1.0)
                    .background_color([red, green, blue, 1.0]) {}

            T.position(0, 0, 0.05) {
                name = "btn_b"
                Style.padding_xy(2.0, 1.0)
                        .background_color([red * 0.75, green * 0.75, blue * 0.75, 1.0]) {}
                Text {
                    Raycastable.enabled()
                    "'#target_b' -> set_text(..)"
                }
            }

            T.position(0, 0, 0.05) {
                Style.padding_xy(4.0, 2.0)
                        .background_color([red * 0.5, green * 0.5, blue * 0.5, 1.0]) {}
                Text { "(unclicked)" name = "target_b" }
            }
        }

        green = 1.0
        blue  = 0.1
        red   = 0.2

        // ── Demo C: '#target_c' -> fn(t) {..} ────────────────────────────────
        T.position(0, 0, 0.05) {
            name = "row_c"
            Style.display("flex")
                    .flex_direction("row")
                    .gap(1.0)
                    .padding(1.0)
                    .background_color([red, green, blue, 1.0]) {}

            T.position(0, 0, 0.05) {
                name = "btn_c"
                Style.padding_xy(2.0, 1.0)
                        .background_color([red * 0.75, green * 0.75, blue * 0.75, 1.0]) {}
                Text {
                    Raycastable.enabled()
                    "'#target_c' -> fn(t) {..}"
                }
            }

            T.position(0, 0, 0.05) {
                Style.padding_xy(4.0, 2.0)
                        .background_color([red * 0.5, green * 0.5, blue * 0.5, 1.0]) {}
                Text { "(unclicked)" name = "target_c" }
            }
        }

        red   = 0.2
        green = 0.2
        blue  = 1.0

        // ── Demo D: row_d.query_all("text")  (hits label + target) ───────────
        T.position(0, 0, 0.05) {
            name = "row_d"
            Style.display("flex")
                    .flex_direction("row")
                    .gap(1.0)
                    .padding(1.0)
                    .background_color([red, green, blue, 1.0]) {}

            T.position(0, 0, 0.05) {
                name = "btn_d"
                Style.padding_xy(2.0, 1.0)
                        .background_color([red * 0.75, green * 0.75, blue * 0.75, 1.0]) {}
                Text {
                    Raycastable.enabled()
                    "row_d.query_all('text')"
                }
            }

            T.position(0, 0, 0.05) {
                Style.padding_xy(4.0, 2.0)
                        .background_color([red * 0.5, green * 0.5, blue * 0.5, 1.0]) {}
                Text { "(unclicked)" name = "target_d" }
            }
        }
    }

    layout_root
}


// ── click handlers — one per query form ───────────────────────────────────────
let btn_a = layout_root.query("#btn_a")
let btn_b = layout_root.query("#btn_b")
let btn_c = layout_root.query("#btn_c")
let btn_d = layout_root.query("#btn_d")
    
print("layout_root exists")

on(btn_a, "Click", fn(event) {
    print("btn_a clicked")
    let t = layout_root.query("#target_a")

    if t {
        t.set_text("query() hit me!")
    }
})




on(btn_b, "Click", fn(event) {
    print("btn_b clicked")
    "#target_b" -> set_text("-> shorthand!")
})

on(btn_c, "Click", fn(event) {
    print("btn_c clicked")
    "#target_c" -> fn(t) {
        if t { t.set_text("-> callback!") }
    }
})
// 
on(btn_d, "Click", fn(event) {
    print("btn_d clicked")
    // Descendant combinator — every `text` under `#row_d` (label + target).
    for t in query_all("#row_d text") {
        t.set_text("query_all hit")
    }
})
