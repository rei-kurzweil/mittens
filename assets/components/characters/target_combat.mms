let BEATS_PER_SECOND = 2.0
// The NPC alternates PHASE_SECONDS of semi-auto and of full auto. Each
// cycle it turns to the player (AIM_TURN_SECONDS), then pulls the trigger:
//   semi: one shot every SEMI_PERIOD_SECONDS, SEMI_POINTS if it lands
//   auto: AUTO_HOLD_SECONDS bursts AUTO_WAIT_SECONDS apart, AUTO_POINTS per
//         landed shot at the pistol's auto cadence
// A shot lands if the player is still within SHOT_HALF_WIDTH of the aimed
// line (the NPC does not track during a burst).
let PHASE_SECONDS = 30.0
let AIM_TURN_SECONDS = 0.6
// The turn plus a beat before the trigger.
let AIM_SECONDS = AIM_TURN_SECONDS + 0.1
let SEMI_PERIOD_SECONDS = 5.0
// Semi fires on a keyframe shortly after the press; release after this.
let SEMI_HOLD_SECONDS = 0.3
let SEMI_POINTS = 1.0
let AUTO_HOLD_SECONDS = 1.5
let AUTO_WAIT_SECONDS = 6.0
let AUTO_POINTS = 0.5
// laser_machine_pistol's auto cadence.
let AUTO_SHOT_SECONDS = 1.0 / (8.0 * 1.15)

// Animation timers dispatch only at visual time.
fn after_seconds(events, seconds, name) {
    let beats = seconds * BEATS_PER_SECOND
    return Animation.paused().length(beats) {
        Keyframe.at(0.0) {}
        Keyframe.at(beats) { emit_data(events, name) }
    }
}

export fn practice_combat(options) {
    let gun = options.gun
    // Combat activity state machine, timed by animations (no per-frame
    // script): each activity's timer emits a DataEvent when it ends and the
    // machine enters `next`:
    //   wait -> aim -> fire -> wait ...    (down while the NPC is dead)
    // Entering an activity performs its one action: aim turns to the player,
    // fire presses the trigger, leaving fire releases it. A looping 30 s
    // phase animation flips semi/auto; the flip applies when a wait begins,
    // never mid-burst.
    let combat = { activity = "down" next = "wait" mode = "semi" flip = false }
    let events = T {}
    let timers = { wait = null aim = null fire = null }
    timers.wait = {
        semi = after_seconds(events, SEMI_PERIOD_SECONDS - AIM_SECONDS - SEMI_HOLD_SECONDS, "WaitDone")
        auto = after_seconds(events, AUTO_WAIT_SECONDS, "WaitDone")
    }
    timers.aim = after_seconds(events, AIM_SECONDS, "AimDone")
    timers.fire = { semi = after_seconds(events, SEMI_HOLD_SECONDS, "FireDone") }
    // An auto burst checks each shot at the pistol's cadence.
    let burst_beats = AUTO_HOLD_SECONDS * BEATS_PER_SECOND
    let shot_beats = AUTO_SHOT_SECONDS * BEATS_PER_SECOND
    let burst = Animation.paused().length(burst_beats) {
        Keyframe.at(0.0) {}
        for shot in range(Math.floor(AUTO_HOLD_SECONDS / AUTO_SHOT_SECONDS)) {
            Keyframe.at((shot + 1) * shot_beats) { emit_data(events, "Shot") }
        }
        Keyframe.at(burst_beats) { emit_data(events, "FireDone") }
    }
    timers.fire.auto = burst
    let phase = Animation.looping().length(PHASE_SECONDS * BEATS_PER_SECOND) {
        Keyframe.at(0.0) {}
        // Just inside the loop: a keyframe on a looping clip's very end may
        // never fire.
        Keyframe.at(PHASE_SECONDS * BEATS_PER_SECOND - 0.05) { emit_data(events, "PhaseFlip") }
    }
    let timer_root = T {
        name = options.name + "_combat_timers"
        events timers.wait.semi timers.wait.auto timers.aim timers.fire.semi burst phase
    }
    fn check_shot() {
        if options["can_attack"] && !options.can_attack() { return }
        let points = SEMI_POINTS
        if combat.mode == "auto" { points = AUTO_POINTS }
        if options.shot_lands(null) && options["on_player_hit"] { options.on_player_hit(points) }
    }
    fn stop_timers() {
        timers.wait.semi.pause()
        timers.wait.auto.pause()
        timers.aim.pause()
        timers.fire.semi.pause()
        timers.fire.auto.pause()
    }
    fn enter(activity) {
        if combat.activity == "fire" { gun.fire_end() }
        combat.activity = activity
        if activity == "wait" {
            if combat.flip {
                combat.flip = false
                if combat.mode == "semi" { combat.mode = "auto" } else { combat.mode = "semi" }
                gun.set_mode(combat.mode)
            }
            combat.next = "aim"
            timers.wait[combat.mode].play()
        }
        if activity == "aim" {
            options.aim(null)
            combat.next = "fire"
            timers.aim.play()
        }
        if activity == "fire" {
            gun.fire_start()
            combat.next = "wait"
            // The first shot leaves with the press; auto keyframes check the rest.
            check_shot()
            timers.fire[combat.mode].play()
        }
        if activity == "down" {
            stop_timers()
            combat.next = "wait"
        }
    }
    on(events, "DataEvent", fn(event) {
        if combat.activity == "down" { return }
        if event == "PhaseFlip" { combat.flip = true }
        if event == "Shot" && combat.activity == "fire" { check_shot() }
        if event == "WaitDone" || event == "AimDone" || event == "FireDone" { enter(combat.next) }
    })
    return { root = timer_root start = fn() { enter("wait") } stop = fn() { enter("down") } }
}

