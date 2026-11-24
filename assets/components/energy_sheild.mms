import { ui_unlit_material } from "./materials/ui_unlit.mms"

// A wearable with 100 integrity. Four seconds without a hit starts recovery
// in 20-point steps, once per second. Zero integrity opens the bullet path.
export fn energy_shield(name, beats_per_second) {
    let state = { integrity = 100.0 worn = false opponent = false visual = null proxy = null pickup = null grab = null }
    let material = ui_unlit_material()
    let root = T { name = name Selectable.off() material }

    fn sync_collision() {
        let blocking = state.worn && state.opponent && state.integrity > 0.0
        if blocking && !state.proxy {
            let ray_box = R.cube() { Opacity.opacity(0.0).multiple_layers() Raycastable.enabled() }
            ray_box.energy_shield = root
            let proxy = T.scale(0.96, 1.92, 0.96) { name = name + "_bullet_proxy" ray_box }
            root.attach(proxy)
            state.proxy = proxy
        }
        if !blocking && state.proxy {
            state.proxy.remove_subtree()
            state.proxy = null
        }
    }
    fn hide_shell() {
        if !state.visual { return }
        state.visual.remove_subtree()
        state.visual = null
    }
    let hide_at = 2.5 * beats_per_second
    let events = T {}
    let visibility = Animation.paused().length(hide_at) {
        Keyframe.at(0.0) {}
        Keyframe.at(hide_at) { emit_data(events, "HideShell") }
    }
    fn recharge() {
        state.integrity = Math.clamp(state.integrity + 20.0, 0.0, 100.0)
        sync_collision()
    }
    let recovery = Animation.paused().length(8.0 * beats_per_second) {
        Keyframe.at(0.0) {}
        for seconds in range(5) {
            Keyframe.at((4.0 + seconds) * beats_per_second) { emit_data(events, "Recharge") }
        }
    }
    on(events, "DataEvent", fn(event) {
        if event == "HideShell" { hide_shell() }
        if event == "Recharge" { recharge() }
    })
    root.attach(events)
    root.attach(visibility)
    root.attach(recovery)
    root.take_damage = fn(amount) {
        if !state.worn || amount <= 0.0 { return false }
        state.integrity = Math.clamp(state.integrity - amount, 0.0, 100.0)
        sync_collision()
        recovery.play()
        // A broken shield shows no shell, even mid-flash.
        if state.integrity <= 0.0 {
            hide_shell()
            return true
        }
        if !state.visual {
            let shell = R.capsule_y(0.48, 0.48) {
                name = name + "_shell"
                C.rgba(1.0, 1.0, 1.0, 1.0)
                Texture.with_uri("assets/textures/energy-shield-voronoi.png")
                TextureFiltering.linear()
                Opacity.opacity(0.4)
                Raycastable.disabled()
            }
            root.attach(shell)
            state.visual = shell
        }
        visibility.play()
        return true
    }
    fn loose_pickup() {
        let grab = Grabbable.to_hand(true) {}
        let pickup = T.scale(0.22, 0.13, 0.17) {
            R.cube() { C.rgba(0.30, 0.78, 1.0, 1.0) Raycastable.enabled() }
        }
        root.attach(grab)
        root.attach(pickup)
        state.grab = grab
        state.pickup = pickup
    }
    loose_pickup()
    root.set_worn = fn(worn, opponent) {
        if worn != state.worn {
            if worn {
                state.grab.remove_subtree()
                state.pickup.remove_subtree()
                state.grab = null
                state.pickup = null
            } else {
                hide_shell()
                loose_pickup()
            }
        }
        state.worn = worn
        state.opponent = opponent
        sync_collision()
    }
    root.integrity = fn() { return state.integrity }
    return root
}
