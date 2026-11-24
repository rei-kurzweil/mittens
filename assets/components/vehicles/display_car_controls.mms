import { burst_rocket_weapon_controller, burst_rocket_fire_button } from "../weapons/burst_rocket_weapon_controller.mms"
import { pickup_power_panel } from "./pickup_power_panel.mms"

// Input and heading callbacks exist only while powered and occupied. Native
// force/velocity/contact integration continues while off or dismounted.
// Two nose muzzles fire a three-rocket burst from the dashboard button
// (or Space on desktop).
export fn display_car_controls(body, motion, input_mode, input_source, seated) {
    let state = { powered=false mounted=seated==true user=null axis=[0,0]
        w=false a=false s=false d=false }
    let bindings = { active=false handlers=[] }
    let left_muzzle = T { name="car_rocket_muzzle_left" }
    let right_muzzle = T { name="car_rocket_muzzle_right" }
    let origin = T { name="car_rocket_origin" left_muzzle right_muzzle }
    let model = body.query("#car_model")
    let placement={ready=false}
    // Just ahead of the nose at 56% of its height, a third of the width
    // out to each side, so rockets leave clear of the hull.
    fn place_muzzle() {
        if placement.ready {return true}
        let bounds = model.local_bounds()
        if !bounds { return false }
        let low = bounds["min"] let high = bounds["max"]
        origin.update_transform([(low[0]+high[0])*0.5, low[1]+(high[1]-low[1])*0.56, low[2]-0.10],
            [0,0,0], [1,1,1])
        let side = (high[0]-low[0])*0.30
        left_muzzle.update_transform([-side,0,0], [0,0,0], [1,1,1])
        right_muzzle.update_transform([side,0,0], [0,0,0], [1,1,1])
        placement.ready=true
        return true
    }
    let rockets = burst_rocket_weapon_controller({
        name="car_rockets" muzzles=[left_muzzle, right_muzzle]
        source_tool=body user=fn() { return state.user }
        can_fire=fn() { return state.powered && state.mounted && place_muzzle() }
    })
    fn reset_inputs() {
        state.axis=[0,0] state.w=false state.a=false state.s=false state.d=false
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
                // Desktop shortcut for the dashboard's rocket button.
                if event.code=="Space" {rockets.fire_burst()}
                if event.code=="KeyW" {state.w=true} if event.code=="KeyA" {state.a=true}
                if event.code=="KeyS" {state.s=true} if event.code=="KeyD" {state.d=true}
                publish()
            })
            let up=on(world,"KeyUp",fn(event) {
                if !state.powered || !state.mounted {return}
                // Desktop shortcut for the dashboard's rocket button.
                if event.code=="Space" {rockets.fire_burst()}
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
            bindings.handlers=[axis]
        }
        let frames=on(world,"FrameTick",fn(event) {publish()})
        if input_mode=="desktop" {bindings.handlers=[bindings.handlers[0],bindings.handlers[1],frames]}
        else {bindings.handlers=[bindings.handlers[0],frames]}
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
    body.powered=fn() {return state.powered}
    body.set_powered=fn(value) {
        state.powered=value==true
        reset_inputs() sync_controls() panel.show_power(state.powered)
    }
    on(body,"MountStarted",fn(event) {
        state.mounted=true state.user=event.rider reset_inputs() sync_controls()
    })
    on(body,"MountEnded",fn(event) {
        state.mounted=false state.user=null reset_inputs() sync_controls()
    })
    sync_controls()
    return T { name="car_driver_controls" origin rockets panel_mount fire_button }
}
