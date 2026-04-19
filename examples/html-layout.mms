// html-layout.mms — layout system authoring demo ( ˘ω˘ )
//
// Exercises: Layout root, display block/inline/flex, flex-row, flex-column,
//            justify-content, align-items, relative/absolute positioning,
//            overflow:scroll.
//
// All dimensions in glyph units (1 gu = one monospace character cell).
// World scale: T.scale(0.08, 0.08, 0.08) converts gu → world units.
//
// NOTE: LayoutSystem is not yet implemented. Components spawn correctly and
// are queryable, but no UpdateTransform is emitted from layout passes yet.
// Text children will all render at their parent's origin until LayoutSystem
// is wired. This file is the authoring contract and integration smoke-test.

// ── Camera + input ──────────────────────────────────────────────────────────
I {
    InputTransformMode.forward_z() {
        roll_axis_y()
        fps_rotation()
    }
    T.position(0.0, 1.0, 3.5) {
        C3D {}
        Pointer {}
    }
}

// ── Layout root ─────────────────────────────────────────────────────────────
// 60 gu wide × 50 gu tall viewport. Scale 0.08 → 4.8 × 4.0 world units.
T.position(-2.4, 2.0, 0.0).scale(0.08, 0.08, 0.08) {
    LayoutRoot {
        available_width(60.0)
        available_height(50.0)

        // Body — block container, fills the layout root
        HtmlElement.body {

            // ── 1. Display: block ──────────────────────────────────────────
            // div/p/h* are block by default; they stack vertically.
            HtmlElement.section {
                Style {
                    padding(1.0)
                }
                HtmlElement.h2 {
                    Text { "1. Block layout" }
                }
                HtmlElement.p {
                    Text { "Paragraph A stacks below h2 (display: block)." }
                }
                HtmlElement.p {
                    Text { "Paragraph B stacks below A." }
                }
            }

            // ── 2. Display: inline ─────────────────────────────────────────
            // span is inline by default; items flow left-to-right.
            HtmlElement.section {
                Style {
                    padding(1.0)
                }
                HtmlElement.h2 {
                    Text { "2. Inline layout" }
                }
                HtmlElement.p {
                    HtmlElement.span { Text { "word1 " } }
                    HtmlElement.span { Text { "word2 " } }
                    HtmlElement.span { Text { "word3" } }
                }
            }

            // ── 3. Flex row ────────────────────────────────────────────────
            // Three equal columns via flex-grow:1.
            HtmlElement.section {
                Style {
                    padding(1.0)
                }
                HtmlElement.h2 {
                    Text { "3. Flex row — equal columns" }
                }
                HtmlElement.div {
                    Style {
                        display("flex")
                        flex_direction("row")
                        gap(1.0)
                    }
                    HtmlElement.div {
                        Style { flex_grow(1.0) }
                        Text { "[col A]" }
                    }
                    HtmlElement.div {
                        Style { flex_grow(1.0) }
                        Text { "[col B]" }
                    }
                    HtmlElement.div {
                        Style { flex_grow(1.0) }
                        Text { "[col C]" }
                    }
                }
            }

            // ── 4. Flex column ─────────────────────────────────────────────
            HtmlElement.section {
                Style {
                    padding(1.0)
                }
                HtmlElement.h2 {
                    Text { "4. Flex column" }
                }
                HtmlElement.div {
                    Style {
                        display("flex")
                        flex_direction("column")
                        gap(0.5)
                        width(20.0)
                    }
                    HtmlElement.div { Text { "row A" } }
                    HtmlElement.div { Text { "row B" } }
                    HtmlElement.div { Text { "row C" } }
                }
            }

            // ── 5. justify-content + align-items ──────────────────────────
            HtmlElement.section {
                Style {
                    padding(1.0)
                }
                HtmlElement.h2 {
                    Text { "5. justify: space_between, align: center" }
                }
                HtmlElement.div {
                    Style {
                        display("flex")
                        flex_direction("row")
                        justify_content("space_between")
                        align_items("center")
                        height(4.0)
                    }
                    HtmlElement.span { Text { "[left]" } }
                    HtmlElement.span { Text { "[mid]" } }
                    HtmlElement.span { Text { "[right]" } }
                }
            }

            // ── 6. Mixed flex sidebar + main ───────────────────────────────
            // Two-column layout: narrow sidebar + flex-grow main area.
            HtmlElement.section {
                Style {
                    padding(1.0)
                }
                HtmlElement.h2 {
                    Text { "6. Sidebar + main (flex row)" }
                }
                HtmlElement.div {
                    Style {
                        display("flex")
                        flex_direction("row")
                        gap(1.0)
                    }
                    HtmlElement.aside {
                        Style { width(12.0) }
                        HtmlElement.p { Text { "nav A" } }
                        HtmlElement.p { Text { "nav B" } }
                        HtmlElement.p { Text { "nav C" } }
                    }
                    HtmlElement.main {
                        Style { flex_grow(1.0) }
                        HtmlElement.p {
                            Text { "Main content area. flex-grow:1 so it takes remaining width." }
                        }
                    }
                }
            }

            // ── 7. Relative + absolute positioning ─────────────────────────
            // Badge pinned top-right of a relative-positioned container.
            HtmlElement.section {
                Style {
                    padding(1.0)
                }
                HtmlElement.h2 {
                    Text { "7. Absolute positioning" }
                }
                HtmlElement.div {
                    Style {
                        position("relative")
                        height(5.0)
                    }
                    HtmlElement.p {
                        Text { "Container with position:relative and explicit height." }
                    }
                    HtmlElement.div {
                        Style {
                            position("absolute")
                            top(0.0)
                            right(0.0)
                        }
                        Text { "[badge]" }
                    }
                }
            }

            // ── 8. Overflow: scroll ────────────────────────────────────────
            // Scroll container — fixed height, content overflows.
            // Stencil clipping defines correctness; conservative CPU reject can
            // be added later once clip/content bounding volumes exist.
            HtmlElement.section {
                Style {
                    padding(1.0)
                }
                HtmlElement.h2 {
                    Text { "8. Overflow: scroll" }
                }
                HtmlElement.div {
                    Style {
                        overflow("scroll")
                        height(6.0)
                    }
                    for i in range(20) {
                        HtmlElement.div {
                            Text { "scroll item " + i }
                        }
                    }
                }
            }

        }
    }
}

// ── Lighting ─────────────────────────────────────────────────────────────────
AL {
    C.rgba(0.25, 0.25, 0.28, 1.0)
}
T.position(2.0, 3.0, 2.0) {
    DL {
        intensity(0.85)
        C.rgba(1.0, 0.96, 0.92, 1.0)
    }
}
T.position(-1.0, 1.0, 2.0) {
    DL {
        intensity(0.3)
        C.rgba(0.8, 0.85, 1.0, 1.0)
    }
}
