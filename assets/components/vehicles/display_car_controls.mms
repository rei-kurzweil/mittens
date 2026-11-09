import { laser } from "../laser.mms"
import { muzzle_flash } from "../muzzle-flash.mms"
import { mini_explosion } from "../mini-explosion.mms"
import { pickup_power_panel } from "./pickup_power_panel.mms"

// Input and heading callbacks exist only while powered and occupied. Native
// force/velocity/contact integration continues while off or dismounted.
export fn display_car_controls(body, motion, input_mode, input_source, seated) {
    let state = { powered=false mounted=seated==true user=null axis=[0,0]
        w=false a=false s=false d=false grip=false }
    let bindings = { active=false handlers=[] }
    let weapon = laser({ name="car_laser" beam_name="laser_beam_glow" length=40.0 beam_radius=0.014 })
    let flash = muzzle_flash({ name="car_laser" size=0.90 })
    let sound = AudioClip.wav("assets/audio/tool/laser_machine_pistol_C.0.wav")
    let origin = T { name="car_laser_origin" flash.root AudioOutput { sound } }
    let model = body.query("#car_model")
    fn hit_target(hit) {
        Damage.at(hit.hit_point[0], hit.hit_point[1], hit.hit_point[2], 0.28, 1.0)
        let impact = mini_explosion(hit.hit_point, 2.0)
        impact.root
        impact.animation.play()
        let on_hit = hit.renderable.on_hit
        if on_hit != null { on_hit(hit) }
    }
    let placement={ready=false}
    fn place_muzzle() {
        if placement.ready {return true}
        let bounds = model.local_bounds()
        if !bounds { return false }
        let low = bounds["min"] let high = bounds["max"]
        origin.update_transform([(low[0]+high[0])*0.5, low[1]+(high[1]-low[1])*0.56, low[2]-0.10],
            [0,0,0], [1,1,1])
        placement.ready=true
        return true
    }
    fn fire() {
        if !state.powered || !state.mounted {return}
        if !place_muzzle() {return}
        sound.play() flash.play()
        weapon.launch({ pose=origin.world_trs() source_tool=body source_user=state.user on_hit=hit_target })
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
    let panel=pickup_power_panel(fn() { body.set_powered(!state.powered) })
    // Reuse the truck switch at the mech cockpit's height, facing the seat.
    let panel_mount=T.position(0,1.6,-1.1) { panel }
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
    return T { name="car_driver_controls" origin panel_mount }
}
