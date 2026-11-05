import { ship_vehicle_motion } from "./ship_vehicle_motion.mms"
import { pickup_driving } from "./pickup_driving.mms"
import { pickup_model_suspension_bindings, bind_pickup_model_suspension } from "./pickup_model_suspension.mms"
import { pickup_model_colliders } from "./pickup_model_colliders.mms"
import { matcap_metal } from "../materials/matcap_metal.mms"
import { pickup_dashboard_gauges } from "./pickup_dashboard_gauges.mms"
import { pickup_sliding_colliders } from "./pickup_sliding_colliders.mms"
import { pickup_ride_height } from "./pickup_ride_height.mms"
import { pickup_hydraulics_panel } from "./pickup_hydraulics_panel.mms"

// Original procedural geometry remains in pickup_truck_prototype.mms.
fn make_pickup(color, cargo, suspension_layout) {
    let bindings=pickup_model_suspension_bindings()
    let batch=CombineMesh.keep_transforms() {
        name="pickup_painted_body"
        if color!=null { C.rgba(color[0],color[1],color[2],1) }
    }
    let model=GLTF.new("assets/models/mitten_pickup.glb") {
        name="pickup_model"
        matcap_metal()
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
    let body=T.position(0.047,0,-0.568) {
        name="pickup_chassis_frame"
        T.position(0,0.180146,0).rotation(0,Math.pi,0) {
            name="pickup_model_frame"
            model
        }
        // Sources sit just ahead of the authored headlight faces, pointing -Z.
        for side in [-1,1] {
            T.position(side*0.49,1.09,-1.57).rotation(0,Math.pi,0) {
                name="pickup_headlight_"+side
                SL.color(1.0,0.93,0.82).intensity(12.0).distance(25.0).angle(0.50).penumbra(0.40)
            }
        }
        // Rough placement behind the steering wheel on the new dashboard.
        T.position(0.0,0.02,0.30) { pickup_dashboard_gauges() }
        T.position(0,0.64,1.50) {
            name="pickup_bed_cargo_mount"
            if cargo!=null { cargo }
        }
    }
    let pose=T.position(-0.047,0,0.568) {name="pickup_chassis_pose" body}
    let truck=T {
        name="pickup_truck"
        // Group marker retained; mesh-cut damage currently supports cubes only.
        Destroyable.threshold(3.0)
        pose
        // Rounded movement zones and static proxies remain an upright collision
        // approximation while the visible chassis is raised or tilted.
        pickup_model_colliders()
    }
    fn apply_compression(corner,ratio) {
        for binding in bindings {
            if corner==null || corner==binding.corner {
                binding.ratio=Math.clamp(ratio,0.35,1.35)
                if binding.actuator { binding.actuator.set_compression(binding.ratio) }
            }
        }
    }
    let ride=pickup_ride_height(bindings,pose,apply_compression)
    truck.set_suspension_compression=fn(corner,ratio) {ride.clear_pose() apply_compression(corner,ratio)}
    truck.adjust_ride_height=ride.adjust
    truck.reset_ride_height=ride.reset
    truck.ride_height_status=ride.status
    body.attach(pickup_hydraulics_panel(ride))
    let motion=ship_vehicle_motion(truck,"pickup_velocity")
    truck.attach(pickup_sliding_colliders(truck))
    body.attach(pickup_driving(truck,motion,[-0.33,1.78,0.30]))
    return motion
}

export fn pickup_truck(color, cargo) {
    return make_pickup(color,cargo,"fork")
}

export fn pickup_truck_sibling_suspension(color, cargo) {
    return make_pickup(color,cargo,"siblings")
}
