import { ui_unlit_material } from "./materials/ui_unlit.mms"
import { emissive_surface } from "./materials/emissive_surface.mms"

// The loose pickup's glow: on each face of a thin grey slab, a centre disc,
// a ring around it and an outer ring light in turn, each pulsing between a
// dim dark blue and a bright pale blue (a saturated blue at high emissive
// intensity blooms pale). [inner, outer] radii in metres; the disc has none.
let PULSE_SECONDS = 1.8
let PULSE_LOW = 0.35
let PULSE_HIGH = 7.0
let GLOW_BLUE = [0.16, 0.42, 1.0, 1.0]
let RINGS = [[0.030, 0.044], [0.054, 0.068]]
let DISC_RADIUS = 0.022

// A wearable with 100 integrity. Four seconds without a hit starts recovery
// in 20-point steps, once per second. Zero integrity opens the bullet path.
// options (optional): { worn = true } for a shield worn from the start (an
// NPC's), which skips building the loose pickup only to remove it.
export fn energy_shield(name, beats_per_second, options) {
    let state = { integrity = 100.0 worn = false opponent = false visual = null proxy = null pickup = null grab = null }
    let material = ui_unlit_material()
    // The wearer mount is the transform above the item. Keep both the flash
    // and the hit proxy on that pose even if the item's own grip pose is stale.
    let worn_geometry = T {}
    let root = T {
        name = name
        Selectable.off()
        material
        TOP.fork_trs().drop("rotation").sample_ancestor("rotation", 1)
            .sample_ancestor("translation", 1) { worn_geometry }
    }

    fn sync_collision() {
        let blocking = state.worn && state.opponent && state.integrity > 0.0
        if blocking && !state.proxy {
            let ray_box = R.cube() { Opacity.opacity(0.0).multiple_layers() Raycastable.enabled() }
            ray_box.energy_shield = root
            let proxy = T.scale(0.96, 1.92, 0.96) { name = name + "_bullet_proxy" ray_box }
            worn_geometry.attach(proxy)
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
            worn_geometry.attach(shell)
            state.visual = shell
        }
        visibility.play()
        return true
    }
    let step_beats = PULSE_SECONDS * beats_per_second / 3.0
    fn glow() {
        return EM.on().intensity(PULSE_LOW) {
            Transition { duration_beats(step_beats) ease_in_out_sine() replace_same_target() }
        }
    }
    fn loose_pickup() {
        let grab = Grabbable.to_hand(true) {}
        // glows[step] holds both faces' emitters for the disc (0), the middle
        // ring (1) and the outer ring (2).
        let glows = [[glow(), glow()], [glow(), glow()], [glow(), glow()]]
        fn light(step) {
            for index in range(3) {
                let intensity = PULSE_LOW
                if index == step { intensity = PULSE_HIGH }
                for emitter in glows[index] { emitter.set_intensity(intensity) }
            }
        }
        let pulse = Animation.looping().length(step_beats * 3.0) {
            Keyframe.at(0.0) { light(0) }
            Keyframe.at(step_beats) { light(1) }
            Keyframe.at(step_beats * 2.0) { light(2) }
        }
        let pickup = T {
            name = name + "_device"
            T.scale(0.16, 0.16, 0.035) { R.cube() { C.rgba(0.40, 0.41, 0.44, 1.0) Raycastable.enabled() } }
            for face in range(2) {
                // The back face's glow is turned to face outward too.
                let side = 1.0 - face * 2.0
                T.position(0.0, 0.0, side * 0.0185).rotation(0.0, face * Math.pi, 0.0) {
                    Raycastable.disabled()
                    T.scale(DISC_RADIUS * 2.0, DISC_RADIUS * 2.0, 1.0) {
                        R.circle_2d() { emissive_surface() C.rgba(GLOW_BLUE[0], GLOW_BLUE[1], GLOW_BLUE[2], 1.0) glows[0][face] }
                    }
                    for ring in range(2) {
                        R.partial_annulus_2d(RINGS[ring][0], RINGS[ring][1], 0.0, 6.2831853, 48) {
                            emissive_surface() C.rgba(GLOW_BLUE[0], GLOW_BLUE[1], GLOW_BLUE[2], 1.0) glows[ring + 1][face]
                        }
                    }
                }
            }
            PL.color(0.45, 0.70, 1.0).intensity(0.8).distance(2.5)
            pulse
        }
        root.attach(grab)
        root.attach(pickup)
        pulse.play()
        state.grab = grab
        state.pickup = pickup
    }
    if options && options["worn"] == true { state.worn = true } else { loose_pickup() }
    root.set_worn = fn(worn, opponent) {
        if worn != state.worn {
            if worn {
                if state.grab { state.grab.remove_subtree() }
                if state.pickup { state.pickup.remove_subtree() }
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
