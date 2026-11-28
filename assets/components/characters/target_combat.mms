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
// mitten_antagonist's single shots: aim, fire, release, repeating every
// SINGLE_PERIOD_SECONDS. SINGLE_RATE scales the whole cycle; at 3 a shot
// leaves every ~0.42 s, above the pistol's semi interval (~0.19 s).
let SINGLE_RATE = 3.0
let SINGLE_PERIOD_SECONDS = (2.0 / 1.6) / SINGLE_RATE
let SINGLE_AIM_SECONDS = 0.4 / SINGLE_RATE
let SINGLE_HOLD_SECONDS = SEMI_HOLD_SECONDS / SINGLE_RATE
// Singles before switching to an auto burst; scaled so the semi stretch
// lasts as long as before.
let SINGLES_BEFORE_BURST = 5 * SINGLE_RATE

// Animation timers dispatch only at visual time.
export fn after_seconds(events, seconds, name) {
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

// Keyframes own the cadence. Frame updates are only needed to track the
// target during the wind-up before a single shot and during automatic fire.
export fn mitten_antagonist(options) {
    let gun = options.gun
    let state = { phase = "down" aiming = false holding = false singles = 0 last_hit = null }
    let events = T {}
    let first = after_seconds(events, 1.6, "OpeningReady")
    let singles = Animation.paused().length(SINGLE_PERIOD_SECONDS * BEATS_PER_SECOND) {
        Keyframe.at(0.0) { emit_data(events, "SingleAim") }
        Keyframe.at(SINGLE_AIM_SECONDS * BEATS_PER_SECOND) { emit_data(events, "SingleFire") }
        Keyframe.at((SINGLE_AIM_SECONDS + SINGLE_HOLD_SECONDS) * BEATS_PER_SECOND) { emit_data(events, "SingleDone") }
    }
    let burst = after_seconds(events, 3.0, "BurstDone")
    let cooldown = after_seconds(events, 5.0, "BurstReady")
    let retry = after_seconds(events, 0.25, "BurstReady")
    let reaction = after_seconds(events, 0.6, "ReactionReady")
    let timers = T {
        name = options.name + "_combat_timers"
        events first singles burst cooldown retry reaction
    }

    fn allowed() {
        return (!options["player_alive"] || options.player_alive()) &&
            (!options["can_attack"] || options.can_attack())
    }
    fn set_aiming(active) {
        if state.aiming == active { return }
        state.aiming = active
        options.aiming(active)
    }
    fn release() {
        if !state.holding { return }
        gun.fire_end()
        state.holding = false
    }
    fn pull() {
        gun.fire_start()
        state.holding = true
    }
    fn begin_burst() {
        if !allowed() {
            state.phase = "wait"
            retry.play()
            return
        }
        state.phase = "burst"
        set_aiming(true)
    }
    fn advance(dt) {
        if allowed() {
            options.aim(8.0 * dt)
            if state.phase == "burst" && !state.holding {
                pull()
                burst.play()
            }
            return
        }
        set_aiming(false)
        if state.phase == "burst" {
            release()
            burst.pause()
            state.phase = "wait"
            retry.play()
        }
    }

    on(events, "DataEvent", fn(event) {
        if event == "ReactionReady" && state.phase == "reaction" {
            set_aiming(false)
            state.phase = "single"
            singles.loop_anim()
        } else if event == "OpeningReady" && state.phase == "opening" {
            state.phase = "single"
            singles.loop_anim()
        } else if event == "SingleAim" && state.phase == "single" {
            set_aiming(allowed())
        } else if event == "SingleFire" && state.phase == "single" {
            let ready = state.aiming && allowed()
            set_aiming(false)
            if ready { pull() }
        } else if event == "SingleDone" && state.phase == "single" {
            release()
            if state.singles >= SINGLES_BEFORE_BURST {
                singles.pause()
                gun.set_mode("auto")
                begin_burst()
            }
        } else if event == "BurstDone" && state.phase == "burst" {
            release()
            set_aiming(false)
            state.phase = "wait"
            cooldown.play()
        } else if event == "BurstReady" && state.phase == "wait" {
            begin_burst()
        }
    })
    fn stop() {
        state.phase = "down"
        release()
        set_aiming(false)
        first.pause()
        singles.pause()
        burst.pause()
        cooldown.pause()
        retry.pause()
        reaction.pause()
    }
    fn start() {
        stop()
        state.phase = "opening"
        state.singles = 0
        state.last_hit = null
        gun.set_mode("semi")
        first.play()
    }
    fn shot(pose, mode) {
        if mode == "semi" && state.phase == "single" { state.singles = state.singles + 1 }
        state.last_hit = options.shot_lands(pose)
        if !state.last_hit || !options["on_player_hit"] { return }
        if options["player_alive"] && !options.player_alive() { return }
        let points = SEMI_POINTS
        if mode == "auto" { points = AUTO_POINTS }
        options.on_player_hit(points)
    }
    fn attacked() {
        if state.phase == "down" || state.phase == "reaction" || !allowed() { return }
        stop()
        state.phase = "reaction"
        state.singles = 0
        gun.set_mode("semi")
        set_aiming(true)
        reaction.play()
    }
    return { root = timers advance = advance start = start stop = stop shot = shot attacked = attacked
        last_hit = fn() { return state.last_hit } }
}
