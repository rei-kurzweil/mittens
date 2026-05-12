// assets/components/button.mms — pushable button factory ʕ•ᴥ•ʔ
//
// Usage:
//   import { button } from "../assets/components/button.mms"
//   let btn = button("Click me")
//   T.position(x, y, z) { btn }
//   on(btn, "Click", fn(e) { ... })
//
// v1 scope: visual + Raycastable only. Caller registers Click handlers on
// the returned root. Internal press animation is deferred — needs
// action-target subtree scoping (see
// docs/task/action-target-scoping-and-factory-handlers.md).
//
// Visual: a square face with a text label floating in front.

export fn button(label) {
    let root = T {
        name = "button_root"
        R.square() {
            C.rgba(0.30, 0.45, 0.90, 1.0)
            Raycastable.enabled()
        }
        T.position(0.0, 0.0, 0.05).scale(0.6, 0.6, 0.6) {
            Text { label }
        }
    }
    return root
}
