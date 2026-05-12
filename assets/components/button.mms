// assets/components/button.mms — pushable button factory ʕ•ᴥ•ʔ
//
// Usage:
//   import { button } from "../assets/components/button.mms"
//   let btn = button("Click me")
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

export fn button(label) {
    let root = T {
        name = "button_root"
        Raycastable.enabled()
        Style {
            display("inline-block")
            width(4.0)
            height(4.0)
            padding_xy(0.5, 0.5)
            background_color = [0.30, 0.45, 0.90, 1.0]
        }
        T.position(0.0, 0.0, 0.05).scale(0.6, 0.6, 0.6) {
            Text { label }
        }
    }
    return root
}
