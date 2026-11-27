import { sized_mini_explosion } from "./mini-explosion.mms"

fn noise(value) {
    let raw = Math.sin(value * 12.9898 + 78.233) * 43758.5453
    return raw - Math.floor(raw)
}

// Stay where the player went down, even if they move to the avatar picker.
export fn player_death(position, beats_per_second) {
    let events = T {}
    let state = { next = 0 }
    let seed = position[0] * 19.0 + position[1] * 43.0 + position[2] * 31.0
    let duration = 6.0 * beats_per_second
    let schedule = Animation.paused().length(duration) {
        for index in range(24) {
            Keyframe.at(index * duration / 24.0) { emit_data(events, "Burst") }
        }
        Keyframe.at(duration) { emit_data(events, "Finish") }
    }
    let sequence = T { name = "arena_player_death" events schedule }
    on(events, "DataEvent", fn(event) {
        if event == "Burst" {
            let step = seed + state.next * 3.0
            state.next = state.next + 1
            let angle = 2.0 * Math.pi * noise(step)
            let radius = 0.24 + 0.72 * Math.sqrt(noise(step + 1.0))
            let height = (noise(step + 2.0) - 0.5) * 1.65
            // 1x to 15x the impact size, 5x on average: 1 + 14 * u^2.5.
            let u = noise(step + 1.5)
            let size = 1.0 + 14.0 * u * u * Math.sqrt(u)
            let explosion = sized_mini_explosion([
                position[0] + Math.cos(angle) * radius,
                position[1] + height,
                position[2] + Math.sin(angle) * radius
            ], beats_per_second, size)
            explosion.root
            explosion.animation.play()
        }
        if event == "Finish" { sequence.remove_subtree() }
    })
    sequence
    schedule.play()
}
