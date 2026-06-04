RendererSettings.window_size(640.0, 480.0)

BackgroundColor {
    Color.rgba(1.0, 0.6499999761581421, 0.75, 1.0)
    overlay {
        Transform.position(0.3499999940395355, 1.0499999523162842, -0.3499999940395355).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.05999999865889549, 0.05999999865889549, 0.05999999865889549) {
            Renderable.cube() {
                Color.rgba(0.949999988079071, 0.20000000298023224, 0.4000000059604645, 1.0)
                Emissive.on()
                Raycastable.enabled()
                Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
            }
        }
    }
}

AmbientLight.rgb(0.18000000715255737, 0.18000000715255737, 0.2199999988079071)

RenderGraph.on() {
    EmissivePass {}
    Bloom.enabled(true).intensity(0.949999988079071).radius_ndc(0.05999999865889549).emissive_scale(1.2000000476837158).half_res(true)
}

Transform.position(0.15000000596046448, -0.44999998807907104, 1.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
    DirectionalLight.intensity(1.100000023841858).color(1.0, 0.9800000190734863, 0.949999988079071)
}

Editor.translation_space("world").rotation_space("local") {
    Raycastable.enabled() {
        name = "editor_auto_raycastable"
        Transform.position(0.0, -1.649999976158142, -0.4000000059604645).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(120.0, 0.10000000149011612, 95.0) {
            Renderable.cube() {
                Color.rgba(0.18000000715255737, 0.18000000715255737, 0.2199999988079071, 1.0)
                Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
            }
        }
    }
    Raycastable.enabled() {
        name = "editor_auto_raycastable"
        Transform.position(0.0, 2.1500000953674316, -19.026203155517578).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(8.800000190734863, 3.5999999046325684, 0.23999999463558197) {
            Renderable.cube() {
                Color.rgba(0.10999999940395355, 0.10000000149011612, 0.14000000059604645, 1.0)
                Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
            }
            TransformGizmo.scale(0.5) {
                name = "editor_transform_gizmo"
                transform_fork_trs {
                    name = "gizmo_pipeline"
                    transform_map_translation {
                        name = "gizmo_pipeline:map_translation"
                    }
                    transform_map_rotation {
                        name = "gizmo_pipeline:map_rotation"
                    }
                    transform_map_scale {
                        name = "gizmo_pipeline:map_scale"
                        transform_drop {
                            name = "gizmo_pipeline:drop_scale"
                        }
                    }
                    Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.5, 0.5, 0.5) {
                        name = "gizmo_root"
                        overlay {
                            name = "gizmo_overlay"
                            transform_fork_trs {
                                name = "gizmo_space_world_pipeline"
                                transform_map_translation {
                                    name = "gizmo_space_world_pipeline:map_translation"
                                }
                                transform_map_rotation {
                                    name = "gizmo_space_world_pipeline:map_rotation"
                                    transform_drop {
                                        name = "gizmo_space_world_pipeline:drop_rotation"
                                    }
                                }
                                transform_map_scale {
                                    name = "gizmo_space_world_pipeline:map_scale"
                                }
                                TransformGizmoTranslate.x() {
                                    name = "gizmo_move_x"
                                    Raycastable.enabled() {
                                        name = "gizmo_move_x_pick"
                                        Transform.position(0.5, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 0.05999999865889549, 0.05999999865889549) {
                                            name = "gizmo_move_x_stem_t"
                                            Renderable.cube() {
                                                name = "gizmo_move_x_stem_r"
                                                Color.rgba(1.0, 0.15000000596046448, 0.15000000596046448, 1.0) {
                                                    name = "gizmo_move_x_stem_color"
                                                }
                                                Emissive.on() {
                                                    name = "gizmo_move_x_stem_emissive"
                                                }
                                                Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                            }
                                        }
                                        Transform.position(1.1100000143051147, 0.0, 0.0).rotation_quat(0.0, 0.7071067690849304, 0.0, 0.7071067690849304).scale(0.11999999731779099, 0.11999999731779099, 0.2199999988079071) {
                                            name = "gizmo_move_x_tip_t"
                                            Renderable {
                                                name = "gizmo_move_x_tip_r"
                                                Color.rgba(1.0, 0.15000000596046448, 0.15000000596046448, 1.0) {
                                                    name = "gizmo_move_x_tip_color"
                                                }
                                                Emissive.on() {
                                                    name = "gizmo_move_x_tip_emissive"
                                                }
                                                Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                            }
                                        }
                                    }
                                }
                                TransformGizmoTranslate.y() {
                                    name = "gizmo_move_y"
                                    Raycastable.enabled() {
                                        name = "gizmo_move_y_pick"
                                        Transform.position(0.0, 0.5, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.05999999865889549, 1.0, 0.05999999865889549) {
                                            name = "gizmo_move_y_stem_t"
                                            Renderable.cube() {
                                                name = "gizmo_move_y_stem_r"
                                                Color.rgba(0.15000000596046448, 1.0, 0.15000000596046448, 1.0) {
                                                    name = "gizmo_move_y_stem_color"
                                                }
                                                Emissive.on() {
                                                    name = "gizmo_move_y_stem_emissive"
                                                }
                                                Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                            }
                                        }
                                        Transform.position(0.0, 1.1100000143051147, 0.0).rotation_quat(-0.7071067690849304, 0.0, 0.0, 0.7071067690849304).scale(0.11999999731779099, 0.11999999731779099, 0.2199999988079071) {
                                            name = "gizmo_move_y_tip_t"
                                            Renderable {
                                                name = "gizmo_move_y_tip_r"
                                                Color.rgba(0.15000000596046448, 1.0, 0.15000000596046448, 1.0) {
                                                    name = "gizmo_move_y_tip_color"
                                                }
                                                Emissive.on() {
                                                    name = "gizmo_move_y_tip_emissive"
                                                }
                                                Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                            }
                                        }
                                    }
                                }
                                TransformGizmoTranslate.z() {
                                    name = "gizmo_move_z"
                                    Raycastable.enabled() {
                                        name = "gizmo_move_z_pick"
                                        Transform.position(0.0, 0.0, 0.5).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.05999999865889549, 0.05999999865889549, 1.0) {
                                            name = "gizmo_move_z_stem_t"
                                            Renderable.cube() {
                                                name = "gizmo_move_z_stem_r"
                                                Color.rgba(0.15000000596046448, 0.3499999940395355, 1.0, 1.0) {
                                                    name = "gizmo_move_z_stem_color"
                                                }
                                                Emissive.on() {
                                                    name = "gizmo_move_z_stem_emissive"
                                                }
                                                Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                            }
                                        }
                                        Transform.position(0.0, 0.0, 1.1100000143051147).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.11999999731779099, 0.11999999731779099, 0.2199999988079071) {
                                            name = "gizmo_move_z_tip_t"
                                            Renderable {
                                                name = "gizmo_move_z_tip_r"
                                                Color.rgba(0.15000000596046448, 0.3499999940395355, 1.0, 1.0) {
                                                    name = "gizmo_move_z_tip_color"
                                                }
                                                Emissive.on() {
                                                    name = "gizmo_move_z_tip_emissive"
                                                }
                                                Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                            }
                                        }
                                    }
                                }
                            }
                            transform_fork_trs {
                                name = "gizmo_space_local_pipeline"
                                transform_map_translation {
                                    name = "gizmo_space_local_pipeline:map_translation"
                                }
                                transform_map_rotation {
                                    name = "gizmo_space_local_pipeline:map_rotation"
                                }
                                transform_map_scale {
                                    name = "gizmo_space_local_pipeline:map_scale"
                                }
                                TransformGizmoRotate.x() {
                                    name = "gizmo_rot_x"
                                    GestureCoordType.screen_space_1d_slider() {
                                        name = "gizmo_rot_x_coord"
                                        Raycastable.enabled() {
                                            name = "gizmo_rot_x_pick"
                                            Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, -0.7071067690849304, 0.0, 0.7071067690849304).scale(1.399999976158142, 1.399999976158142, 1.0) {
                                                name = "gizmo_rot_x_ring_t"
                                                Renderable.circle2d() {
                                                    name = "gizmo_rot_x_ring_r"
                                                    Color.rgba(1.0, 0.15000000596046448, 0.15000000596046448, 1.0) {
                                                        name = "gizmo_rot_x_ring_color"
                                                    }
                                                    Emissive.on() {
                                                        name = "gizmo_rot_x_ring_emissive"
                                                    }
                                                    Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                }
                                            }
                                        }
                                    }
                                }
                                TransformGizmoRotate.y() {
                                    name = "gizmo_rot_y"
                                    GestureCoordType.screen_space_1d_slider() {
                                        name = "gizmo_rot_y_coord"
                                        Raycastable.enabled() {
                                            name = "gizmo_rot_y_pick"
                                            Transform.position(0.0, 0.0, 0.0).rotation_quat(0.7071067690849304, 0.0, 0.0, 0.7071067690849304).scale(1.399999976158142, 1.399999976158142, 1.0) {
                                                name = "gizmo_rot_y_ring_t"
                                                Renderable.circle2d() {
                                                    name = "gizmo_rot_y_ring_r"
                                                    Color.rgba(0.15000000596046448, 1.0, 0.15000000596046448, 1.0) {
                                                        name = "gizmo_rot_y_ring_color"
                                                    }
                                                    Emissive.on() {
                                                        name = "gizmo_rot_y_ring_emissive"
                                                    }
                                                    Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                }
                                            }
                                        }
                                    }
                                }
                                TransformGizmoRotate.z() {
                                    name = "gizmo_rot_z"
                                    GestureCoordType.screen_space_1d_slider() {
                                        name = "gizmo_rot_z_coord"
                                        Raycastable.enabled() {
                                            name = "gizmo_rot_z_pick"
                                            Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.399999976158142, 1.399999976158142, 1.0) {
                                                name = "gizmo_rot_z_ring_t"
                                                Renderable.circle2d() {
                                                    name = "gizmo_rot_z_ring_r"
                                                    Color.rgba(0.15000000596046448, 0.3499999940395355, 1.0, 1.0) {
                                                        name = "gizmo_rot_z_ring_color"
                                                    }
                                                    Emissive.on() {
                                                        name = "gizmo_rot_z_ring_emissive"
                                                    }
                                                    Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    Raycastable.enabled() {
        name = "editor_auto_raycastable"
        Transform.position(2.0, 0.0, 1.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
            Transform.position(-0.8999999761581421, -0.4399999976158142, -1.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.5, 0.5, 0.5) {
                Renderable.cube() {
                    Color.rgba(1.0, 0.8799999952316284, 0.15000000596046448, 1.0)
                    Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                }
            }
            Transform.position(0.0, -0.4399999976158142, -0.699999988079071).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.5, 0.5, 0.5) {
                Renderable.cube() {
                    Color.rgba(1.0, 0.3499999940395355, 0.7799999713897705, 1.0)
                    Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                }
            }
            Transform.position(0.8999999761581421, -0.4399999976158142, -1.100000023841858).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.5, 0.5, 0.5) {
                Renderable.cube() {
                    Color.rgba(0.10000000149011612, 0.949999988079071, 1.0, 1.0)
                    Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                }
            }
        }
    }
    Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
        name = "editor_gizmo_anchor"
    }
}

Background {
    Transform.position(2.0, 1.5, -8.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(3.5, 3.5, 3.5) {
        Renderable.circle2d() {
            Color.rgba(1.0, 0.8500000238418579, 0.15000000596046448, 1.0)
            Emissive.on()
            Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
        }
        Transform.position(-0.3499999940395355, 0.3499999940395355, -0.009999999776482582).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.44999998807907104, 0.44999998807907104, 0.44999998807907104) {
            Renderable.circle2d() {
                Color.rgba(1.0, 1.0, 1.0, 1.0)
                Emissive.on()
                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
            }
        }
    }
}

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

InputXR.on() {
    Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
        ControllerXR.new(true, "Left", "Aim") {
            Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.05999999865889549, 0.05999999865889549, 0.11999999731779099) {
                transform_fork_trs {
                    transform_map_translation {}
                    transform_map_rotation {
                        QuatTemporalFilter.smoothing_factor(220.0)
                    }
                    transform_map_scale {}
                    Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                        Renderable.cube() {
                            Color.rgba(0.10000000149011612, 0.8999999761581421, 1.0, 1.0)
                            Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                        }
                    }
                }
            }
        }
        ControllerXR.new(true, "Right", "Aim") {
            Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.05999999865889549, 0.05999999865889549, 0.11999999731779099) {
                transform_fork_trs {
                    transform_map_translation {}
                    transform_map_rotation {
                        QuatTemporalFilter.smoothing_factor(220.0)
                    }
                    transform_map_scale {}
                    Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                        Renderable.cube() {
                            Color.rgba(1.0, 0.3499999940395355, 0.3499999940395355, 1.0)
                            Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                        }
                    }
                }
            }
        }
    }
}

