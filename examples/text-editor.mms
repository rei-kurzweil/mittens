// text-editor.mms — scrollable text input editor surface

I {
    speed(1.0)
    InputTransformMode.forward_z() {
        roll_axis_y()
        fps_rotation()
    }

    T.position(0.0, 1.1, 3.8) {
        C3D {}
        Pointer {}
    }
}

let editor_text = "// text-editor.mms\n// A scrollable TextInput demo.\n\nfn main() {\n    let mut buffer = String::new();\n    buffer.push_str(\"line 01: cat engine text editor demo\\n\");\n    buffer.push_str(\"line 02: use this area for longer notes\\n\");\n    buffer.push_str(\"line 03: the container scrolls when content exceeds 80x40 gu\\n\");\n    buffer.push_str(\"line 04: caret movement and editing stay inside the same input\\n\");\n    buffer.push_str(\"line 05: the surface is intentionally tall enough to need scroll\\n\");\n    buffer.push_str(\"line 06: long buffers should stay readable while editing\\n\");\n    buffer.push_str(\"line 07: line wrapping may still happen depending on the text layout\\n\");\n    buffer.push_str(\"line 08: the example keeps the authoring contract simple\\n\");\n    buffer.push_str(\"line 09: scroll to inspect later lines\\n\");\n    buffer.push_str(\"line 10: editing should work normally here\\n\");\n    buffer.push_str(\"line 11: more sample content follows\\n\");\n    buffer.push_str(\"line 12: more sample content follows\\n\");\n    buffer.push_str(\"line 13: more sample content follows\\n\");\n    buffer.push_str(\"line 14: more sample content follows\\n\");\n    buffer.push_str(\"line 15: more sample content follows\\n\");\n    buffer.push_str(\"line 16: more sample content follows\\n\");\n    buffer.push_str(\"line 17: more sample content follows\\n\");\n    buffer.push_str(\"line 18: more sample content follows\\n\");\n    buffer.push_str(\"line 19: more sample content follows\\n\");\n    buffer.push_str(\"line 20: more sample content follows\\n\");\n    println!(\"{}\", buffer);\n}\n"

T.position(-3.2, 2.0, 0.0).scale(0.08, 0.08, 0.08) {
    LayoutRoot {
        name = "text_editor_demo"
        available_width(70.0)
        available_height(50.0)

        T {
            name = "editor_shell"
            Style {
                display("block")
                width(70.0)
                height(50.0)
                padding(1.0)
                overflow("scroll")
                background_color = [1.0, 0.61, 0.61, 0.96]
                color = [0.0, 0.0, 0.0, 1.0]
                font_size(1.35)
                word_wrap("normal")
            }

            T {
                name = "editor_input"
                Style {
                    display("inline-block")
                    width(100%)
                    color = [0.0, 0.0, 0.0, 1.0]
                    font_size(1.35)
                }
                TextInput {
                    editor_text
                }
            }
        }
    }
}

T.position(-2, 3, 2) {
    PL {
        intensity(2.0)
        distance(40.0)
        C.rgba(1.0, 1.0, 1.0, 1.0)
    }
}

AL {
    C.rgba(0.22, 0.22, 0.24, 1.0)
}

BGC {
    C.rgba(0.8, 0.7, 0.7, 1.0)
}