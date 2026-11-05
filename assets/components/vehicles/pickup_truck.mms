import { ship_vehicle_motion } from "./ship_vehicle_motion.mms"
import { pickup_driving } from "./pickup_driving.mms"
import { pickup_model_suspension_bindings, bind_pickup_model_suspension } from "./pickup_model_suspension.mms"
import { pickup_model_colliders } from "./pickup_model_colliders.mms"
import { pickup_painted_metal } from "../materials/pickup_painted_metal.mms"
import { pickup_dashboard_gauges } from "./pickup_dashboard_gauges.mms"

// Original procedural geometry remains in pickup_truck_prototype.mms.
fn make_pickup(color, cargo, suspension_layout) {
    let bindings=pickup_model_suspension_bindings()
    let batch=CombineMesh.keep_transforms() {
        name="pickup_painted_body"
        if color!=null { C.rgba(color[0],color[1],color[2],1) }
    }
    let model=GLTF.new("assets/models/mitten_pickup.glb") {
        name="pickup_model"
        pickup_painted_metal()
        Texture.with_uri("assets/textures/pool/chrome-matcap.png")
        TextureFiltering.linear()
    }
    on(model,"GLTFInitialized",fn(event) {
        bind_pickup_model_suspension(event.gltf,bindings,suspension_layout)
        let chassis=event.gltf.query("#truck")
        if chassis {
            // Combine only stationary geometry. CombineMesh adds no transform,
            // so reparenting these parts retains their authored local poses.
            chassis.attach(batch)
            for renderable in event.gltf.query_all("#truck > renderable") { batch.attach(renderable) }
            for name in ["car-seat","car-seat.001","door_left","door_right","headlights","Plane.001","steering_wheel"] {
                let part=event.gltf.query("[name='"+name+"']")
                if part { batch.attach(part) }
            }
        }
    })
    let truck=T {
        name="pickup_truck"
        // Group marker retained; mesh-cut damage currently supports cubes only.
        Destroyable.threshold(3.0)
        T.position(0,0.180146,0).rotation(0,Math.pi,0) {
            name="pickup_model_frame"
            model
        }
        pickup_model_colliders()
        // Sources sit just ahead of the authored headlight faces, pointing -Z.
        for side in [-1,1] {
            T.position(side*0.49,1.09,-1.57).rotation(0,Math.pi,0) {
                name="pickup_headlight_"+side
                SL.color(1.0,0.93,0.82).intensity(12.0).distance(25.0).angle(0.50).penumbra(0.40)
            }
        }
        // Rough placement behind the steering wheel on the new dashboard.
        T.position(0.15,0.02,0.30) { pickup_dashboard_gauges() }
        T.position(0,0.64,1.50) {
            name="pickup_bed_cargo_mount"
            if cargo!=null { cargo }
        }
    }
    truck.set_suspension_compression=fn(corner,ratio) {
        for binding in bindings {
            if corner==null || corner==binding.corner {
                binding.ratio=Math.clamp(ratio,0.35,1.35)
                if binding.actuator { binding.actuator.set_compression(binding.ratio) }
            }
        }
    }
    let motion=ship_vehicle_motion(truck,"pickup_velocity")
    truck.attach(pickup_driving(truck,motion,[-0.33,1.78,0.30]))
    return motion
}

export fn pickup_truck(color, cargo) {
    return make_pickup(color,cargo,"fork")
}

export fn pickup_truck_sibling_suspension(color, cargo) {
    return make_pickup(color,cargo,"siblings")
}