Input.speed(1.0) {
    InputTransformMode.forward_z().roll_axis_y().fps_rotation()
    Transform.position(3.0, 1.2000000476837158, 3.5).rotation_quat(-0.1064513623714447, -0.027149943634867668, -0.002907761372625828, 0.9939429759979248).scale(1.0, 1.0, 1.0) {
        Camera3D.target("window").fov(60.0).near(0.10000000149011612).far(150.0)
        Pointer {
            Raycast.event_driven().max_distance(200.0)
        }
    }
}

OpenXR.on()

Transform.position(0.0, 2.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
    DirectionalLight.intensity(0.800000011920929).color(1.0, 0.44999998807907104, 0.8500000238418579)
}

Transform.position(-1.0, -1.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
    DirectionalLight.intensity(0.800000011920929).color(1.0, 0.8999999761581421, 0.15000000596046448)
}

Transform.position(1.0, -1.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
    DirectionalLight.intensity(0.800000011920929).color(1.0, 0.6000000238418579, 0.15000000596046448)
}

Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
    name = "paint_panel_root"
    Style {}
    Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
        name = "title_bar"
        Style {}
        Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
            Text {
                "Preview"
            }
        }
    }
    Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
        name = "content_slot"
        Style {}
        Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
            name = "paint_tool_options_wrap"
            Selection.()
            Style {}
            Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                name = "paint_panel_item"
                Option.()
                Raycastable.disabled()
                Style {}
                Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                    Style {}
                    Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                        Style {}
                        FitBounds.renderable_only().to_container()
                    }
                    Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                        Style {}
                        Text {
                            "Free Draw"
                        }
                    }
                }
            }
            Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                name = "paint_panel_item"
                Option.()
                Raycastable.disabled()
                Style {}
                Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                    Style {}
                    Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                        Style {}
                        FitBounds.renderable_only().to_container()
                    }
                    Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                        Style {}
                        Text {
                            "Line"
                        }
                    }
                }
            }
            Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                name = "paint_panel_item"
                Option.()
                Raycastable.disabled()
                Style {}
                Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                    Style {}
                    Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                        Style {}
                        FitBounds.renderable_only().to_container()
                    }
                    Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                        Style {}
                        Text {
                            "Spray Can"
                        }
                    }
                }
            }
            Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                name = "paint_panel_item"
                Option.()
                Raycastable.disabled()
                Style {}
                Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                    Style {}
                    Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                        Style {}
                        FitBounds.renderable_only().to_container()
                    }
                    Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                        Style {}
                        Text {
                            "Fill"
                        }
                    }
                }
            }
            Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                name = "paint_panel_item"
                Option.()
                Raycastable.disabled()
                Style {}
                Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                    Style {}
                    Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                        Style {}
                        FitBounds.renderable_only().to_container()
                    }
                    Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                        Style {}
                        Text {
                            "Erase"
                        }
                    }
                }
            }
        }
    }
}

Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
    name = "world_panel_root"
    Style {}
    Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
        name = "path_input_wrap"
        Style {}
        Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
            TextInput {
                "assets/world/default.mms"
                name = "path_input"
            }
        }
    }
    Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
        name = "title_bar"
        Style {}
        Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
            name = "title_label_wrap"
            Style {}
            Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                Text {
                    "Preview"
                    name = "title_label"
                }
            }
        }
        Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
            name = "save_button"
            Raycastable.disabled()
            Style {}
            Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                Text {
                    "Save"
                }
            }
        }
        Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
            name = "load_button"
            Raycastable.disabled()
            Style {}
            Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                Text {
                    "Load"
                }
            }
        }
    }
    Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
        name = "content_slot"
        Style {}
        Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
            name = "world_panel_content_root"
            Style {}
            Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                name = "rows_mount"
                Style {}
            }
        }
    }
    Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
        name = "save_status_wrap"
        Style {}
        Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
            name = "panel_status_root"
            Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                Text {
                    "idle"
                    name = "panel_status_value"
                }
            }
        }
    }
}

Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
    name = "inspector_panel_root"
    Style {}
    Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
        name = "title_bar"
        Style {}
        Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
            name = "title_label_wrap"
            Style {}
            Transform.position(0.0, 0.0, 0.014999999664723873).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                Text {
                    "Preview"
                    name = "title_label"
                }
            }
        }
    }
    Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
        name = "content_slot"
        Style {}
        Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
            name = "inspector_panel_content_root"
            Style {}
            Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                name = "rows_mount"
                Style {}
            }
        }
    }
}

Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
    name = "assets_root"
    Style {}
    Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
        name = "title_bar"
        Style {}
        Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
            Text {
                "Preview"
            }
        }
    }
    Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
        name = "content_slot"
        Style {}
        Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
            name = "assets_content_area"
            Selection.()
            Style {}
        }
    }
}

Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
    name = "world_panel_content_root"
    Style {}
    Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
        name = "rows_mount"
        Style {}
    }
}

Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
    name = "inspector_panel_content_root"
    Style {}
    Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
        name = "rows_mount"
        Style {}
    }
}

Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
    name = "world_panel_content_root"
    Style {}
    Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
        name = "rows_mount"
        Style {}
    }
}

Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
    name = "paint_panel_item"
    Option.()
    Raycastable.disabled()
    Style {}
    Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
        Style {}
        Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
            Style {}
            FitBounds.renderable_only().to_container()
        }
        Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
            Style {}
            Text {
                "Preview"
            }
        }
    }
}

Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
    name = "panel_status_root"
    Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
        Text {
            "Preview"
            name = "panel_status_value"
        }
    }
}

