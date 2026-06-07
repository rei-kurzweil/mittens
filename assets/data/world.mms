Editor.translation_space("world").rotation_space("local")

Editor.translation_space("world").rotation_space("local") {
    InputXR.on() {
        Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
            AvatarControl.head_bone("J_Bip_C_Head").body_yaw_threshold(0.7853981852531433).body_yaw_rate(3.0).left_hand_bone("J_Bip_L_Hand").right_hand_bone("J_Bip_R_Hand").hand_rotation_smoothing(220.0).camera_bone("J_Bip_C_Head") {
                ControllerXR.new(true, "Left", "Grip") {
                    Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                        Pointer {
                            Raycast.event_driven().max_distance(200.0)
                        }
                    }
                }
                ControllerXR.new(true, "Right", "Grip") {
                    Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                        Pointer {
                            Raycast.event_driven().max_distance(200.0)
                        }
                    }
                }
            }
            overlay {
                Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.05999999865889549, 0.05999999865889549, 0.11999999731779099) {
                    Renderable.cube() {
                        Color.rgba(0.0, 1.0, 1.0, 0.5)
                        Emissive.on()
                        Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                    }
                }
            }
            Transform.position(0.000000010490732726964325, -0.07999999821186066, 0.11999999731779099).rotation_quat(0.0, 1.0, 0.0, -0.00000004371138828673793).scale(1.0, 1.0, 1.0)
        }
    }
}

