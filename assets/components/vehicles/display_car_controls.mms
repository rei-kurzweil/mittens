import { laser } from "../laser.mms"
import { muzzle_flash } from "../muzzle-flash.mms"
import { mini_explosion } from "../mini-explosion.mms"
import { burst_rocket_weapon_controller, burst_rocket_fire_button } from "../weapons/burst_rocket_weapon_controller.mms"
import { pickup_power_panel } from "./pickup_power_panel.mms"

// Input and heading callbacks exist only while powered and occupied. Native
// force/velocity/contact integration continues while off or dismounted.
// Two weapons share one elevation slider on the dashboard: the nose laser on
// the trigger (grip + trigger in XR, Space on desktop) fires two shots, and
// the dashboard button (R on desktop) fires a five-rocket burst, 300 ms apart
// with a 1.5 s cooldown, from two nose muzzles. A scene can route their hits
// into its own rules by setting body.on_laser_hit(hit) and body.on_rocket_hit(hit).
let LASER_SHOT_SECONDS = 0.12
// The handheld pistol's semi-auto clip at half the pistol's level (0.6).
let LASER_GAIN = 0.3
let ELEVATION_MIN_DEG = -10.0
let ELEVATION_MAX_DEG = 35.0
// The reticle: a white glowing wireframe cube on the guns' pivot, this far
// ahead of the nose along the aim.
let RETICLE_DISTANCE = 12.0
let RETICLE_HALF = 0.35
let RETICLE_EDGE = 0.025

