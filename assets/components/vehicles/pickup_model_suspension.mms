import { pickup_ride_transition } from "./pickup_ride_height.mms"

// Authored GLB names after turning its +Z front toward vehicle-local -Z.
export fn pickup_model_suspension_bindings() {
    return [
        { corner="fl" spring="suspension_spring" wheel="front-left-WHEEL" ratio=1.0 actuator=null },
        { corner="fr" spring="suspension_spring.002" wheel="front-left-WHEEL.003" ratio=1.0 actuator=null },
        { corner="rl" spring="suspension_spring.001" wheel="front-left-WHEEL.001" ratio=1.0 actuator=null },
        { corner="rr" spring="suspension_spring.003" wheel="front-left-WHEEL.002" ratio=1.0 actuator=null },
    ]
}

fn bind_wheel(chassis, spring, wheel, corner) {
    let hub=wheel.translation()
    let spin=T { name="wheel_"+corner }
    let steering=T { name="steering_"+corner spin }
    let endpoint=T.position(hub[0],hub[1],hub[2]) {
        name="suspension_endpoint_"+corner
        TOP.fork_trs().drop("scale") { steering }
    }
    let spring_control=T {
        name="spring_"+corner
        pickup_ride_transition()
        endpoint
    }
    let mount=T {
        name="suspension_mount_"+corner
        spring_control
    }
    // Put the authored pose on the fixed mount, leaving a clean compression
    // transform without discarding the spring's orientation relative to the body.
    mount.trs(spring.trs())
    chassis.attach(mount)
    spring_control.attach(spring)
    spring.update_transform([0,0,0],[0,0,0],[1,1,1])
    spin.attach(wheel)
    wheel.update_transform([0,0,0])
    mount.set_compression=fn(ratio) {
        let compression=Math.clamp(ratio,0.35,1.35)
        spring_control.update_transform([0,0,0],[0,0,0],[1,compression,1])
    }
    // Hydraulic UI travel is normalized to a 0.45528 m reference. Convert it
    // to this model's authored spring length so equal lift keeps wheels planted
    // even when a Blender revision moves the spring origins or hub offsets.
    mount.set_ride_compression=fn(ratio) {
        let length=Math.abs(hub[1])
        if length<0.001 {length=0.001}
        let scale=1.0+(ratio-1.0)*0.45528/length
        mount.set_compression(scale)
    }
    return mount
}

export fn bind_pickup_model_suspension(model, bindings) {
    let chassis=model.query("#truck")
    if !chassis { print("Pickup GLB: truck node is missing") return }
    for binding in bindings {
        let spring=model.query("[name='"+binding.spring+"']")
        let wheel=model.query("[name='"+binding.wheel+"']")
        if spring && wheel {
            binding.actuator=bind_wheel(chassis,spring,wheel,binding.corner)
            if binding.raw==false {binding.actuator.set_ride_compression(binding.ratio)}
            else {binding.actuator.set_compression(binding.ratio)}
        } else { print("Pickup GLB: missing suspension nodes for "+binding.corner) }
    }
}
