import { rocket } from "../projectiles/rocket.mms"
import { ui_unlit_material } from "../materials/ui_unlit.mms"

// Fires a burst of rockets from one or more muzzles, alternating between them
// (two muzzles fire left, right, left). The first rocket flies straight along
// its muzzle's -Z; later ones turn by a small pseudo-random pitch and yaw. One
// paused Animation times the burst, so nothing runs per frame between shots,
// and a press during a burst is ignored.
//
// options:
//   name              label prefix
//   muzzles           transforms whose -Z is the firing direction
//   count             rockets per burst (3)
//   interval_seconds  time between rockets (0.4)
//   spread            largest pitch/yaw offset in radians for rockets 2+ (0.05)
//   cooldown_seconds  after the last rocket, before the next burst (0)
//   beats_per_second  the scene Clock's tempo (2.0 at 120 bpm)
//   can_fire()        optional gate checked when a burst starts
//   source_tool, user()  attribution passed to each rocket and its hits
//   on_hit(hit), impact(point, amount, rotation)  as for the STA rocket
//   on_launch(index)  optional, after each rocket leaves
fn noise(value) {
    let raw = Math.sin(value * 12.9898 + 78.233) * 43758.5453
    return raw - Math.floor(raw)
}

fn quat_mul(a, b) {
    return [
        a[3] * b[0] + a[0] * b[3] + a[1] * b[2] - a[2] * b[1],
        a[3] * b[1] - a[0] * b[2] + a[1] * b[3] + a[2] * b[0],
        a[3] * b[2] + a[0] * b[1] - a[1] * b[0] + a[2] * b[3],
        a[3] * b[3] - a[0] * b[0] - a[1] * b[1] - a[2] * b[2],
    ]
}

// Local pitch then yaw: the same order as the practice target's hand aim.
fn pitch_yaw(pitch, yaw) {
    let sa = Math.sin(pitch / 2.0)
    let ca = Math.cos(pitch / 2.0)
    let sb = Math.sin(yaw / 2.0)
    let cb = Math.cos(yaw / 2.0)
    return [sa * cb, ca * sb, sa * sb, ca * cb]
}

export fn burst_rocket_weapon_controller(options) {
    let name = options.name
    let muzzles = options.muzzles
    let count = 3
    if options["count"] != null { count = options.count }
    let interval = 0.4
    if options["interval_seconds"] != null { interval = options.interval_seconds }
    let spread = 0.05
    if options["spread"] != null { spread = options.spread }
    let bps = 2.0
    if options["beats_per_second"] != null { bps = options.beats_per_second }
    let state = { busy = false bursts = 0 shot = 0 }

    // Keyframes run after the loop that built them, so each one takes the
    // next shot from a counter rather than capturing its loop index.
    fn launch_next() {
        let index = state.shot
        state.shot = state.shot + 1
        let muzzle = muzzles[index % len(muzzles)]
        let pose = muzzle.world_trs()
        let rotation = pose.rotation_quat_xyzw
        if index > 0 {
            let seed = state.bursts * 7.0 + index * 2.0
            let pitch = (noise(seed) * 2.0 - 1.0) * spread
            let yaw = (noise(seed + 1.0) * 2.0 - 1.0) * spread
            rotation = quat_mul(rotation, pitch_yaw(pitch, yaw))
        }
        let user = null
        if options["user"] != null { user = options.user() }
        rocket({
            pose = { translation = pose.translation rotation_quat_xyzw = rotation scale = [1.0, 1.0, 1.0] }
            source_tool = options["source_tool"] source_user = user
            on_hit = options["on_hit"] impact = options["impact"]
        })
        if options["on_launch"] != null { options.on_launch(index) }
    }

    let cooldown = 0.0
    if options["cooldown_seconds"] != null { cooldown = options.cooldown_seconds }
    let step = interval * bps
    let ready_at = step * (count - 1) + cooldown * bps
    // The ready keyframe sits inside the clip, not on its end, so the burst
    // always frees the trigger again.
    let burst = Animation.paused().length(ready_at + 0.1) {
        name = name + "_burst"
        for index in range(count) {
            Keyframe.at(index * step) { launch_next() }
        }
        Keyframe.at(ready_at) { state.busy = false }
    }

    fn fire_burst() {
        if state.busy { return false }
        if options["can_fire"] != null && !options.can_fire() { return false }
        state.busy = true
        state.bursts = state.bursts + 1
        state.shot = 0
        burst.play()
        return true
    }

    let root = T { name = name + "_controller" burst }
    root.fire_burst = fire_burst
    root.busy = fn() { return state.busy }
    return root
}

// A dashboard push button that fires the burst; reachable without mounting,
// like the vehicle power panel, but the controller's can_fire still gates it.
export fn burst_rocket_fire_button(name, fire, options) {
    if options == null { options = {} }
    let position = [0.0, 0.0, 0.0]
    let size = 1.0
    if options["position"] != null { position = options.position }
    if options["scale"] != null { size = options.scale }
    let readout = Text { name = name + "_fire_label" "ROCKETS x3" }
    let button = T.position(position[0], position[1], position[2]).scale(size, size, size) {
        name = name + "_fire_button"
        ui_unlit_material()
        Grabbable.off()
        Selectable.off()
        T.scale(0.20, 0.09, 0.03) {
            R.cube() {
                name = name + "_fire_hit_target"
                C.rgba(0.46, 0.16, 0.18, 1)
                Raycastable.click_only() { interaction_priority(180) }
            }
        }
        T.position(-0.077, -0.009, 0.021).scale(0.015, 0.015, 0.015) { readout }
    }
    on(button, "Click", fn(event) { fire() })
    return button
}
