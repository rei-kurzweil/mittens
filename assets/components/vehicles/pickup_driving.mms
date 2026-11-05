// Input and heading refreshes publish commands to existing native Velocity.
// This removable asset registers no global callback or MMS pose integration.
export fn pickup_driving(truck,motion,driver_eye) {
    if driver_eye==null { driver_eye=[-0.48,1.90,-0.20] }
    let entry_frame=T.position(-1.2,1.05,-0.25) { name="pickup_entry_frame" }
    let entry=Zone.cube([0.85,1.05,1.0]).at(entry_frame).role("vehicle_entry") { name="pickup_entry_zone" }
    let seat=T.position(driver_eye[0],driver_eye[1],driver_eye[2]) { name="pickup_driver_eye_anchor" }
    let exit=T.position(-1.95,1.65,-0.25) { name="pickup_dismount_anchor" }
    let mountable=Mountable.entry_zone(entry).mount_anchor(seat).dismount_anchor(exit).on_grip() {}
    let state={mounted=false axis=[0,0] w=false a=false s=false d=false brake=false last_throttle=0 last_steering=0 last_brake=false}
    fn publish(refresh) {
        let throttle=state.axis[1] let steering=state.axis[0]
        if state.w { throttle=1 } if state.s { throttle=-1 }
        if state.a { steering=-1 } if state.d { steering=1 }
        if Math.abs(throttle)<0.16 { throttle=0 }
        if Math.abs(steering)<0.16 { steering=0 }
        if !refresh && throttle==state.last_throttle && steering==state.last_steering && state.brake==state.last_brake {return}
        state.last_throttle=throttle state.last_steering=steering state.last_brake=state.brake
        motion.reset()
        if !state.brake {
            motion.translate([0,0,-throttle*5.0])
            motion.rotate([0,-steering*throttle*1.2,0])
        }
    }
    fn reset() {
        state.axis=[0,0] state.w=false state.a=false state.s=false state.d=false state.brake=false
        state.last_throttle=0 state.last_steering=0 state.last_brake=false
        motion.reset()
    }
    on(mountable,"MountStarted",fn(event) { state.mounted=true reset() })
    on(mountable,"MountEnded",fn(event) { state.mounted=false reset() })
    // Called only by the persistent ship input controller while this truck lives.
    truck.ship_input=fn(kind,event) {
        if !state.mounted {return}
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
        if state.mounted && !state.brake && state.last_throttle!=0 && state.last_steering!=0 {publish(true)}
    }
    truck.drive_command=fn(throttle,steering,braking) {
        state.axis=[Math.clamp(steering,-1,1),Math.clamp(throttle,-1,1)] state.brake=braking publish(false)
    }
    truck.drive_status=fn() {return {mounted=state.mounted linear=motion.linear() angular=motion.angular()}}
    truck.attach(mountable)
    return T {name="pickup_driver_controls" entry_frame entry seat exit}
}
