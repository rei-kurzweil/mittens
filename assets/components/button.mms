// assets/components/button.mms
//
// Pushable button component. ʕ•ᴥ•ʔ
//
// Loaded via:
//   MeowMeowRunner::eval_file("assets/components/button.mms")
//
// Structure:
//   T (root, LayoutRoot + Style — outer shell)
//   └── T (button_face, Style + Raycastable — the pushable surface)
//       ├── T.position(0, 0, 0.05)        — label floats above face
//       │   └── Text { "Button" }
//       └── Animation (press animation, sibling under root)
//
// Internal handlers animate the face on DragStart/DragEnd. External callers
// can additionally listen for Click on the returned root handle.
// Layout controls x/y sizing; z is authored independently (layout ignores z).

// ─────────────────────────────────────────────────
// ( ˘ω˘ ) Component tree + internal handlers
// ─────────────────────────────────────────────────

let root = T {
    name = "button_root"
    LayoutRoot {}
    Style.display("inline_block")
         .padding_xy(12.0, 6.0)
         .background_color([0.12, 0.12, 0.18, 1.0]) {}

    // Pushable face — receives pointer events via Raycastable
    T {
        name = "button_face"
        Raycastable.enabled()
        Style.display("inline_block")
             .padding_xy(12.0, 6.0)
             .background_color([0.30, 0.45, 0.90, 1.0]) {}

        // Label floats at +z; layout drives x/y, z is ignored by layout
        T.position(0.0, 0.0, 0.05) {
            Text { "Button" }
        }
    }

    // Press animation — translates button_face along -z (into the surface).
    //
    // TODO [naming scope]: Action.update_transform uses world-wide name lookup.
    // Multiple button instances will clash on "#button_face".
    // Fix needed: instance-unique name generation or subtree-scoped name resolution.
    Animation.paused() {
        name = "button_press_anim"

        Keyframe.at(0.0) {
            Action.update_transform("#button_face", [0.0, 0.0,  0.00], [0.0, 0.0, 0.0], [1.0, 1.0, 1.0])
        }
        Keyframe.at(0.08) {
            Action.update_transform("#button_face", [0.0, 0.0, -0.02], [0.0, 0.0, 0.0], [1.0, 1.0, 1.0])
        }
    }
}

// Resolve child handles via subtree query.
let face = root.query("#button_face")
let anim = root.query("#button_press_anim")

// Internal press animation: play forward on DragStart, pause on DragEnd.
// (DragEnd within 8px / 0.02wu also fires Click — external handlers see that.)
//
// TODO [anim.reverse]: only play/pause/loop_anim are dispatched today; no reverse
// intent. For now we just pause on DragEnd — the face stays pressed in.
on(face, "DragStart", fn(event) {
    anim.play()
})

on(face, "DragEnd", fn(event) {
    anim.pause()
})

// ─────────────────────────────────────────────────
// ʕ •ᴥ•ʔ What still doesn't work
// ─────────────────────────────────────────────────
//
// WORKS NOW (was blocked at write-time, now resolved):
//   - HostCallKind::RegisterHandler + on() syntax
//   - Method dispatch on ComponentObject (anim.play, anim.pause, t.set_text)
//   - Subtree Query HostCall: root.query("#button_face")
//   - File loading via eval_file("assets/components/button.mms")
//   - import / export — `import { x } from "f.mms"` is wired
//     (src/meow_meow/evaluator.rs:507). This file does not yet export
//     a handle; turning the body into `export fn button(label) { … }`
//     is the next step (see task doc below).
//
// STILL BLOCKED — both tracked in
//   docs/task/action-target-scoping-and-factory-handlers.md:
//
//   - Multi-instance: Action.update_transform("#button_face") is world-scoped,
//     so two buttons would fight over the same selector. The fix is to
//     store the selector unresolved on the ActionComponent and resolve it
//     at fire-time against the enclosing Animation's parent subtree,
//     using the shared `src/query/` adapter.
//
//   - Factory-function handler registration: `on(...)` inside a
//     `fn button(...) { ... }` body silently no-ops because the
//     evaluator hard-codes channels = None / host_world = None when
//     entering a function call (src/meow_meow/evaluator.rs:891-899).
//     Fix is to forward those from the caller's ctx.
//
//   - anim.reverse() — only play/pause/loop_anim are wired in
//     eval_method_call (separate task).
//
// POINTER EVENT REFRESHER (relevant to external callers):
//   EventSignal::Click { raycaster, renderable, hit_point, screen_pos_px }
//     → emitted by GestureSystem when DragEnd is within 8px screen / 0.02wu of DragStart
//     → NOT emitted as PointerDown/PointerUp — engine uses drag primitives only
//     → no PointerEnter / PointerExit exist today
//   EventSignal::DragStart / DragMove / DragEnd
//     → finer-grained if the caller needs press-and-hold vs click distinction
