import { hold_rider_walking } from "./seat_rider_walking.mms"
// Input and heading refreshes publish commands to existing native Velocity.
// This removable asset registers no global callback or MMS pose integration.
export fn pickup_driving(truck,motion,driver_eye,power_changed,mount_changed,acceleration) {
    if driver_eye==null { driver_eye=[-0.48,1.90,-0.20] }
    let entry_frame=T.position(-1.2,1.05,-0.25) { name="pickup_entry_frame" }
    let entry=Zone.cube([0.85,1.05,1.0]).at(entry_frame).role("vehicle_entry") { name="pickup_entry_zone" }
    let seat=T.position(driver_eye[0],driver_eye[1],driver_eye[2]) { name="pickup_driver_eye_anchor" }
    let exit=T.position(-1.95,1.65,-0.25) { name="pickup_dismount_anchor" }
    let attachment={mountable=null}
    let state={powered=false mounted=false axis=[0,0] w=false a=false s=false d=false brake=false last_throttle=0 last_steering=0 last_brake=false}
    fn publish(refresh) {
        let throttle=state.axis[1] let steering=state.axis[0]
        if state.w { throttle=1 } if state.s { throttle=-1 }
        if state.a { steering=-1 } if state.d { steering=1 }
        if Math.abs(throttle)<0.16 { throttle=0 }
        if Math.abs(steering)<0.16 { steering=0 }
        if !refresh && throttle==state.last_throttle && steering==state.last_steering && state.brake==state.last_brake {return}
        state.last_throttle=throttle state.last_steering=steering state.last_brake=state.brake
        // The stick goes through the truck's common vehicle input.
        truck.set_control_input({throttle=throttle steering=steering brake=state.brake})
    }
    // Clear input latches without changing existing physical velocity.
    fn reset_inputs() {
        if acceleration {truck.set_control_input({})}
        state.axis=[0,0] state.w=false state.a=false state.s=false state.d=false state.brake=false
        state.last_throttle=0 state.last_steering=0 state.last_brake=false
    }
    truck.powered=fn() {return state.powered}
    truck.set_powered=fn(value) {
        let next=value==true
        if next==state.powered {return}
        state.powered=next
        reset_inputs()
        let controller=query("#ship_input_controller")
        if controller {
            if state.powered && state.mounted {controller.set_driver(truck)}
            else {controller.clear_driver(truck)}
        }
        if power_changed!=null {power_changed(state.powered)}
    }
    fn stop_driving() {
        state.mounted=false
        reset_inputs()
        let controller=query("#ship_input_controller")
        if controller {controller.clear_driver(truck)}
    }
    fn install_mountable() {
        let mountable=Mountable.entry_zone(entry).mount_anchor(seat).dismount_anchor(exit).on_grip() {}
        hold_rider_walking(mountable)
        attachment.mountable=mountable
        on(mountable,"MountStarted",fn(event) {
            state.mounted=true
            reset_inputs()
            let controller=query("#ship_input_controller")
            if controller && state.powered {controller.set_driver(truck)}
            if mount_changed {mount_changed(true)}
        })
        on(mountable,"MountEnded",fn(event) {
            stop_driving()
            if mount_changed {mount_changed(false)}
        })
        truck.attach(mountable)
    }
    truck.set_remote_occupied=fn(occupied) {
        if occupied && attachment.mountable {
            // Removing the seat registration lets native attachment cleanup
            // dismount a losing rider; its MountEnded restores their walking.
            if state.mounted {stop_driving()}
            attachment.mountable.remove_subtree()
            attachment.mountable=null
        }
        if !occupied && !attachment.mountable {install_mountable()}
    }
    install_mountable()
    // Called only by the persistent ship input controller while this truck lives.
    truck.ship_input=fn(kind,event) {
        if !state.powered || !state.mounted {return}
        if kind=="XrAxisChanged" && event.control=="LeftStick" {state.axis=event.value publish(false)}
        if kind=="KeyDown" || kind=="KeyUp" {
            let pressed=kind=="KeyDown"
            if event.code=="KeyW" {state.w=pressed} if event.code=="KeyA" {state.a=pressed}
            if event.code=="KeyS" {state.s=pressed} if event.code=="KeyD" {state.d=pressed}
            if event.code=="Space" {state.brake=pressed}
            publish(false)
        }
    }
    truck.refresh_heading=fn() {
        if acceleration {return}
        if state.powered && state.mounted && !state.brake && state.last_throttle!=0 && state.last_steering!=0 {publish(true)}
    }
    truck.needs_heading_refresh=acceleration==null
    truck.drive_command=fn(throttle,steering,braking) {
        if !state.powered {return}
        state.axis=[Math.clamp(steering,-1,1),Math.clamp(throttle,-1,1)] state.brake=braking publish(false)
    }
    truck.drive_status=fn() {return {powered=state.powered mounted=state.mounted linear=motion.linear() angular=motion.angular()}}
    return T {name="pickup_driver_controls" entry_frame entry seat exit}
}