// Mitten opens with five single shots at two-second intervals, then holds
// auto for three seconds and rests for five. The pistol owns shot timing.
export fn mitten_antagonist(options) {
    let gun = options.gun
    let state = { elapsed = 0.0 next_shot = 2.0 release_at = null
        automatic = false burst_on = false burst_until = 0.0 holding = false last_hit = null }
    fn release() {
        gun.fire_end()
        state.holding = false
    }
    fn pull() {
        if state.holding { return }
        gun.fire_start()
        state.holding = true
    }
    fn track(dt) {
        options.aim(8.0 * dt)
    }
    let tick = on(world, "FrameTick", fn(event) {
        if (options["player_alive"] && !options.player_alive()) || (options["can_attack"] && !options.can_attack()) {
            if state.holding { release() }
            return
        }
        let dt = event.dt_sec
        state.elapsed = state.elapsed + dt
        if !state.automatic {
            if state.release_at != null && state.elapsed >= state.release_at {
                release()
                state.release_at = null
                if state.next_shot > 10.0 {
                    state.automatic = true
                    state.burst_on = true
                    state.burst_until = state.elapsed + 3.0
                    gun.set_mode("auto")
                }
            } else if state.elapsed >= state.next_shot - 0.4 {
                track(dt)
                if state.elapsed >= state.next_shot && state.release_at == null {
                    pull()
                    state.release_at = state.elapsed + SEMI_HOLD_SECONDS
                    state.next_shot = state.next_shot + 2.0
                }
            }
            if !state.automatic { return }
        }
        if state.elapsed >= state.burst_until {
            state.burst_on = !state.burst_on
            let period = 5.0
            if state.burst_on { period = 3.0 }
            else { release() }
            state.burst_until = state.elapsed + period
        }
        if state.burst_on {
            track(dt)
            pull()
        }
    })
    tick.bypass(true)
    fn start() {
        state.elapsed = 0.0
        state.next_shot = 2.0
        state.release_at = null
        state.automatic = false
        state.last_hit = null
        gun.set_mode("semi")
        tick.bypass(false)
    }
    fn stop() { release() tick.bypass(true) }
    fn shot(pose, mode) {
        state.last_hit = options.shot_lands(pose)
        if !state.last_hit || !options["on_player_hit"] { return }
        if options["player_alive"] && !options.player_alive() { return }
        let points = SEMI_POINTS
        if mode == "auto" { points = AUTO_POINTS }
        options.on_player_hit(points)
    }
    return { root = null start = start stop = stop shot = shot dispose = fn() { stop() tick.remove() }
        last_hit = fn() { return state.last_hit } }
}
