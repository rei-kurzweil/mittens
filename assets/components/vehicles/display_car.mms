// Reusable display-car vehicle fixture.
//
// The car's GLTF and its spatial mounting affordances belong together. The
// entry zone overlaps the lower front of the imported car while extending
// beyond local -Z for an approaching rider. Local -Z is this asset's semantic
// front.

import { bisket_anime_shading } from "../materials/bisket_anime_shading.mms"

export fn display_car(root_name, position, yaw, mount_anchor_name, extra_children) {
    let entry_zone_frame = T.position(0.0, 1.0, -1.4) {
        name = "car_entry_zone_frame"
    }
    let entry_zone = Zone.cube([4.6, 1.4, 1.4]).at(entry_zone_frame).role("vehicle_entry") {
        name = "car_entry_zone"
    }
    let mount_anchor = T.position(0.0, 4.5, -1.0) {
        name = mount_anchor_name
    }
    let dismount_anchor = T.position(0.0, 2.4, -4.6) {
        name = "car_dismount"
    }
    let mountable = Mountable
        .entry_zone(entry_zone)
        .mount_anchor(mount_anchor)
        .dismount_anchor(dismount_anchor)
        .on_grip() {}

    return T.position(position[0], position[1], position[2]).rotation(0.0, yaw, 0.0) {
        name = root_name
        mountable
        entry_zone_frame
        entry_zone
        mount_anchor
        dismount_anchor

        T {
            name = "car_model"
            GLTF.new("assets/models/car.glb") {
                bisket_anime_shading()
            }
        }

        for child in extra_children { child }
    }
}


