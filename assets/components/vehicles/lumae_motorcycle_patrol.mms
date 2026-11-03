import { motorcycle } from "./motorcycle.mms"
import { pose as riding_pose } from "../characters/lumae/poses/motorcycle-seated.pose.mms"

// Kinematic demo on the trailer park's clear central lane. The passenger is a
// child of the motorcycle seat, with no AVC capsule or walking solver to fight
// the authored riding pose. No independent rider position updates are needed.
export fn lumae_motorcycle_patrol() {
    let state = { running = true phase = "waiting" elapsed = 0.0
        x = 3.5 z = -1.0 yaw = Math.pi / 2.0 direction = -1.0
        speed = 0.7 wait_seconds = 2.5 turn_seconds = 2.0 }
    let rider = GLTF("assets/components/characters/lumae/lumae.glb") {
        name = "ship_lumae_motorcycle_rider"
        Material(MaterialDefinition.anime())
        riding_pose()
    }
    let bike = motorcycle([0.52, 0.24, 0.12], rider)
    let root = T.position(state.x, 0.0, state.z).rotation(0.0, state.yaw, 0.0) {
        name = "ship_lumae_motorcycle_patrol"
        bike
    }
    root.status = fn() {
        return { running = state.running phase = state.phase elapsed = state.elapsed
            x = state.x yaw = state.yaw direction = state.direction }
    }
    root.set_running = fn(enabled) { state.running = enabled }
    fn smaller(a, b) { if a < b { return a } return b }
    fn advance(dt) {
        if !state.running || dt <= 0.0 { return }
        // Never jump across the lot after a paused window or a long frame.
        let remaining = smaller(dt, 0.10)
        while remaining > 0.000001 {
            if state.phase == "waiting" {
                let step = smaller(remaining, state.wait_seconds - state.elapsed)
                state.elapsed = state.elapsed + step
                remaining = remaining - step
                if state.elapsed >= state.wait_seconds - 0.000001 {
                    state.elapsed = 0.0
                    state.phase = "driving"
                }
            } else if state.phase == "driving" {
                let target = state.direction * 3.5
                let seconds = Math.abs(target - state.x) / state.speed
                let step = smaller(remaining, seconds)
                state.x = state.x + state.direction * state.speed * step
                remaining = remaining - step
                if seconds <= step + 0.000001 {
                    state.x = target
                    state.elapsed = 0.0
                    state.phase = "turning"
                }
            } else {
                let step = smaller(remaining, state.turn_seconds - state.elapsed)
                let before = state.elapsed / state.turn_seconds
                state.elapsed = state.elapsed + step
                let after = state.elapsed / state.turn_seconds
                // Smoothly integrate exactly half a turn, then pause again.
                let a = before * before * (3.0 - 2.0 * before)
                let b = after * after * (3.0 - 2.0 * after)
                state.yaw = state.yaw + state.direction * Math.pi * (b - a)
                remaining = remaining - step
                if state.elapsed >= state.turn_seconds - 0.000001 {
                    state.elapsed = 0.0
                    state.direction = -state.direction
                    state.phase = "waiting"
                }
            }
        }
        root.update_transform([state.x, 0.0, state.z], [0.0, state.yaw, 0.0], [1.0, 1.0, 1.0])
    }
    // Also exposed for deterministic scene checks and manual preview stepping.
    root.advance = advance
    on_global("FrameTick", fn(event) { advance(event.dt_sec) })
    return root
}
