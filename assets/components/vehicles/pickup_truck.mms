import { pickup_power_panel } from "./pickup_power_panel.mms"
import { ship_vehicle_motion } from "./ship_vehicle_motion.mms"
import { pickup_driving } from "./pickup_driving.mms"
import { pickup_passenger_seat, pickup_bed_deck } from "./pickup_ride_seats.mms"
import { pickup_model_suspension_bindings, bind_pickup_model_suspension } from "./pickup_model_suspension.mms"
import { pickup_model_colliders } from "./pickup_model_colliders.mms"
import { pickup_anime_material } from "../materials/pickup_anime.mms"
import { pickup_dashboard_gauges } from "./pickup_dashboard_gauges.mms"
import { pickup_sliding_colliders } from "./pickup_sliding_colliders.mms"
import { pickup_ride_height, pickup_ride_transition } from "./pickup_ride_height.mms"
import { pickup_hydraulics_panel } from "./pickup_hydraulics_panel.mms"
import { pickup_replication } from "../network/pickup_replication.mms"
import { attach_drops } from "../network/shared_mounts.mms"
import { pickup_doors } from "./pickup_doors.mms"

// Original procedural geometry remains in pickup_truck_prototype.mms.
fn make_pickup(color, cargo, gravity_enabled, options) {
    let damage_threshold = 16.0
    if options && options["damage_threshold"] != null {
        damage_threshold = options.damage_threshold
    }
    let gauges=pickup_dashboard_gauges()
    let vehicle_id="pickup"
    if options && options["vehicle_id"] {vehicle_id=options.vehicle_id}
    let doors=pickup_doors(vehicle_id)
    let network={link=null group=null address=null}
    let bindings=pickup_model_suspension_bindings()
    let drive=null
    if options && options["acceleration_drive"] {
        drive=Acceleration.drive() {name="pickup_drive_acceleration"}
    }
    let batch=CombineMesh.keep_transforms() {
        name="pickup_painted_body"
        // Untextured body: the baked atlas replaces uv0 and paint colour stays
        // an instance multiplier, so every truck shares one bake.
        AmbientOcclusion.texcoord0().proximity_radius(1.20).cavity_radius(0.56).strength(0.34)
    }
    let model=GLTF.new("assets/models/mitten_pickup.glb") {
        name="pickup_model"
        pickup_anime_material()
        Selectable.off()
    }
    on(model,"GLTFInitialized",fn(event) {
        for binding in bindings {
            // Each wheel exports rubber first and the metal rim second.
            let tire=event.gltf.query("[name='"+binding.wheel+"'] > renderable")
            if tire {
                let tread=Texture.from_png("assets/textures/vehicles/tire-tread.png") {}
                tire.attach(tread)
            }
        }
        bind_pickup_model_suspension(event.gltf,bindings,drive)
        let chassis=event.gltf.query("#truck")
        if chassis {
            let cabin=event.gltf.query("#truck > renderable")
            // Combine only stationary geometry. CombineMesh adds no transform,
            // so reparenting these parts retains their authored local poses.
            chassis.attach(batch)
            // The first source supplies the combined mesh's shared paint colour.
            if cabin && color!=null {
                cabin.set_color([color[0],color[1],color[2],1.0])
            }
            for renderable in event.gltf.query_all("#truck > renderable") { batch.attach(renderable) }
            // The doors hang on hinges outside the static batch.
            doors.install(event.gltf,chassis,color)
            if network.group {doors.connect(options.socket,network.group,network.address)}
            for name in ["car_seat","car_seat.passenger_seat","headlights","hood","roof","wheel_wells","truck_bed","steering_wheel"] {
                let part=event.gltf.query("[name='"+name+"']")
                if part { batch.attach(part) }
            }
            // Each closed imported primitive owns its damage state. Keep the
            // open lower cabin intact; wheels and springs remain independent.
            for part_mesh in event.gltf.query_all("renderable") {
                if part_mesh!=cabin {
                    let damage=Destroyable.threshold(damage_threshold) {name="pickup_imported_part_damage"}
                    part_mesh.attach(damage)
                }
            }
            // Physical panel collision is explicit. Decorative coils and tires
            // stay ray-queryable/destroyable without costly moving contact BVHs.
            for panel_name in ["door_left","door_right","roof","truck_bed","wheel_wells"] {
                for panel_mesh in event.gltf.query_all("[name='"+panel_name+"'] > renderable") {
                    let contact=Collidable.static().friction(0.85) {}
                    panel_mesh.attach(contact)
                }
            }
        }
    })
    // Items released over the bed attach to it and ride the truck. Interior
    // of the authored bed in chassis space: floor ~0.6, walls to ~1.36.
    let bed_zone=Zone.cube([0.78,0.45,0.92]).receive_drops(0) {name="pickup_bed_cargo_zone"}
    let bed_cargo=T.position(0,1.05,1.68) {name="pickup_bed_cargo_frame" bed_zone}
    let mounts=null
    let bed_mount=null
    if options && options["mounts"] {mounts=options.mounts}
    if options && options["bed_mount"]!=null {bed_mount=options.bed_mount}
    attach_drops(bed_zone,mounts,bed_mount)
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
        gauges
        T.position(0,0.64,1.50) {
            name="pickup_bed_cargo_mount"
            if cargo!=null { cargo }
        }
        bed_cargo
    }
    let pose=T.position(-0.047,0,0.568) {name="pickup_chassis_pose" pickup_ride_transition() body}
    let truck=T {
        name="pickup_truck"
        // Owns the static batches and the separate procedural dashboard pieces.
        Destroyable.threshold(damage_threshold)
        pose
        // Rounded movement zones and static proxies remain an upright collision
        // approximation while the visible chassis is raised or tilted.
        pickup_model_colliders()
    }
    fn apply_compression(corner,ratio,raw) {
        for binding in bindings {
            if corner==null || corner==binding.corner {
                binding.ratio=Math.clamp(ratio,0.35,1.35)
                binding.raw=raw
                if binding.actuator {
                    if raw {binding.actuator.set_compression(binding.ratio)}
                    else {binding.actuator.set_ride_compression(binding.ratio)}
                }
            }
        }
    }
    let ride=pickup_ride_height(bindings,pose,fn(corner,ratio) {apply_compression(corner,ratio,false)})
    fn can_control() {
        if !network.link {return true}
        return network.link.claim()
    }
    fn adjust(corner,ratio) {if can_control() {ride.adjust(corner,ratio)}}
    fn reset() {if can_control() {ride.reset()}}
    truck.set_suspension_compression=fn(corner,ratio) {ride.clear_pose() apply_compression(corner,ratio,true)}
    truck.adjust_ride_height=adjust
    truck.reset_ride_height=reset
    truck.ride_height_status=ride.status
    let hydraulics=pickup_hydraulics_panel({adjust=adjust reset=reset status=ride.status})
    body.attach(hydraulics)
    let motion=ship_vehicle_motion(truck,"pickup_velocity")
    // A direct child is an existing body-local Velocity gravity override.
    let gravity=Gravity.enabled(gravity_enabled!=false) {name="pickup_gravity"}
    motion.attach(gravity)
    if drive {motion.attach(drive)}
    truck.attach(pickup_sliding_colliders(truck))
    let power=pickup_power_panel(fn() {if can_control() {truck.set_powered(!truck.powered())}})
    fn mount_changed(occupied) {
        if network.link {network.link.mount_changed(occupied)}
    }
    body.attach(pickup_driving(truck,motion,[-0.33,1.78,0.30],fn(value) {power.show_power(value)},mount_changed,drive))
    body.attach(power)
    // The passenger seat and the bed deck sit beside the driver's seat on the
    // truck root; see pickup_ride_seats. The bed's collision context rides the
    // chassis (so it tilts with the hydraulics) and excludes the whole truck.
    let passenger=pickup_passenger_seat()
    body.attach(passenger.frames)
    truck.attach(passenger.mountable)
    let bed=pickup_bed_deck(body, truck)
    body.attach(bed.frames)
    truck.attach(bed.mountable)
    body.attach(bed.context)
    if options && options["socket"] {
        fn follow_peer(following,occupied) {
            truck.set_remote_occupied(occupied)
            if following {
                motion.reset()
                if drive {drive.set_drive(0,0,false)}
            }
            gravity.set_enabled(!following && gravity_enabled!=false)
        }
        fn apply_controls(powered,ratios) {
            truck.set_powered(powered)
            if ride.apply_ratios(ratios) {hydraulics.refresh()}
        }
        let link=pickup_replication({
            socket=options.socket name=options.vehicle_id rank=options.rank target=truck
            powered=truck.powered mounted=fn() {return truck.drive_status().mounted}
            hydraulics=ride.ratios apply_controls=apply_controls follow_peer=follow_peer
        })
        network.link=link
        motion.connect=fn(group,address) {
            network.group=group
            network.address=address
            link.connect(group,address)
            doors.connect(options.socket,group,address)
        }
        motion.disconnect=fn() {
            network.group=null
            network.address=null
            link.disconnect()
            doors.disconnect()
        }
        motion.dispose_replication=fn() {
            network.group=null
            network.address=null
            link.dispose()
            doors.disconnect()
        }
        motion.owns_motion=link.owns_motion
    }
    return motion
}

// For a shared pickup pass { socket, vehicle_id, rank }, then call
// connect(group, address) and disconnect() with the scene's peer lifecycle.
// Pass { mounts, bed_mount } (shared_mounts() and a numeric id unique in the
// scene) to sync items dropped in the bed; without them the bed is local.
// options.damage_threshold overrides the default damage per cell before a cut.
export fn pickup_truck(color, cargo, gravity_enabled, options) {
    return make_pickup(color,cargo,gravity_enabled,options)
}
