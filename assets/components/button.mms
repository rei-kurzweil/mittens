// assets/components/button.mms — pushable button factory ʕ•ᴥ•ʔ
//
// Usage:
//   import { button } from "../assets/components/button.mms"
//   let btn = button("Click me")
//   let accent_btn = button("Accent", {
//       background_color = [0.18, 0.48, 0.88, 1.0]
//       color = [0.98, 0.98, 0.98, 1.0]
//   })
//   T.position(x, y, z) { btn }
//   on(btn, "Click", fn(e) { ... })
//
// Style + Raycastable form: layout-managed inline-block box with bg, and the
// click surface is the bg quad. The layout system grafts a Raycastable onto
// the generated __bg renderable when the author T carries Raycastable.enabled();
// see `sync_bg_author_raycastable` in src/engine/ecs/system/layout/block.rs.
//
// v1 scope: visual + Raycastable only. Caller registers Click handlers on
// the returned root. Internal press animation is deferred — needs
// action-target subtree scoping (see
// docs/task/action-target-scoping-and-factory-handlers.md).

export fn button(label, options) {
    let background_color = [0.88, 0.18, 0.18, 1.0]
    let color = [0.98, 0.98, 0.98, 1.0]

    if options {
        if options.background_color {
            background_color = options.background_color
        }
        if options.color {
            color = options.color
        }
    }

    let root = T {
        name = "button_root"
        Raycastable.enabled()
        Style {
            display("inline-block")
            padding_xy(0.6, 0.6)
            text_align("center")
            vertical_align("middle")
            background_color = background_color
            color = color
        }
        T.position(0.0, 0.0, 0.0) {
            Text { label }
        }
    }
    return root
}