// V1 mounted-car behavior. The spatial factory above intentionally remains
// usable without controls; these wrappers add the device-specific command
// adapter and the car-local drive/laser behavior.
fn display_car_with_controls(root_name, position, yaw, mount_anchor_name, input_mode, input_source) {
    let vehicle_state = {
        mounted = false
        movement = [0.0, 0.0]
        w_held = false
        a_held = false
        s_held = false
        d_held = false
        right_grip_held = false
        position = [position[0], position[1], position[2]]
        yaw = yaw
    }
    let car_drive_speed = 5.0
    let car_turn_speed = 1.25
    let car_stick_deadzone = 0.16

    let laser_placement = { ready = false }
    let laser_length = 40.0
    let muzzle_clearance = 0.10
    let muzzle_height_fraction = 0.56

    let muzzle_flash_0_emissive = Emissive.off()
    let muzzle_flash_1_emissive = Emissive.off()
    let laser_outer_emissive = Emissive.off()
    let laser_middle_emissive = Emissive.off()
    let laser_core_emissive = Emissive.off()

    let muzzle_flash_0 = T.position(0.0, 0.0, 0.0).scale(0.0, 0.0, 0.0) {
        name = "car_laser_muzzle_flash_0"
        R.square() {
            C.rgba(1.0, 1.0, 1.0, 1.0)
            Texture.with_uri("assets/images/flash_red_0.png")
            TextureFiltering.linear()
            Opacity.opacity(0.99)
            muzzle_flash_0_emissive
        }
    }
    let muzzle_flash_1 = T.position(0.0, 0.0, 0.002).scale(0.0, 0.0, 0.0) {
        name = "car_laser_muzzle_flash_1"
        R.square() {
            C.rgba(1.0, 1.0, 1.0, 1.0)
            Texture.with_uri("assets/images/flash_red_1.png")
            TextureFiltering.linear()
            Opacity.opacity(0.99)
            muzzle_flash_1_emissive
        }
    }
    let muzzle_flash = T.position(0.0, 0.0, 0.0).rotation(0.0, 3.14159, 0.0) {
        name = "car_laser_muzzle_flash"
        muzzle_flash_0
        muzzle_flash_1
    }
    let laser_beam_glow = T.position(0.0, 0.0, 0.0)
        .rotation(-1.5708, 0.0, 0.0).scale(0.0, 0.0, 0.0) {
        name = "laser_beam_glow"
        T.scale(0.16, laser_length, 1.0) {
            R.square() {
                C.rgba(1.0, 0.06, 0.03, 1.0)
                Opacity.opacity(0.18)
                laser_outer_emissive
            }
        }
        T.position(0.0, 0.0, 0.002).scale(0.08, laser_length, 1.0) {
            R.square() {
                C.rgba(1.0, 0.22, 0.08, 1.0)
                Opacity.opacity(0.38)
                laser_middle_emissive
            }
        }
        T.position(0.0, 0.0, 0.004).scale(0.028, laser_length, 1.0) {
            R.square() {
                C.rgba(1.0, 0.88, 0.58, 1.0)
                Opacity.opacity(0.88)
                laser_core_emissive
            }
        }
    }

    let laser_shot = Animation.paused().length(0.22) {
        Keyframe.at(0.0) {
            muzzle_flash.update_transform(
                [0.0, 0.0, 0.0], [0.0, 3.14159, 0.0], [1.0, 1.0, 1.0]
            )
            muzzle_flash_0.update_transform(
                [0.0, 0.0, 0.0], [0.0, 0.0, 0.0], [0.90, 0.90, 0.90]
            )
            muzzle_flash_1.update_transform(
                [0.0, 0.0, 0.002], [0.0, 0.0, 0.0], [0.0, 0.0, 0.0]
            )
            laser_beam_glow.update_transform(
                [0.0, 0.0, -laser_length * 0.5], [-1.5708, 0.0, 0.0], [1.0, 1.0, 1.0]
            )
            muzzle_flash_0_emissive.set_intensity(8.0)
            muzzle_flash_1_emissive.off()
            laser_outer_emissive.set_intensity(3.0)
            laser_middle_emissive.set_intensity(6.0)
            laser_core_emissive.set_intensity(12.0)
        }
        Keyframe.at(0.05) {
            muzzle_flash_0.update_transform(
                [0.0, 0.0, 0.0], [0.0, 0.0, 0.0], [0.0, 0.0, 0.0]
            )
            muzzle_flash_1.update_transform(
                [0.0, 0.0, 0.002], [0.0, 0.0, 0.0], [0.90, 0.90, 0.90]
            )
            muzzle_flash_0_emissive.off()
            muzzle_flash_1_emissive.set_intensity(8.0)
        }
        Keyframe.at(0.10) {
            muzzle_flash_0.update_transform(
                [0.0, 0.0, 0.0], [0.0, 0.0, 0.0], [0.0, 0.0, 0.0]
            )
            muzzle_flash_1.update_transform(
                [0.0, 0.0, 0.002], [0.0, 0.0, 0.0], [0.0, 0.0, 0.0]
            )
            laser_beam_glow.update_transform(
                [0.0, 0.0, -laser_length * 0.5], [-1.5708, 0.0, 0.0], [0.0, 0.0, 0.0]
            )
            muzzle_flash_0_emissive.off()
            muzzle_flash_1_emissive.off()
            laser_outer_emissive.off()
            laser_middle_emissive.off()
            laser_core_emissive.off()
        }
    }

    fn fire_laser() {
        if laser_placement.ready { laser_shot.play() }
    }

    let laser_origin = T {
        name = "car_laser_origin"
        muzzle_flash
        laser_beam_glow
    }
    let car_root = display_car(
        root_name,
        position,
        yaw,
        mount_anchor_name,
        [laser_origin, laser_shot],
    )

    on(car_root, "MountStarted", fn(event) {
        vehicle_state.mounted = true
        vehicle_state.movement = [0.0, 0.0]
        vehicle_state.w_held = false
        vehicle_state.a_held = false
        vehicle_state.s_held = false
        vehicle_state.d_held = false
        vehicle_state.right_grip_held = false
    })
    on(car_root, "MountEnded", fn(event) {
        vehicle_state.mounted = false
        vehicle_state.movement = [0.0, 0.0]
        vehicle_state.w_held = false
        vehicle_state.a_held = false
        vehicle_state.s_held = false
        vehicle_state.d_held = false
        vehicle_state.right_grip_held = false
    })

    if input_mode == "desktop" {
        on_global("KeyDown", fn(event) {
            if event.code == "Space" {
                if vehicle_state.mounted { fire_laser() }
            } else if event.code == "KeyW" {
                vehicle_state.w_held = true
            } else if event.code == "KeyA" {
                vehicle_state.a_held = true
            } else if event.code == "KeyS" {
                vehicle_state.s_held = true
            } else if event.code == "KeyD" {
                vehicle_state.d_held = true
            }
        })
        on_global("KeyUp", fn(event) {
            if event.code == "KeyW" {
                vehicle_state.w_held = false
            } else if event.code == "KeyA" {
                vehicle_state.a_held = false
            } else if event.code == "KeyS" {
                vehicle_state.s_held = false
            } else if event.code == "KeyD" {
                vehicle_state.d_held = false
            }
        })
    } else {
        on(input_source, "XrAxisChanged", fn(event) {
            if event.control == "LeftStick" {
                vehicle_state.movement = event.value
            }
        })
        on(input_source, "XrButtonDown", fn(event) {
            if event.control == "RightGrip" {
                vehicle_state.right_grip_held = true
            } else if event.control == "RightTrigger" {
                if vehicle_state.mounted && vehicle_state.right_grip_held {
                    fire_laser()
                }
            }
        })
        on(input_source, "XrButtonUp", fn(event) {
            if event.control == "RightGrip" {
                vehicle_state.right_grip_held = false
            }
        })
    }

    let car_model = car_root.query("#car_model")
    let muzzle_origin = car_root.query("#car_laser_origin")
    on_global("FrameTick", fn(event) {
        if !laser_placement.ready {
            let model_box = car_model.local_bounds()
            if model_box {
                let x = (model_box["min"][0] + model_box["max"][0]) * 0.5
                let y = model_box["min"][1] + (model_box["max"][1] - model_box["min"][1]) * muzzle_height_fraction
                let front_z = model_box["min"][2] - muzzle_clearance
                muzzle_origin.update_transform(
                    [x, y, front_z], [0.0, 0.0, 0.0], [1.0, 1.0, 1.0],
                )
                laser_placement.ready = true
            }
        }

        if vehicle_state.mounted {
            let steering = vehicle_state.movement[0]
            let throttle = vehicle_state.movement[1]
            if input_mode == "desktop" {
                steering = 0.0
                throttle = 0.0
                if vehicle_state.a_held { steering = steering - 1.0 }
                if vehicle_state.d_held { steering = steering + 1.0 }
                if vehicle_state.w_held { throttle = throttle + 1.0 }
                if vehicle_state.s_held { throttle = throttle - 1.0 }
            }
            let command_length = Math.sqrt(steering * steering + throttle * throttle)
            let minimum_command = 0.0
            if input_mode == "xr" { minimum_command = car_stick_deadzone }
            if command_length > minimum_command {
                vehicle_state.yaw = vehicle_state.yaw - steering * car_turn_speed * event.dt_sec
                let distance = throttle * car_drive_speed * event.dt_sec
                vehicle_state.position = [
                    vehicle_state.position[0] - Math.sin(vehicle_state.yaw) * distance,
                    position[1],
                    vehicle_state.position[2] - Math.cos(vehicle_state.yaw) * distance,
                ]
                car_root.update_transform(
                    vehicle_state.position,
                    [0.0, vehicle_state.yaw, 0.0],
                    [1.0, 1.0, 1.0],
                )
            }
        }
    })

    return car_root
}

export fn display_car_desktop(root_name, position, yaw, mount_anchor_name) {
    return display_car_with_controls(
        root_name, position, yaw, mount_anchor_name, "desktop", null,
    )
}

export fn display_car_xr(root_name, position, yaw, mount_anchor_name, xr_gamepad) {
    return display_car_with_controls(
        root_name, position, yaw, mount_anchor_name, "xr", xr_gamepad,
    )
}
