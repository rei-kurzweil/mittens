// assets/components/button.mms
//
// Pushable button component. ʕ•ᴥ•ʔ
//
// Usage (once MMS import is wired to assets/):
//   import { button } from "components/button.mms"
//   emit(button)
//
// Structure:
//   T (root, LayoutRoot + Style — outer shell)
//   └── T (button_face, Style + Raycastable — the pushable surface)
//       ├── T.position(0, 0, 0.1)   — label floats at +z above face
//       │   └── Text { "Button" }
//       └── Animation (press animation, nested child of root T)
//
// The button internally handles DragStart/DragEnd to animate the face.
// External callers can additionally listen for Click on the returned root handle.
// Layout controls x/y sizing; z is authored independently (layout ignores z).

// ─────────────────────────────────────────────────
// ( ˘ω˘ ) Component tree
// ─────────────────────────────────────────────────

// TODO [MMS missing: export eval block]
// Ideal form once MMS supports script-level export values:
//   export let button = { ... ; root }
// For now this file emits the tree at top level.

T {
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

    // Press animation — translates button_face along -z (into the surface)
    // Targets "button_face" by name using the #name selector.
    //
    // TODO [naming scope]: Action.update_transform uses world-wide name lookup today.
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

    // ─────────────────────────────────────────────────
    // (｡•́︿•̀｡) Signal handlers — NOT YET SUPPORTED in MMS
    // ─────────────────────────────────────────────────
    //
    // What needs to exist in MMS before this works:
    //   1. HostCallKind::RegisterHandler { scope, signal_kind, handler_fn }
    //   2. Evaluator syntax: on(component_handle, "SignalKind", fn(event) { ... })
    //   3. Method dispatch on ComponentObject: anim.play(), anim.pause(), anim.reverse()
    //   4. Query HostCall to resolve child handle by name: root->"button_press_anim"
    //      (MMQ: query child T#button_press_anim or Animation#button_press_anim)
    //
    // Intended implementation once those exist:
    //
    //   let face = root->"button_face"         // MMQ subtree query
    //   let anim = root->"button_press_anim"   // MMQ subtree query
    //
    //   on(face, "DragStart", fn(event) {
    //       anim.play()       // animate forward: face pushes in
    //   })
    //
    //   on(face, "DragEnd", fn(event) {
    //       anim.reverse()    // animate back: face springs out
    //   })
    //
    //   // Click event is emitted by GestureSystem on DragEnd within the 8px/0.02wu threshold.
    //   // External callers register a second Click handler on the root handle:
    //   //   on(button_instance, "Click", fn(e) { ... })
    //   // Both handlers fire independently — internal animation + external response.
}

// ─────────────────────────────────────────────────
// ʕ •ᴥ•ʔ What works today vs what's blocked
// ─────────────────────────────────────────────────
//
// WORKS NOW:
//   - T / LayoutRoot / Style component tree syntax
//   - Raycastable.enabled() on button face
//   - Text { "..." } label
//   - T.position(0, 0, 0.1) for z-offset label above face
//   - Animation.paused() with Keyframe.at(beat) / Action.update_transform
//   - name = "..." property for animation action targeting
//   - File loading via eval_file("assets/components/button.mms")
//
// BLOCKED (per docs/meow_meow/task/mms-reply-channel-objectworld-and-mmq-status.md):
//   - Signal handler registration from MMS script
//     → needs HostCallKind::RegisterHandler + on(...) syntax
//   - Method dispatch on spawned handles (anim.play(), anim.reverse())
//     → needs ComponentHandle shape + method dispatch HostCall
//   - Subtree query to resolve child handles by name/type
//     → needs HostCallKind::Query + MMQ parser (section 7–8 of task doc)
//   - export let from script level
//     → needs evaluator to expose the final emitted handle to the caller
//   - Multiple instances (naming scope conflict on "#button_face")
//     → needs instance-unique names or scoped name resolution
//
// POINTER EVENT REFRESHER (relevant to external callers):
//   EventSignal::Click { raycaster, renderable, hit_point, screen_pos_px }
//     → emitted by GestureSystem when DragEnd is within 8px screen / 0.02wu of DragStart
//     → NOT emitted as PointerDown/PointerUp — the engine uses drag primitives only
//     → no PointerEnter / PointerExit exist today
//   EventSignal::DragStart / DragMove / DragEnd
//     → finer-grained if the caller needs press-and-hold vs click distinction
//   External Rust registration (works today):
//     rx.add_scoped_handler_closure(scope_id, button_root_id, SignalKind::Click,
//         |world, emit, env| { ... });