// Twelve thin glowing edges of a cube with half-size h.
fn wireframe_cube(h, edge) {
    return T {
        Raycastable.disabled()
        Selectable.off()
        for axis in [0, 1, 2] {
            for a in [-1.0, 1.0] {
                for b in [-1.0, 1.0] {
                    let position = [0.0, 0.0, 0.0]
                    let size = [edge, edge, edge]
                    if axis == 0 { position = [0.0, a * h, b * h] size = [2.0 * h, edge, edge] }
                    if axis == 1 { position = [a * h, 0.0, b * h] size = [edge, 2.0 * h, edge] }
                    if axis == 2 { position = [a * h, b * h, 0.0] size = [edge, edge, 2.0 * h] }
                    T.position(position[0], position[1], position[2]).scale(size[0], size[1], size[2]) {
                        R.cube() {
                            C.rgba(1.0, 1.0, 1.0, 1.0)
                            Material(MaterialDefinition.toon())
                            EM.on().intensity(1.5)
                        }
                    }
                }
            }
        }
    }
}
export fn display_car_controls(body, motion, input_mode, input_source, seated) {
    let state = { powered=false mounted=seated==true user=null axis=[0,0]
        w=false a=false s=false d=false grip=false }
    let bindings = { active=false handlers=[] }
    let weapon = laser({ name="car_laser" beam_name="laser_beam_glow" length=40.0 beam_radius=0.014 })
    let flash = muzzle_flash({ name="car_laser" size=0.90 })
    let sound = AudioClip.wav("assets/audio/tool/laser_machine_pistol_A.0.wav") {
        name="car_laser_sound" AudioGain.new(LASER_GAIN)
    }
    // The guns' pivot: the slider turns it, carrying the aim reticle ahead.
    let reticle = T.position(0.0, 0.0, -RETICLE_DISTANCE) {
        name="car_aim_reticle"
        wireframe_cube(RETICLE_HALF, RETICLE_EDGE)
    }
    let laser_origin = T { name="car_laser_origin" flash.root AudioOutput { sound } reticle }
    fn hit_target(hit) {
        Damage.at(hit.hit_point[0], hit.hit_point[1], hit.hit_point[2], 0.28, 1.0)
        let impact = mini_explosion(hit.hit_point, 2.0)
        impact.root
        impact.animation.play()
        let on_hit = hit.renderable.on_hit
        if on_hit != null { on_hit(hit) }
        if body.on_laser_hit != null { body.on_laser_hit(hit) }
    }
    let left_muzzle = T { name="car_rocket_muzzle_left" }
    let right_muzzle = T { name="car_rocket_muzzle_right" }
    let origin = T { name="car_rocket_origin" left_muzzle right_muzzle }
    let model = body.query("#car_model")
    let placement={ready=false nose=[0,0,0] elevation=0.0}
    // Both guns pitch up or down together from the nose.
    fn aim_guns() {
        if !placement.ready {return}
        laser_origin.update_transform(placement.nose, [placement.elevation,0,0], [1,1,1])
        origin.update_transform(placement.nose, [placement.elevation,0,0], [1,1,1])
    }
    // Just ahead of the nose at 56% of its height: the laser at the centre,
    // rockets a third of the width out to each side, clear of the hull.
    fn place_muzzle() {
        if placement.ready {return true}
        let bounds = model.local_bounds()
        if !bounds { return false }
        let low = bounds["min"] let high = bounds["max"]
        let nose = [(low[0]+high[0])*0.5, low[1]+(high[1]-low[1])*0.56, low[2]-0.10]
        placement.nose=nose
        laser_origin.update_transform(nose, [placement.elevation,0,0], [1,1,1])
        origin.update_transform(nose, [placement.elevation,0,0], [1,1,1])
        let side = (high[0]-low[0])*0.30
        left_muzzle.update_transform([-side,0,0], [0,0,0], [1,1,1])
        right_muzzle.update_transform([side,0,0], [0,0,0], [1,1,1])
        placement.ready=true
        return true
    }
    let rockets = burst_rocket_weapon_controller({
        name="car_rockets" muzzles=[left_muzzle, right_muzzle]
        count=5 interval_seconds=0.3 cooldown_seconds=1.5
        source_tool=body user=fn() { return state.user }
        can_fire=fn() { return state.powered && state.mounted && place_muzzle() }
        on_hit=fn(hit) { if body.on_rocket_hit != null { body.on_rocket_hit(hit) } }
    })
    fn laser_shot() {
        if !state.powered || !state.mounted {return}
        flash.play()
        weapon.launch({ pose=laser_origin.world_trs() source_tool=body source_user=state.user on_hit=hit_target })
    }
    // Two shots per trigger pull. Keyframes play the sound (audio time) and
    // emit the shots, which fire once at visual time, like the pistol.
    let laser_events = T { name="car_laser_events" }
    let laser_pair = Animation.paused().length(LASER_SHOT_SECONDS * 2.0 + 0.1) {
        name="car_laser_pair"
        Keyframe.at(0.0) { sound.play() }
        Keyframe.at(0.0) { emit_data(laser_events, "LaserShot") }
        Keyframe.at(LASER_SHOT_SECONDS * 2.0) { sound.play() }
        Keyframe.at(LASER_SHOT_SECONDS * 2.0) { emit_data(laser_events, "LaserShot") }
    }
    on(laser_events, "DataEvent", fn(event) { if event == "LaserShot" { laser_shot() } })
    fn fire() {
        if !state.powered || !state.mounted {return}
        if !place_muzzle() {return}
        laser_pair.play()
    }
    fn reset_inputs() {
        state.axis=[0,0] state.w=false state.a=false state.s=false state.d=false state.grip=false
    }
    fn publish() {
        if !state.powered || !state.mounted { return }
        place_muzzle()
        let steering=state.axis[0] let throttle=state.axis[1]
        if input_mode=="desktop" {
            steering=0 throttle=0
            if state.a {steering=steering-1} if state.d {steering=steering+1}
            if state.w {throttle=throttle+1} if state.s {throttle=throttle-1}
        }
        if Math.abs(steering)<0.16 {steering=0}
        if Math.abs(throttle)<0.16 {throttle=0}
        let vertical=motion.linear()[1]
        motion.reset()
        motion.translate([0,vertical,-throttle*5.0])
        motion.rotate([0,-steering*1.25,0])
    }
    fn sync_controls() {
        if !state.powered || !state.mounted {
            for binding in bindings.handlers {binding.remove()}
            bindings.handlers=[] bindings.active=false
            return
        }
        if bindings.active {return}
        bindings.active=true
        if input_mode=="desktop" {
            let down=on(world,"KeyDown",fn(event) {
                if !state.powered || !state.mounted {return}
                if event.code=="Space" {fire()}
                // Desktop shortcut for the dashboard's rocket button.
                if event.code=="KeyR" {rockets.fire_burst()}
                if event.code=="KeyW" {state.w=true} if event.code=="KeyA" {state.a=true}
                if event.code=="KeyS" {state.s=true} if event.code=="KeyD" {state.d=true}
                publish()
            })
            let up=on(world,"KeyUp",fn(event) {
                if !state.powered || !state.mounted {return}
                if event.code=="KeyW" {state.w=false} if event.code=="KeyA" {state.a=false}
                if event.code=="KeyS" {state.s=false} if event.code=="KeyD" {state.d=false}
                publish()
            })
            bindings.handlers=[down,up]
        } else {
            let axis=on(input_source,"XrAxisChanged",fn(event) {
                if !state.powered || !state.mounted {return}
                if event.control=="LeftStick" {state.axis=event.value publish()}
            })
            let down=on(input_source,"XrButtonDown",fn(event) {
                if !state.powered || !state.mounted {return}
                if event.control=="RightGrip" {state.grip=true}
                if event.control=="RightTrigger" && state.grip {fire()}
            })
            let up=on(input_source,"XrButtonUp",fn(event) {
                if !state.powered || !state.mounted {return}
                if event.control=="RightGrip" {state.grip=false}
            })
            bindings.handlers=[axis,down,up]
        }
        let frames=on(world,"FrameTick",fn(event) {publish()})
        if input_mode=="desktop" {bindings.handlers=[bindings.handlers[0],bindings.handlers[1],frames]}
        else {bindings.handlers=[bindings.handlers[0],bindings.handlers[1],bindings.handlers[2],frames]}
    }
    // Place in the open cockpit view above the dashboard. The old truck-local
    // offset put this below the view and behind dashboard triangles.
    let panel=pickup_power_panel(fn() { body.set_powered(!state.powered) }, {
        position=[0.30,4.20,-1.90] scale=1.5
    })
    let panel_mount=T { name="car_power_panel_mount" panel }
    let fire_button=burst_rocket_fire_button("car_rockets", fn() { rockets.fire_burst() }, {
        position=[-0.30,4.20,-1.90] scale=1.5
    })
    // Gun elevation.
    let elevation_track = T.scale(0.30, 0.02, 0.05) {
        R.cube() { C.rgba(0.20,0.28,0.25,1.0) Raycastable.enabled() { interaction_priority(180.0) } }
    }
    let elevation_thumb = T.scale(0.035, 0.07, 0.07) {
        R.sphere() { C.rgba(0.95,0.78,0.40,1.0) Raycastable.enabled() { interaction_priority(180.0) } }
    }
    let elevation_slider = Slider.range(ELEVATION_MIN_DEG, ELEVATION_MAX_DEG).step(1.0).value(0.0).width(0.27)
        .track(elevation_track).thumb(elevation_thumb) { name="car_gun_elevation_slider" }
    let elevation_readout = Text { name="car_gun_elevation_readout" "AIM 0" }
    on(elevation_slider, "SliderChanged", fn(event) {
        placement.elevation = event.value * Math.pi / 180.0
        elevation_readout.set_text("AIM " + event.value)
        aim_guns()
    })
    // Off to the right of the dashboard, vertical: up raises the guns.
    let elevation_panel=T.position(0.95,4.05,-1.75).scale(1.5,1.5,1.5) {
        name="car_gun_elevation"
        Grabbable.off()
        Selectable.off()
        T.rotation(0.0,0.0,Math.pi/2.0) { elevation_slider }
        T.position(0.04,-0.20,0.0).scale(0.015,0.015,0.015) { elevation_readout }
    }
    body.powered=fn() {return state.powered}
    body.set_powered=fn(value) {
        state.powered=value==true
        reset_inputs() sync_controls() panel.show_power(state.powered)
    }
    // Only the driver's seat takes the controls, not riders boarding the
    // deck: listen on the seat itself when the car names one.
    let seat_events = body
    if body.seat != null { seat_events = body.seat }
    on(seat_events,"MountStarted",fn(event) {
        state.mounted=true state.user=event.rider reset_inputs() sync_controls()
    })
    on(seat_events,"MountEnded",fn(event) {
        state.mounted=false state.user=null reset_inputs() sync_controls()
    })
    sync_controls()
    return T { name="car_driver_controls" laser_origin laser_events laser_pair origin rockets panel_mount fire_button elevation_panel }
}
