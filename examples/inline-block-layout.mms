// inline-block-layout.mms — inline-block formatting context demo (=^･ω･^=)
//
// One LayoutRoot. Three rows stacked block-style. Inside each row, an
// icon + a text run flow inline-block (icon left, text right). When a
// row's children are all inline-block, layout dispatches to the inline
// formatting context recursively.
//
// Topology:
//   LayoutRoot (block)
//     row1 T  + Style (default block — stacks under row0)
//       icon1 T  + Style{display:inline-block, w, h, margin-right}
//         R.circle2d
//       text1 T  + Style{display:inline-block, w, h}
//         Text { "..." }
//     row2 T (triangle)
//     row3 T (square)
//
// Glyph units everywhere; outer wrapper T.scale shrinks gu → world units.

I {
    InputTransformMode.forward_z() {
        roll_axis_y()
        fps_rotation()
    }
    T.position(0.0, 1.6, 3.0) {
        C3D {}
        Pointer {}
    }
}

T.position(-1.6, 1.8, 0.0).scale(0.12, 0.12, 0.12) {
    LayoutRoot {
        name = "rows_root"
        available_width(40.0)
        available_height(20.0)

        // ── Row 1: circle + text ─────────────────────────────────────
        T {
            name = "row1"
            Style {
                margin_bottom(0.5)
                height(2.5)
            }
            T {
                name = "icon_circle"
                Style {
                    display("inline-block")
                    width(2.0)
                    height(2.0)
                    margin_right(0.5)
                }
                R.circle2d() {
                    C.rgba(0.95, 0.45, 0.25, 1.0)
                }
            }
            T {
                name = "text1"
                Style {
                    display("inline-block")
                    width(28.0)
                    height(2.0)
                }
                Text {
                    "it's a piece of cake"
                    C.rgba(0.95, 0.95, 0.95, 1.0)
                }
            }
        }

        // ── Row 2: triangle + text ───────────────────────────────────
        T {
            name = "row2"
            Style {
                margin_bottom(0.5)
                height(2.5)
            }
            T {
                name = "icon_triangle"
                Style {
                    display("inline-block")
                    width(2.0)
                    height(2.0)
                    margin_right(0.5)
                }
                R.triangle() {
                    C.rgba(0.45, 0.85, 0.45, 1.0)
                }
            }
            T {
                name = "text2"
                Style {
                    display("inline-block")
                    width(28.0)
                    height(2.0)
                }
                Text {
                    "to bake"
                    C.rgba(0.95, 0.95, 0.95, 1.0)
                }
            }
        }

        // ── Row 3: square + text ─────────────────────────────────────
        T {
            name = "row3"
            Style {
                margin_bottom(0.5)
                height(2.5)
            }
            T {
                name = "icon_square"
                Style {
                    display("inline-block")
                    width(2.0)
                    height(2.0)
                    margin_right(0.5)
                }
                R.square() {
                    C.rgba(0.4, 0.55, 0.95, 1.0)
                }
            }
            T {
                name = "text3"
                Style {
                    display("inline-block")
                    width(28.0)
                    height(2.0)
                }
                Text {
                    "a pretty cake"
                    C.rgba(0.95, 0.95, 0.95, 1.0)
                }
            }
        }
    }
}

// ── Lighting ────────────────────────────────────────────────────────
AL {
    C.rgba(0.30, 0.30, 0.32, 1.0)
}
T.position(2.0, 3.0, 2.0) {
    DL {
        intensity(0.85)
        C.rgba(1.0, 0.96, 0.92, 1.0)
    }
}
