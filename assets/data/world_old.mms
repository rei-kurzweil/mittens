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
        Transform.position(0.0, -0.7799999713897705, -0.4000000059604645).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(12.0, 0.18000000715255737, 9.5) {
            Renderable.cube() {
                Color.rgba(0.18000000715255737, 0.18000000715255737, 0.2199999988079071, 1.0)
                Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
            }
        }
    }
    Raycastable.enabled() {
        name = "editor_auto_raycastable"
        Transform.position(0.0, 2.1500000953674316, -7.199999809265137).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(8.800000190734863, 3.5999999046325684, 0.23999999463558197) {
            Renderable.cube() {
                Color.rgba(0.10999999940395355, 0.10000000149011612, 0.14000000059604645, 1.0)
                Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
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
        Transform.position(-0.9250506162643433, 1.6984316110610962, -0.17940136790275574).rotation_quat(0.4078806936740875, -0.18548893928527832, -0.13585136830806732, 0.8836128115653992).scale(1.0, 1.0, 1.0) {
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
                transform_fork_trs {
                    transform_map_rotation {
                        QuatYawFollow.new(0.7853981852531433, 3.0).initial_yaw(3.141590118408203)
                    }
                    Transform.position(0.059735991060733795, -1.649067997932434, -0.01011747308075428).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                        GLTF.new("assets/models/bisket.8.0.glb").with_visualized_transforms(true)
                        Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                            name = "Armature.003"
                            BoneRestPose.translation(0.0, 0.0, 0.0)
                            Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                name = "viz:Armature.003"
                                SignalRouteUpward.new("update_transform", "transform") {
                                    name = "route_upward:viz:Armature.003"
                                }
                                overlay {
                                    name = "viz_overlay:Armature.003"
                                    Renderable.cube() {
                                        name = "viz_box:Armature.003"
                                        Raycastable.enabled()
                                        Color.rgba(1.0, 1.0, 1.0, 1.0)
                                        Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                    }
                                }
                            }
                            Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                                name = "Body"
                                BoneRestPose.translation(0.0, 0.0, 0.0)
                                Renderable {
                                    Mesh.new("bisket.8.0:Body_(merged).baked.001:prim0")
                                    SkinnedMesh.new(0.0)
                                    Texture.with_uri("bisket.8.0:_10_001")
                                    Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                }
                                Renderable {
                                    Mesh.new("bisket.8.0:Body_(merged).baked.001:prim1")
                                    SkinnedMesh.new(0.0)
                                    Texture.with_uri("bisket.8.0:_12_001")
                                    Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                }
                                Renderable {
                                    Mesh.new("bisket.8.0:Body_(merged).baked.001:prim2")
                                    SkinnedMesh.new(0.0)
                                    Texture.with_uri("bisket.8.0:_13_001")
                                    Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                }
                                Renderable {
                                    Mesh.new("bisket.8.0:Body_(merged).baked.001:prim3")
                                    SkinnedMesh.new(0.0)
                                    Texture.with_uri("bisket.8.0:_14_001")
                                    Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                }
                                Renderable {
                                    Mesh.new("bisket.8.0:Body_(merged).baked.001:prim4")
                                    SkinnedMesh.new(0.0)
                                    Texture.with_uri("bisket.8.0:_15_001")
                                    Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                }
                                Renderable {
                                    Mesh.new("bisket.8.0:Body_(merged).baked.001:prim5")
                                    SkinnedMesh.new(0.0)
                                    Texture.with_uri("bisket.8.0:_16_001")
                                    Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                }
                                Renderable {
                                    Mesh.new("bisket.8.0:Body_(merged).baked.001:prim6")
                                    SkinnedMesh.new(0.0)
                                    Texture.with_uri("bisket.8.0:_17")
                                    Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                }
                            }
                            Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                                name = "Face.001"
                                BoneRestPose.translation(0.0, 0.0, 0.0)
                                Renderable {
                                    Mesh.new("bisket.8.0:Face_(merged).baked.001:prim0")
                                    SkinnedMesh.new(0.0)
                                    Texture.with_uri("bisket.8.0:_01_001")
                                    Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                }
                                Renderable {
                                    Mesh.new("bisket.8.0:Face_(merged).baked.001:prim1")
                                    SkinnedMesh.new(0.0)
                                    Texture.with_uri("bisket.8.0:_02_001")
                                    Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                }
                                Renderable {
                                    Mesh.new("bisket.8.0:Face_(merged).baked.001:prim2")
                                    SkinnedMesh.new(0.0)
                                    Texture.with_uri("bisket.8.0:_03_001")
                                    Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                }
                                Renderable {
                                    Mesh.new("bisket.8.0:Face_(merged).baked.001:prim3")
                                    SkinnedMesh.new(0.0)
                                    Texture.with_uri("bisket.8.0:_04_001")
                                    Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                }
                                Renderable {
                                    Mesh.new("bisket.8.0:Face_(merged).baked.001:prim4")
                                    SkinnedMesh.new(0.0)
                                    Texture.with_uri("bisket.8.0:_06_001")
                                    Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                }
                                Renderable {
                                    Mesh.new("bisket.8.0:Face_(merged).baked.001:prim5")
                                    SkinnedMesh.new(0.0)
                                    Texture.with_uri("bisket.8.0:_07_001")
                                    Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                }
                                Renderable {
                                    Mesh.new("bisket.8.0:Face_(merged).baked.001:prim6")
                                    SkinnedMesh.new(0.0)
                                    Texture.with_uri("bisket.8.0:_08_001")
                                    Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                }
                                Renderable {
                                    Mesh.new("bisket.8.0:Face_(merged).baked.001:prim7")
                                    SkinnedMesh.new(0.0)
                                    Texture.with_uri("bisket.8.0:_09_001")
                                    Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                }
                            }
                            Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                                name = "Hair"
                                BoneRestPose.translation(0.0, 0.0, 0.0)
                                Renderable {
                                    Mesh.new("bisket.8.0:Hair001_(merged).baked.001:prim0")
                                    SkinnedMesh.new(0.0)
                                    Texture.with_uri("bisket.8.0:_17_001")
                                    Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                }
                            }
                            Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0018655582098290324, -0.0000000001634013202522766, -0.0000000001634013341300644, 0.9999982714653015).scale(1.0, 0.9999999403953552, 0.9999999403953552) {
                                name = "Root"
                                BoneRestPose.translation(0.0, 0.0, 0.0)
                                Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                    name = "viz:Root"
                                    SignalRouteUpward.new("update_transform", "transform") {
                                        name = "route_upward:viz:Root"
                                    }
                                    overlay {
                                        name = "viz_overlay:Root"
                                        Renderable.cube() {
                                            name = "viz_box:Root"
                                            Raycastable.enabled()
                                            Color.rgba(1.0, 1.0, 1.0, 1.0)
                                            Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                        }
                                    }
                                }
                                Transform.position(0.00000000000000004423544863740858, 1.0383638143539429, 0.0).rotation_quat(0.11583171784877777, -0.000000011511732900260085, -0.000000011506645414272043, 0.9932688474655151).scale(1.0, 1.0, 1.0) {
                                    name = "J_Bip_C_Hips"
                                    BoneRestPose.translation(0.00000000000000004423544863740858, 1.0383638143539429, 0.0)
                                    Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                        name = "viz:J_Bip_C_Hips"
                                        SignalRouteUpward.new("update_transform", "transform") {
                                            name = "route_upward:viz:J_Bip_C_Hips"
                                        }
                                        overlay {
                                            name = "viz_overlay:J_Bip_C_Hips"
                                            Renderable.cube() {
                                                name = "viz_box:J_Bip_C_Hips"
                                                Raycastable.enabled()
                                                Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                            }
                                        }
                                    }
                                    Transform.position(0.0000000000000005551115123125783, 0.05355042964220047, -0.000000005587935447692871).rotation_quat(-0.10427791625261307, 0.00000001050004883040856, 0.000000010466528088670657, 0.9945482015609741).scale(1.0, 1.0, 1.0) {
                                        name = "J_Bip_C_Spine"
                                        BoneRestPose.translation(0.0000000000000005551115123125783, 0.05355042964220047, -0.000000005587935447692871)
                                        Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                            name = "viz:J_Bip_C_Spine"
                                            SignalRouteUpward.new("update_transform", "transform") {
                                                name = "route_upward:viz:J_Bip_C_Spine"
                                            }
                                            overlay {
                                                name = "viz_overlay:J_Bip_C_Spine"
                                                Renderable.cube() {
                                                    name = "viz_box:J_Bip_C_Spine"
                                                    Raycastable.enabled()
                                                    Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                    Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                                }
                                            }
                                        }
                                        Transform.position(0.00000000000000020469737016526324, 0.11304620653390884, 0.0000000009313225746154785).rotation_quat(-0.08101870119571686, 0.000000006718217537837745, 0.000000006730505486274296, 0.9967125654220581).scale(1.0, 1.0, 1.0) {
                                            name = "J_Bip_C_Chest"
                                            BoneRestPose.translation(0.00000000000000020469737016526324, 0.11304620653390884, 0.0000000009313225746154785)
                                            Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                                name = "viz:J_Bip_C_Chest"
                                                SignalRouteUpward.new("update_transform", "transform") {
                                                    name = "route_upward:viz:J_Bip_C_Chest"
                                                }
                                                overlay {
                                                    name = "viz_overlay:J_Bip_C_Chest"
                                                    Renderable.cube() {
                                                        name = "viz_box:J_Bip_C_Chest"
                                                        Raycastable.enabled()
                                                        Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                        Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                                    }
                                                }
                                            }
                                            Transform.position(-0.00000000000000022898349882893854, 0.10862815380096436, -0.000000009313225746154785).rotation_quat(-0.07341954857110977, 0.000000005314375162157603, 0.000000005213109055546283, 0.9973011612892151).scale(1.0, 0.9999997615814209, 0.9999999403953552) {
                                                name = "J_Bip_C_UpperChest"
                                                BoneRestPose.translation(-0.00000000000000022898349882893854, 0.10862815380096436, -0.000000009313225746154785)
                                                Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                                    name = "viz:J_Bip_C_UpperChest"
                                                    SignalRouteUpward.new("update_transform", "transform") {
                                                        name = "route_upward:viz:J_Bip_C_UpperChest"
                                                    }
                                                    overlay {
                                                        name = "viz_overlay:J_Bip_C_UpperChest"
                                                        Renderable.cube() {
                                                            name = "viz_box:J_Bip_C_UpperChest"
                                                            Raycastable.enabled()
                                                            Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                            Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                                        }
                                                    }
                                                }
                                                Transform.position(0.0528692826628685, -0.02617679163813591, 0.06422580778598785).rotation_quat(0.04079008847475052, -0.5668908953666687, -0.754605770111084, 0.32793447375297546).scale(1.0, 0.9999998807907104, 1.0) {
                                                    name = "J_Sec_L_Bust1"
                                                    BoneRestPose.translation(0.0528692826628685, -0.02617679163813591, 0.06422580778598785)
                                                    Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                                        name = "viz:J_Sec_L_Bust1"
                                                        SignalRouteUpward.new("update_transform", "transform") {
                                                            name = "route_upward:viz:J_Sec_L_Bust1"
                                                        }
                                                        overlay {
                                                            name = "viz_overlay:J_Sec_L_Bust1"
                                                            Renderable.cube() {
                                                                name = "viz_box:J_Sec_L_Bust1"
                                                                Raycastable.enabled()
                                                                Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                                Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                                            }
                                                        }
                                                    }
                                                    Transform.position(-0.00000004470348358154297, 0.04274602234363556, 0.000000017695128917694092).rotation_quat(-0.00000014528632164001465, -0.00000042188912630081177, 0.00000006984919309616089, 1.0).scale(1.0, 0.9999999403953552, 1.0) {
                                                        name = "J_Sec_L_Bust2"
                                                        BoneRestPose.translation(-0.00000004470348358154297, 0.04274602234363556, 0.000000017695128917694092)
                                                        Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                                            name = "viz:J_Sec_L_Bust2"
                                                            SignalRouteUpward.new("update_transform", "transform") {
                                                                name = "route_upward:viz:J_Sec_L_Bust2"
                                                            }
                                                            overlay {
                                                                name = "viz_overlay:J_Sec_L_Bust2"
                                                                Renderable.cube() {
                                                                    name = "viz_box:J_Sec_L_Bust2"
                                                                    Raycastable.enabled()
                                                                    Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                                    Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                                Transform.position(-0.0528692826628685, -0.02617679163813591, 0.06422582268714905).rotation_quat(0.04079004004597664, 0.5668910145759583, 0.754605770111084, 0.3279343545436859).scale(0.9999999403953552, 1.000000238418579, 1.0000001192092896) {
                                                    name = "J_Sec_R_Bust1"
                                                    BoneRestPose.translation(-0.0528692826628685, -0.02617679163813591, 0.06422582268714905)
                                                    Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                                        name = "viz:J_Sec_R_Bust1"
                                                        SignalRouteUpward.new("update_transform", "transform") {
                                                            name = "route_upward:viz:J_Sec_R_Bust1"
                                                        }
                                                        overlay {
                                                            name = "viz_overlay:J_Sec_R_Bust1"
                                                            Renderable.cube() {
                                                                name = "viz_box:J_Sec_R_Bust1"
                                                                Raycastable.enabled()
                                                                Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                                Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                                            }
                                                        }
                                                    }
                                                    Transform.position(-0.000000007450580596923828, 0.04274601489305496, -0.00000010523945093154907).rotation_quat(-0.00000010803343286625022, 0.00000016577541828155518, 0.0000000894069742685133, 1.0).scale(1.0, 0.9999999403953552, 0.9999999403953552) {
                                                        name = "J_Sec_R_Bust2"
                                                        BoneRestPose.translation(-0.000000007450580596923828, 0.04274601489305496, -0.00000010523945093154907)
                                                        Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                                            name = "viz:J_Sec_R_Bust2"
                                                            SignalRouteUpward.new("update_transform", "transform") {
                                                                name = "route_upward:viz:J_Sec_R_Bust2"
                                                            }
                                                            overlay {
                                                                name = "viz_overlay:J_Sec_R_Bust2"
                                                                Renderable.cube() {
                                                                    name = "viz_box:J_Sec_R_Bust2"
                                                                    Raycastable.enabled()
                                                                    Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                                    Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                                Transform.position(-0.00000000000000011102230246251565, 0.13772687315940857, -0.000000014901161193847656).rotation_quat(0.20333121716976166, -0.00000001798450632861659, -0.00000001878573563374175, 0.9791100025177002).scale(1.0, 1.0, 1.0) {
                                                    name = "J_Bip_C_Neck"
                                                    BoneRestPose.translation(-0.00000000000000011102230246251565, 0.13772687315940857, -0.000000014901161193847656)
                                                    Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                                        name = "viz:J_Bip_C_Neck"
                                                        SignalRouteUpward.new("update_transform", "transform") {
                                                            name = "route_upward:viz:J_Bip_C_Neck"
                                                        }
                                                        overlay {
                                                            name = "viz_overlay:J_Bip_C_Neck"
                                                            Renderable.cube() {
                                                                name = "viz_box:J_Bip_C_Neck"
                                                                Raycastable.enabled()
                                                                Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                                Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                                            }
                                                        }
                                                    }
                                                    Transform.position(-0.00000000028972801935367443, 0.029923057183623314, 0.008656224235892296).rotation_quat(-0.06361284106969833, 0.000000007718107930543283, 0.000000007718105266008024, 0.997974693775177).scale(1.0, 1.0000001192092896, 1.0) {
                                                        name = "J_Bip_C_Neck_collider_0.001"
                                                        BoneRestPose.translation(-0.00000000028972801935367443, 0.029923057183623314, 0.008656224235892296)
                                                        Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                                            name = "viz:J_Bip_C_Neck_collider_0.001"
                                                            SignalRouteUpward.new("update_transform", "transform") {
                                                                name = "route_upward:viz:J_Bip_C_Neck_collider_0.001"
                                                            }
                                                            overlay {
                                                                name = "viz_overlay:J_Bip_C_Neck_collider_0.001"
                                                                Renderable.cube() {
                                                                    name = "viz_box:J_Bip_C_Neck_collider_0.001"
                                                                    Raycastable.enabled()
                                                                    Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                                    Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                                                }
                                                            }
                                                        }
                                                    }
                                                    overlay {
                                                        Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.02500000037252903, 0.02500000037252903, 0.02500000037252903) {
                                                            Renderable.cube() {
                                                                Color.rgba(0.20000000298023224, 0.8500000238418579, 0.8500000238418579, 0.8999999761581421)
                                                                Emissive.on()
                                                                Raycastable.enabled()
                                                                Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                                            }
                                                        }
                                                    }
                                                    Transform.position(0.000000000000002525757381022231, 0.07545606046915054, -0.000000005587935447692871).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0)
                                                }
                                                Transform.position(0.021677928045392036, 0.10984349995851517, 0.0006475210539065301).rotation_quat(-0.5069339871406555, -0.42019739747047424, -0.5593377351760864, 0.5035805106163025).scale(1.0, 1.0, 1.0) {
                                                    name = "J_Bip_L_Shoulder"
                                                    BoneRestPose.translation(0.021677928045392036, 0.10984349995851517, 0.0006475210539065301)
                                                    Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                                        name = "viz:J_Bip_L_Shoulder"
                                                        SignalRouteUpward.new("update_transform", "transform") {
                                                            name = "route_upward:viz:J_Bip_L_Shoulder"
                                                        }
                                                        overlay {
                                                            name = "viz_overlay:J_Bip_L_Shoulder"
                                                            Renderable.cube() {
                                                                name = "viz_box:J_Bip_L_Shoulder"
                                                                Raycastable.enabled()
                                                                Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                                Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                                            }
                                                        }
                                                    }
                                                    Transform.position(-0.0000000023283064365386963, 0.07559478282928467, 0.00000006426125764846802).rotation_quat(-0.17225423455238342, -0.5584241151809692, -0.7140906453132629, 0.3854421377182007).scale(1.0, 0.9999999403953552, 1.0000001192092896) {
                                                        name = "J_Bip_L_UpperArm"
                                                        BoneRestPose.translation(-0.0000000023283064365386963, 0.07559478282928467, 0.00000006426125764846802)
                                                        Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                                            name = "viz:J_Bip_L_UpperArm"
                                                            SignalRouteUpward.new("update_transform", "transform") {
                                                                name = "route_upward:viz:J_Bip_L_UpperArm"
                                                            }
                                                            overlay {
                                                                name = "viz_overlay:J_Bip_L_UpperArm"
                                                                Renderable.cube() {
                                                                    name = "viz_box:J_Bip_L_UpperArm"
                                                                    Raycastable.enabled()
                                                                    Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                                    Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                                                }
                                                            }
                                                        }
                                                        Transform.position(-0.004558193497359753, 0.12023422122001648, -0.08020519465208054).rotation_quat(0.08458263427019119, 0.08458265662193298, 0.0035606196615844965, 0.9928136467933655).scale(0.9999999403953552, 1.0, 0.9999999403953552) {
                                                            name = "J_Sec_L_TopsUpperArmInside_01"
                                                            BoneRestPose.translation(-0.004558193497359753, 0.12023422122001648, -0.08020519465208054)
                                                            Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                                                name = "viz:J_Sec_L_TopsUpperArmInside_01"
                                                                SignalRouteUpward.new("update_transform", "transform") {
                                                                    name = "route_upward:viz:J_Sec_L_TopsUpperArmInside_01"
                                                                }
                                                                overlay {
                                                                    name = "viz_overlay:J_Sec_L_TopsUpperArmInside_01"
                                                                    Renderable.cube() {
                                                                        name = "viz_box:J_Sec_L_TopsUpperArmInside_01"
                                                                        Raycastable.enabled()
                                                                        Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                                        Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                                                    }
                                                                }
                                                            }
                                                            Transform.position(0.00000001210719347000122, 0.1125951036810875, -0.0000001341104507446289).rotation_quat(-0.000000007130438905988967, -0.000000031082890927791595, 0.000000023283066141743802, 1.0).scale(1.0, 0.9999999403953552, 1.0) {
                                                                name = "J_Sec_L_TopsUpperArmInside_end_01"
                                                                BoneRestPose.translation(0.00000001210719347000122, 0.1125951036810875, -0.0000001341104507446289)
                                                                Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                                                    name = "viz:J_Sec_L_TopsUpperArmInside_end_01"
                                                                    SignalRouteUpward.new("update_transform", "transform") {
                                                                        name = "route_upward:viz:J_Sec_L_TopsUpperArmInside_end_01"
                                                                    }
                                                                    overlay {
                                                                        name = "viz_overlay:J_Sec_L_TopsUpperArmInside_end_01"
                                                                        Renderable.cube() {
                                                                            name = "viz_box:J_Sec_L_TopsUpperArmInside_end_01"
                                                                            Raycastable.enabled()
                                                                            Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                                            Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                        }
                                                        Transform.position(-0.010608416050672531, 0.019302375614643097, 0.044086702167987823).rotation_quat(0.09242912381887436, 0.09242913872003555, -0.0037926076911389828, 0.9914128184318542).scale(0.9999999403953552, 0.9999999403953552, 0.9999999403953552) {
                                                            name = "J_Sec_L_TopsUpperArmOutside_01"
                                                            BoneRestPose.translation(-0.010608416050672531, 0.019302375614643097, 0.044086702167987823)
                                                            Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                                                name = "viz:J_Sec_L_TopsUpperArmOutside_01"
                                                                SignalRouteUpward.new("update_transform", "transform") {
                                                                    name = "route_upward:viz:J_Sec_L_TopsUpperArmOutside_01"
                                                                }
                                                                overlay {
                                                                    name = "viz_overlay:J_Sec_L_TopsUpperArmOutside_01"
                                                                    Renderable.cube() {
                                                                        name = "viz_box:J_Sec_L_TopsUpperArmOutside_01"
                                                                        Raycastable.enabled()
                                                                        Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                                        Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                                                    }
                                                                }
                                                            }
                                                            Transform.position(-0.000000021187588572502136, 0.18002934753894806, 0.000000004190951585769653).rotation_quat(-0.000000003725290742551124, -0.00000003908645140882072, 0.000000022351745343485163, 1.0).scale(0.9999999403953552, 0.9999998807907104, 0.9999999403953552) {
                                                                name = "J_Sec_L_TopsUpperArmOutside_end_01"
                                                                BoneRestPose.translation(-0.000000021187588572502136, 0.18002934753894806, 0.000000004190951585769653)
                                                                Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                                                    name = "viz:J_Sec_L_TopsUpperArmOutside_end_01"
                                                                    SignalRouteUpward.new("update_transform", "transform") {
                                                                        name = "route_upward:viz:J_Sec_L_TopsUpperArmOutside_end_01"
                                                                    }
                                                                    overlay {
                                                                        name = "viz_overlay:J_Sec_L_TopsUpperArmOutside_end_01"
                                                                        Renderable.cube() {
                                                                            name = "viz_box:J_Sec_L_TopsUpperArmOutside_end_01"
                                                                            Raycastable.enabled()
                                                                            Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                                            Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                        }
                                                        Transform.position(-0.0000000023774280322186314, 0.21951669454574585, 0.00000000995981075391228).rotation_quat(-0.4505547881126404, -0.02222420647740364, -0.5771459341049194, 0.6807416081428528).scale(1.0, 1.0, 1.0) {
                                                            name = "J_Bip_L_LowerArm"
                                                            BoneRestPose.translation(-0.0000000023774280322186314, 0.21951669454574585, 0.00000000995981075391228)
                                                            Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                                                name = "viz:J_Bip_L_LowerArm"
                                                                SignalRouteUpward.new("update_transform", "transform") {
                                                                    name = "route_upward:viz:J_Bip_L_LowerArm"
                                                                }
                                                                overlay {
                                                                    name = "viz_overlay:J_Bip_L_LowerArm"
                                                                    Renderable.cube() {
                                                                        name = "viz_box:J_Bip_L_LowerArm"
                                                                        Raycastable.enabled()
                                                                        Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                                        Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                                                    }
                                                                }
                                                            }
                                                            Transform.position(0.0000000004347384674474597, 0.20797443389892578, -0.0000000541592157787818).rotation_quat(0.38345497846603394, 0.7227474451065063, 0.10409173369407654, 0.5654761791229248).scale(0.9999998807907104, 1.0, 1.0) {
                                                                name = "J_Bip_L_Hand"
                                                                BoneRestPose.translation(0.0000000004347384674474597, 0.20797443389892578, -0.0000000541592157787818)
                                                                Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                                                    name = "viz:J_Bip_L_Hand"
                                                                    SignalRouteUpward.new("update_transform", "transform") {
                                                                        name = "route_upward:viz:J_Bip_L_Hand"
                                                                    }
                                                                    overlay {
                                                                        name = "viz_overlay:J_Bip_L_Hand"
                                                                        Renderable.cube() {
                                                                            name = "viz_box:J_Bip_L_Hand"
                                                                            Raycastable.enabled()
                                                                            Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                                            Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                                                        }
                                                                    }
                                                                }
                                                                Transform.position(0.02067512273788452, 0.06335750967264175, 0.004095223732292652).rotation_quat(-0.038334719836711884, -0.03757308050990105, -0.0014515905641019344, 0.9985572695732117).scale(1.0000001192092896, 0.9999998211860657, 1.0) {
                                                                    name = "J_Bip_L_Index1"
                                                                    BoneRestPose.translation(0.02067512273788452, 0.06335750967264175, 0.004095223732292652)
                                                                    Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                                                        name = "viz:J_Bip_L_Index1"
                                                                        SignalRouteUpward.new("update_transform", "transform") {
                                                                            name = "route_upward:viz:J_Bip_L_Index1"
                                                                        }
                                                                        overlay {
                                                                            name = "viz_overlay:J_Bip_L_Index1"
                                                                            Renderable.cube() {
                                                                                name = "viz_box:J_Bip_L_Index1"
                                                                                Raycastable.enabled()
                                                                                Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                                                Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                                                            }
                                                                        }
                                                                    }
                                                                    Transform.position(-0.0000000003583409124985337, 0.03220314905047417, 0.000000007833305559756809).rotation_quat(-0.014766003005206585, -0.014472668059170246, 0.010086135007441044, 0.9997353553771973).scale(0.9999999403953552, 0.9999998807907104, 1.0) {
                                                                        name = "J_Bip_L_Index2"
                                                                        BoneRestPose.translation(-0.0000000003583409124985337, 0.03220314905047417, 0.000000007833305559756809)
                                                                        Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                                                            name = "viz:J_Bip_L_Index2"
                                                                            SignalRouteUpward.new("update_transform", "transform") {
                                                                                name = "route_upward:viz:J_Bip_L_Index2"
                                                                            }
                                                                            overlay {
                                                                                name = "viz_overlay:J_Bip_L_Index2"
                                                                                Renderable.cube() {
                                                                                    name = "viz_box:J_Bip_L_Index2"
                                                                                    Raycastable.enabled()
                                                                                    Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                                                    Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                                                                }
                                                                            }
                                                                        }
                                                                        Transform.position(0.00000000605359673500061, 0.019828328862786293, 0.000000039726728573441505).rotation_quat(0.0000000013387762010097504, 0.0000000031141098588705063, 0.00000007096969056874514, 1.0).scale(1.0, 1.0, 1.0) {
                                                                            name = "J_Bip_L_Index3"
                                                                            BoneRestPose.translation(0.00000000605359673500061, 0.019828328862786293, 0.000000039726728573441505)
                                                                            Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                                                                name = "viz:J_Bip_L_Index3"
                                                                                SignalRouteUpward.new("update_transform", "transform") {
                                                                                    name = "route_upward:viz:J_Bip_L_Index3"
                                                                                }
                                                                                overlay {
                                                                                    name = "viz_overlay:J_Bip_L_Index3"
                                                                                    Renderable.cube() {
                                                                                        name = "viz_box:J_Bip_L_Index3"
                                                                                        Raycastable.enabled()
                                                                                        Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                                                        Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                                                                    }
                                                                                }
                                                                            }
                                                                        }
                                                                    }
                                                                }
                                                                Transform.position(-0.028109662234783173, 0.06010688096284866, 0.0006694854237139225).rotation_quat(-0.0379519984126091, -0.037952058017253876, -0.011470513418316841, 0.9984927177429199).scale(1.0, 0.9999999403953552, 0.9999999403953552) {
                                                                    name = "J_Bip_L_Little1"
                                                                    BoneRestPose.translation(-0.028109662234783173, 0.06010688096284866, 0.0006694854237139225)
                                                                    Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                                                        name = "viz:J_Bip_L_Little1"
                                                                        SignalRouteUpward.new("update_transform", "transform") {
                                                                            name = "route_upward:viz:J_Bip_L_Little1"
                                                                        }
                                                                        overlay {
                                                                            name = "viz_overlay:J_Bip_L_Little1"
                                                                            Renderable.cube() {
                                                                                name = "viz_box:J_Bip_L_Little1"
                                                                                Raycastable.enabled()
                                                                                Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                                                Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                                                            }
                                                                        }
                                                                    }
                                                                    Transform.position(0.0000000027752662390412297, 0.030435627326369286, 0.00000002985255775911355).rotation_quat(-0.0000019520527985150693, -0.0000019557783161872067, 0.00000011988805681539816, 1.0).scale(1.0, 1.0, 1.0) {
                                                                        name = "J_Bip_L_Little2"
                                                                        BoneRestPose.translation(0.0000000027752662390412297, 0.030435627326369286, 0.00000002985255775911355)
                                                                        Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                                                            name = "viz:J_Bip_L_Little2"
                                                                            SignalRouteUpward.new("update_transform", "transform") {
                                                                                name = "route_upward:viz:J_Bip_L_Little2"
                                                                            }
                                                                            overlay {
                                                                                name = "viz_overlay:J_Bip_L_Little2"
                                                                                Renderable.cube() {
                                                                                    name = "viz_box:J_Bip_L_Little2"
                                                                                    Raycastable.enabled()
                                                                                    Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                                                    Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                                                                }
                                                                            }
                                                                        }
                                                                        Transform.position(0.00000000008724669597892643, 0.017541274428367615, 0.000000013671886733845895).rotation_quat(0.00000000000020047438915090504, 0.0000000000004547476219370072, -0.00000010618667545259086, 1.0).scale(1.0, 1.0, 1.0) {
                                                                            name = "J_Bip_L_Little3"
                                                                            BoneRestPose.translation(0.00000000008724669597892643, 0.017541274428367615, 0.000000013671886733845895)
                                                                            Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                                                                name = "viz:J_Bip_L_Little3"
                                                                                SignalRouteUpward.new("update_transform", "transform") {
                                                                                    name = "route_upward:viz:J_Bip_L_Little3"
                                                                                }
                                                                                overlay {
                                                                                    name = "viz_overlay:J_Bip_L_Little3"
                                                                                    Renderable.cube() {
                                                                                        name = "viz_box:J_Bip_L_Little3"
                                                                                        Raycastable.enabled()
                                                                                        Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                                                        Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                                                                    }
                                                                                }
                                                                            }
                                                                        }
                                                                    }
                                                                }
                                                                Transform.position(0.003144238144159317, 0.06457093358039856, 0.002680704463273287).rotation_quat(-0.03795566037297249, -0.03795560449361801, -0.011470030061900616, 0.9984924793243408).scale(0.9999999403953552, 0.9999999403953552, 1.0) {
                                                                    name = "J_Bip_L_Middle1"
                                                                    BoneRestPose.translation(0.003144238144159317, 0.06457093358039856, 0.002680704463273287)
                                                                    Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                                                        name = "viz:J_Bip_L_Middle1"
                                                                        SignalRouteUpward.new("update_transform", "transform") {
                                                                            name = "route_upward:viz:J_Bip_L_Middle1"
                                                                        }
                                                                        overlay {
                                                                            name = "viz_overlay:J_Bip_L_Middle1"
                                                                            Renderable.cube() {
                                                                                name = "viz_box:J_Bip_L_Middle1"
                                                                                Raycastable.enabled()
                                                                                Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                                                Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                                                            }
                                                                        }
                                                                    }
                                                                    Transform.position(0.0000000026665816221793648, 0.03586991876363754, -0.00000010983578135892458).rotation_quat(0.0000016614790183666628, 0.0000016614792457403382, -0.0000002572806181433407, 1.0).scale(1.0, 1.0, 1.0) {
                                                                        name = "J_Bip_L_Middle2"
                                                                        BoneRestPose.translation(0.0000000026665816221793648, 0.03586991876363754, -0.00000010983578135892458)
                                                                        Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                                                            name = "viz:J_Bip_L_Middle2"
                                                                            SignalRouteUpward.new("update_transform", "transform") {
                                                                                name = "route_upward:viz:J_Bip_L_Middle2"
                                                                            }
                                                                            overlay {
                                                                                name = "viz_overlay:J_Bip_L_Middle2"
                                                                                Renderable.cube() {
                                                                                    name = "viz_box:J_Bip_L_Middle2"
                                                                                    Raycastable.enabled()
                                                                                    Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                                                    Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                                                                }
                                                                            }
                                                                        }
                                                                        Transform.position(0.0000000018620118780177108, 0.02212887816131115, -0.0000001134463758489801).rotation_quat(0.00000000000011368683772161603, -0.00000000000022737367544323206, -0.000000000000007159767062266207, 1.0).scale(1.0, 1.0, 1.0) {
                                                                            name = "J_Bip_L_Middle3"
                                                                            BoneRestPose.translation(0.0000000018620118780177108, 0.02212887816131115, -0.0000001134463758489801)
                                                                            Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                                                                name = "viz:J_Bip_L_Middle3"
                                                                                SignalRouteUpward.new("update_transform", "transform") {
                                                                                    name = "route_upward:viz:J_Bip_L_Middle3"
                                                                                }
                                                                                overlay {
                                                                                    name = "viz_overlay:J_Bip_L_Middle3"
                                                                                    Renderable.cube() {
                                                                                        name = "viz_box:J_Bip_L_Middle3"
                                                                                        Raycastable.enabled()
                                                                                        Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                                                        Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                                                                    }
                                                                                }
                                                                            }
                                                                        }
                                                                    }
                                                                }
                                                                Transform.position(-0.012459427118301392, 0.06415431201457977, 0.0015368678141385317).rotation_quat(-0.03795398026704788, -0.03795398026704788, -0.01147023681551218, 0.9984925985336304).scale(1.0000001192092896, 1.0, 1.0) {
                                                                    name = "J_Bip_L_Ring1"
                                                                    BoneRestPose.translation(-0.012459427118301392, 0.06415431201457977, 0.0015368678141385317)
                                                                    Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                                                        name = "viz:J_Bip_L_Ring1"
                                                                        SignalRouteUpward.new("update_transform", "transform") {
                                                                            name = "route_upward:viz:J_Bip_L_Ring1"
                                                                        }
                                                                        overlay {
                                                                            name = "viz_overlay:J_Bip_L_Ring1"
                                                                            Renderable.cube() {
                                                                                name = "viz_box:J_Bip_L_Ring1"
                                                                                Raycastable.enabled()
                                                                                Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                                                Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                                                            }
                                                                        }
                                                                    }
                                                                    Transform.position(-0.000000005912879075253841, 0.033274054527282715, -0.00000010920022219806924).rotation_quat(0.00000000000006097689220956395, 0.000000007450581485102248, 0.0000002407111878710566, 1.0).scale(1.0, 0.9999999403953552, 0.9999999403953552) {
                                                                        name = "J_Bip_L_Ring2"
                                                                        BoneRestPose.translation(-0.000000005912879075253841, 0.033274054527282715, -0.00000010920022219806924)
                                                                        Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                                                            name = "viz:J_Bip_L_Ring2"
                                                                            SignalRouteUpward.new("update_transform", "transform") {
                                                                                name = "route_upward:viz:J_Bip_L_Ring2"
                                                                            }
                                                                            overlay {
                                                                                name = "viz_overlay:J_Bip_L_Ring2"
                                                                                Renderable.cube() {
                                                                                    name = "viz_box:J_Bip_L_Ring2"
                                                                                    Raycastable.enabled()
                                                                                    Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                                                    Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                                                                }
                                                                            }
                                                                        }
                                                                        Transform.position(-0.000000003798711123437215, 0.019202372059226036, -0.00000010977224462749291).rotation_quat(-0.000000000000000000010078233681820409, -0.0000000000000013322323718037584, -0.000000000000014210847091905478, 1.0).scale(1.0, 0.9999999403953552, 0.9999999403953552) {
                                                                            name = "J_Bip_L_Ring3"
                                                                            BoneRestPose.translation(-0.000000003798711123437215, 0.019202372059226036, -0.00000010977224462749291)
                                                                            Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                                                                name = "viz:J_Bip_L_Ring3"
                                                                                SignalRouteUpward.new("update_transform", "transform") {
                                                                                    name = "route_upward:viz:J_Bip_L_Ring3"
                                                                                }
                                                                                overlay {
                                                                                    name = "viz_overlay:J_Bip_L_Ring3"
                                                                                    Renderable.cube() {
                                                                                        name = "viz_box:J_Bip_L_Ring3"
                                                                                        Raycastable.enabled()
                                                                                        Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                                                        Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                                                                    }
                                                                                }
                                                                            }
                                                                        }
                                                                    }
                                                                }
                                                                Transform.position(0.01674976944923401, 0.002313171047717333, -0.008981633000075817).rotation_quat(-0.0612194649875164, -0.08708399534225464, -0.36263346672058105, 0.9258323907852173).scale(1.0, 1.0, 1.0) {
                                                                    name = "J_Bip_L_Thumb1"
                                                                    BoneRestPose.translation(0.01674976944923401, 0.002313171047717333, -0.008981633000075817)
                                                                    Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                                                        name = "viz:J_Bip_L_Thumb1"
                                                                        SignalRouteUpward.new("update_transform", "transform") {
                                                                            name = "route_upward:viz:J_Bip_L_Thumb1"
                                                                        }
                                                                        overlay {
                                                                            name = "viz_overlay:J_Bip_L_Thumb1"
                                                                            Renderable.cube() {
                                                                                name = "viz_box:J_Bip_L_Thumb1"
                                                                                Raycastable.enabled()
                                                                                Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                                                Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                                                            }
                                                                        }
                                                                    }
                                                                    Transform.position(0.00000003166496753692627, 0.04574253782629967, -0.000000021595042198896408).rotation_quat(0.00716137932613492, 0.012972678057849407, 0.021510086953639984, 0.9996588230133057).scale(1.0, 1.0000001192092896, 1.0) {
                                                                        name = "J_Bip_L_Thumb2"
                                                                        BoneRestPose.translation(0.00000003166496753692627, 0.04574253782629967, -0.000000021595042198896408)
                                                                        Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                                                            name = "viz:J_Bip_L_Thumb2"
                                                                            SignalRouteUpward.new("update_transform", "transform") {
                                                                                name = "route_upward:viz:J_Bip_L_Thumb2"
                                                                            }
                                                                            overlay {
                                                                                name = "viz_overlay:J_Bip_L_Thumb2"
                                                                                Renderable.cube() {
                                                                                    name = "viz_box:J_Bip_L_Thumb2"
                                                                                    Raycastable.enabled()
                                                                                    Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                                                    Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                                                                }
                                                                            }
                                                                        }
                                                                        Transform.position(-0.000000005587935447692871, 0.028096850961446762, -0.00000005948822945356369).rotation_quat(0.000000004190952473948073, -0.000000003725290298461914, 0.000000010186341548035216, 1.0).scale(1.0, 0.9999999403953552, 0.9999999403953552) {
                                                                            name = "J_Bip_L_Thumb3"
                                                                            BoneRestPose.translation(-0.000000005587935447692871, 0.028096850961446762, -0.00000005948822945356369)
                                                                            Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                                                                name = "viz:J_Bip_L_Thumb3"
                                                                                SignalRouteUpward.new("update_transform", "transform") {
                                                                                    name = "route_upward:viz:J_Bip_L_Thumb3"
                                                                                }
                                                                                overlay {
                                                                                    name = "viz_overlay:J_Bip_L_Thumb3"
                                                                                    Renderable.cube() {
                                                                                        name = "viz_box:J_Bip_L_Thumb3"
                                                                                        Raycastable.enabled()
                                                                                        Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                                                        Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                                                                    }
                                                                                }
                                                                            }
                                                                        }
                                                                    }
                                                                }
                                                                Transform.position(0.000491374172270298, 0.018994593992829323, -0.001427265116944909).rotation_quat(0.4670275151729584, 0.4935111403465271, 0.4935110807418823, 0.5429353713989258).scale(1.0000001192092896, 1.0, 1.0) {
                                                                    name = "J_Bip_L_Hand_collider_0.001"
                                                                    BoneRestPose.translation(0.000491374172270298, 0.018994593992829323, -0.001427265116944909)
                                                                    Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                                                        name = "viz:J_Bip_L_Hand_collider_0.001"
                                                                        SignalRouteUpward.new("update_transform", "transform") {
                                                                            name = "route_upward:viz:J_Bip_L_Hand_collider_0.001"
                                                                        }
                                                                        overlay {
                                                                            name = "viz_overlay:J_Bip_L_Hand_collider_0.001"
                                                                            Renderable.cube() {
                                                                                name = "viz_box:J_Bip_L_Hand_collider_0.001"
                                                                                Raycastable.enabled()
                                                                                Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                                                Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                                                            }
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                            Transform.position(-0.0000000020372681319713593, -0.00000000009640643838793039, -0.00000005415870418801205).rotation_quat(0.49954158067703247, 0.5004521012306213, 0.5004519820213318, 0.4995536208152771).scale(1.0, 1.0000001192092896, 0.9999998211860657) {
                                                                name = "J_Bip_L_LowerArm_collider_0.001"
                                                                BoneRestPose.translation(-0.0000000020372681319713593, -0.00000000009640643838793039, -0.00000005415870418801205)
                                                                Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                                                    name = "viz:J_Bip_L_LowerArm_collider_0.001"
                                                                    SignalRouteUpward.new("update_transform", "transform") {
                                                                        name = "route_upward:viz:J_Bip_L_LowerArm_collider_0.001"
                                                                    }
                                                                    overlay {
                                                                        name = "viz_overlay:J_Bip_L_LowerArm_collider_0.001"
                                                                        Renderable.cube() {
                                                                            name = "viz_box:J_Bip_L_LowerArm_collider_0.001"
                                                                            Raycastable.enabled()
                                                                            Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                                            Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                            Transform.position(-0.00008618022548034787, 0.04763630032539368, -0.0000006285773679337581).rotation_quat(0.49954158067703247, 0.5004521012306213, 0.5004519820213318, 0.4995536208152771).scale(1.0, 1.0000001192092896, 0.9999998211860657) {
                                                                name = "J_Bip_L_LowerArm_collider_1.001"
                                                                BoneRestPose.translation(-0.00008618022548034787, 0.04763630032539368, -0.0000006285773679337581)
                                                                Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                                                    name = "viz:J_Bip_L_LowerArm_collider_1.001"
                                                                    SignalRouteUpward.new("update_transform", "transform") {
                                                                        name = "route_upward:viz:J_Bip_L_LowerArm_collider_1.001"
                                                                    }
                                                                    overlay {
                                                                        name = "viz_overlay:J_Bip_L_LowerArm_collider_1.001"
                                                                        Renderable.cube() {
                                                                            name = "viz_box:J_Bip_L_LowerArm_collider_1.001"
                                                                            Raycastable.enabled()
                                                                            Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                                            Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                            Transform.position(-0.0001723585883155465, 0.0952727198600769, -0.0000012029969411742059).rotation_quat(0.49954158067703247, 0.5004521012306213, 0.5004519820213318, 0.4995536208152771).scale(1.0, 1.0000001192092896, 0.9999998211860657) {
                                                                name = "J_Bip_L_LowerArm_collider_2.001"
                                                                BoneRestPose.translation(-0.0001723585883155465, 0.0952727198600769, -0.0000012029969411742059)
                                                                Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                                                    name = "viz:J_Bip_L_LowerArm_collider_2.001"
                                                                    SignalRouteUpward.new("update_transform", "transform") {
                                                                        name = "route_upward:viz:J_Bip_L_LowerArm_collider_2.001"
                                                                    }
                                                                    overlay {
                                                                        name = "viz_overlay:J_Bip_L_LowerArm_collider_2.001"
                                                                        Renderable.cube() {
                                                                            name = "viz_box:J_Bip_L_LowerArm_collider_2.001"
                                                                            Raycastable.enabled()
                                                                            Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                                            Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                            Transform.position(-0.0002585368347354233, 0.14290904998779297, -0.0000018966248944707331).rotation_quat(0.49954158067703247, 0.5004521012306213, 0.5004519820213318, 0.4995536208152771).scale(1.0, 1.0000001192092896, 0.9999998211860657) {
                                                                name = "J_Bip_L_LowerArm_collider_3.001"
                                                                BoneRestPose.translation(-0.0002585368347354233, 0.14290904998779297, -0.0000018966248944707331)
                                                                Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                                                    name = "viz:J_Bip_L_LowerArm_collider_3.001"
                                                                    SignalRouteUpward.new("update_transform", "transform") {
                                                                        name = "route_upward:viz:J_Bip_L_LowerArm_collider_3.001"
                                                                    }
                                                                    overlay {
                                                                        name = "viz_overlay:J_Bip_L_LowerArm_collider_3.001"
                                                                        Renderable.cube() {
                                                                            name = "viz_box:J_Bip_L_LowerArm_collider_3.001"
                                                                            Raycastable.enabled()
                                                                            Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                                            Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                        }
                                                        Transform.position(-0.000000001548543071550057, 0.000000017571986532516348, -0.009527319110929966).rotation_quat(0.5000000596046448, 0.5, 0.5, 0.5).scale(1.0000001192092896, 1.0000001192092896, 0.9999998807907104) {
                                                            name = "J_Bip_L_UpperArm_collider_0.001"
                                                            BoneRestPose.translation(-0.000000001548543071550057, 0.000000017571986532516348, -0.009527319110929966)
                                                            Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                                                name = "viz:J_Bip_L_UpperArm_collider_0.001"
                                                                SignalRouteUpward.new("update_transform", "transform") {
                                                                    name = "route_upward:viz:J_Bip_L_UpperArm_collider_0.001"
                                                                }
                                                                overlay {
                                                                    name = "viz_overlay:J_Bip_L_UpperArm_collider_0.001"
                                                                    Renderable.cube() {
                                                                        name = "viz_box:J_Bip_L_UpperArm_collider_0.001"
                                                                        Raycastable.enabled()
                                                                        Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                                        Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                                                    }
                                                                }
                                                            }
                                                        }
                                                        Transform.position(0.00000000004927169783286445, 0.07145465165376663, -0.009527318179607391).rotation_quat(0.5000000596046448, 0.5, 0.5, 0.5).scale(1.0000001192092896, 1.0000001192092896, 0.9999998807907104) {
                                                            name = "J_Bip_L_UpperArm_collider_1.001"
                                                            BoneRestPose.translation(0.00000000004927169783286445, 0.07145465165376663, -0.009527318179607391)
                                                            Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                                                name = "viz:J_Bip_L_UpperArm_collider_1.001"
                                                                SignalRouteUpward.new("update_transform", "transform") {
                                                                    name = "route_upward:viz:J_Bip_L_UpperArm_collider_1.001"
                                                                }
                                                                overlay {
                                                                    name = "viz_overlay:J_Bip_L_UpperArm_collider_1.001"
                                                                    Renderable.cube() {
                                                                        name = "viz_box:J_Bip_L_UpperArm_collider_1.001"
                                                                        Raycastable.enabled()
                                                                        Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                                        Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                                                    }
                                                                }
                                                            }
                                                        }
                                                        Transform.position(0.000000003509734725071212, 0.14290936291217804, -0.009527317248284817).rotation_quat(0.5000000596046448, 0.5, 0.5, 0.5).scale(1.0000001192092896, 1.0000001192092896, 0.9999998807907104) {
                                                            name = "J_Bip_L_UpperArm_collider_2.001"
                                                            BoneRestPose.translation(0.000000003509734725071212, 0.14290936291217804, -0.009527317248284817)
                                                            Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                                                name = "viz:J_Bip_L_UpperArm_collider_2.001"
                                                                SignalRouteUpward.new("update_transform", "transform") {
                                                                    name = "route_upward:viz:J_Bip_L_UpperArm_collider_2.001"
                                                                }
                                                                overlay {
                                                                    name = "viz_overlay:J_Bip_L_UpperArm_collider_2.001"
                                                                    Renderable.cube() {
                                                                        name = "viz_box:J_Bip_L_UpperArm_collider_2.001"
                                                                        Raycastable.enabled()
                                                                        Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                                        Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                                Transform.position(-0.021677924320101738, 0.10984349995851517, 0.0006475239642895758).rotation_quat(-0.5069340467453003, 0.42019742727279663, 0.5593377351760864, 0.5035804510116577).scale(1.0, 1.0, 1.0) {
                                                    name = "J_Bip_R_Shoulder"
                                                    BoneRestPose.translation(-0.021677924320101738, 0.10984349995851517, 0.0006475239642895758)
                                                    Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                                        name = "viz:J_Bip_R_Shoulder"
                                                        SignalRouteUpward.new("update_transform", "transform") {
                                                            name = "route_upward:viz:J_Bip_R_Shoulder"
                                                        }
                                                        overlay {
                                                            name = "viz_overlay:J_Bip_R_Shoulder"
                                                            Renderable.cube() {
                                                                name = "viz_box:J_Bip_R_Shoulder"
                                                                Raycastable.enabled()
                                                                Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                                Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                                            }
                                                        }
                                                    }
                                                    Transform.position(-0.000000015599653124809265, 0.07559477537870407, 0.0000000651925802230835).rotation_quat(0.1414700597524643, -0.6540139317512512, -0.5402553081512451, -0.5102705955505371).scale(0.9999999403953552, 0.9999999403953552, 1.0) {
                                                        name = "J_Bip_R_UpperArm"
                                                        BoneRestPose.translation(-0.000000015599653124809265, 0.07559477537870407, 0.0000000651925802230835)
                                                        Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                                            name = "viz:J_Bip_R_UpperArm"
                                                            SignalRouteUpward.new("update_transform", "transform") {
                                                                name = "route_upward:viz:J_Bip_R_UpperArm"
                                                            }
                                                            overlay {
                                                                name = "viz_overlay:J_Bip_R_UpperArm"
                                                                Renderable.cube() {
                                                                    name = "viz_box:J_Bip_R_UpperArm"
                                                                    Raycastable.enabled()
                                                                    Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                                    Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                                                }
                                                            }
                                                        }
                                                        Transform.position(-0.0026809247210621834, 0.11260025203227997, -0.08594035357236862).rotation_quat(0.10203304886817932, -0.10203301161527634, 0.015921805053949356, 0.989406406879425).scale(0.9999999403953552, 0.9999999403953552, 1.0) {
                                                            name = "J_Sec_R_TopsUpperArmInside_01"
                                                            BoneRestPose.translation(-0.0026809247210621834, 0.11260025203227997, -0.08594035357236862)
                                                            Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                                                name = "viz:J_Sec_R_TopsUpperArmInside_01"
                                                                SignalRouteUpward.new("update_transform", "transform") {
                                                                    name = "route_upward:viz:J_Sec_R_TopsUpperArmInside_01"
                                                                }
                                                                overlay {
                                                                    name = "viz_overlay:J_Sec_R_TopsUpperArmInside_01"
                                                                    Renderable.cube() {
                                                                        name = "viz_box:J_Sec_R_TopsUpperArmInside_01"
                                                                        Raycastable.enabled()
                                                                        Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                                        Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                                                    }
                                                                }
                                                            }
                                                            Transform.position(0.000000013737007975578308, 0.1125369518995285, -0.000000027706846594810486).rotation_quat(-0.000000039814043617525385, 0.000000034691765904426575, -0.000000029103832233090543, 1.0).scale(1.0, 0.9999999403953552, 1.0) {
                                                                name = "J_Sec_R_TopsUpperArmInside_end_01"
                                                                BoneRestPose.translation(0.000000013737007975578308, 0.1125369518995285, -0.000000027706846594810486)
                                                                Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                                                    name = "viz:J_Sec_R_TopsUpperArmInside_end_01"
                                                                    SignalRouteUpward.new("update_transform", "transform") {
                                                                        name = "route_upward:viz:J_Sec_R_TopsUpperArmInside_end_01"
                                                                    }
                                                                    overlay {
                                                                        name = "viz_overlay:J_Sec_R_TopsUpperArmInside_end_01"
                                                                        Renderable.cube() {
                                                                            name = "viz_box:J_Sec_R_TopsUpperArmInside_end_01"
                                                                            Raycastable.enabled()
                                                                            Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                                            Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                        }
                                                        Transform.position(0.004093822091817856, 0.007631546352058649, 0.03519690781831741).rotation_quat(0.11011027544736862, -0.11011029034852982, 0.023006245493888855, 0.9875333905220032).scale(1.0, 0.9999999403953552, 0.9999999403953552) {
                                                            name = "J_Sec_R_TopsUpperArmOutside_01"
                                                            BoneRestPose.translation(0.004093822091817856, 0.007631546352058649, 0.03519690781831741)
                                                            Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                                                name = "viz:J_Sec_R_TopsUpperArmOutside_01"
                                                                SignalRouteUpward.new("update_transform", "transform") {
                                                                    name = "route_upward:viz:J_Sec_R_TopsUpperArmOutside_01"
                                                                }
                                                                overlay {
                                                                    name = "viz_overlay:J_Sec_R_TopsUpperArmOutside_01"
                                                                    Renderable.cube() {
                                                                        name = "viz_box:J_Sec_R_TopsUpperArmOutside_01"
                                                                        Raycastable.enabled()
                                                                        Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                                        Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                                                    }
                                                                }
                                                            }
                                                            Transform.position(-0.00000003864988684654236, 0.1799262911081314, 0.00000006798654794692993).rotation_quat(0.000000006752089554140639, 0.000000017578713595867157, 0.00000007776543498039246, 1.0).scale(1.0, 0.9999998807907104, 0.9999999403953552) {
                                                                name = "J_Sec_R_TopsUpperArmOutside_end_01"
                                                                BoneRestPose.translation(-0.00000003864988684654236, 0.1799262911081314, 0.00000006798654794692993)
                                                                Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                                                    name = "viz:J_Sec_R_TopsUpperArmOutside_end_01"
                                                                    SignalRouteUpward.new("update_transform", "transform") {
                                                                        name = "route_upward:viz:J_Sec_R_TopsUpperArmOutside_end_01"
                                                                    }
                                                                    overlay {
                                                                        name = "viz_overlay:J_Sec_R_TopsUpperArmOutside_end_01"
                                                                        Renderable.cube() {
                                                                            name = "viz_box:J_Sec_R_TopsUpperArmOutside_end_01"
                                                                            Raycastable.enabled()
                                                                            Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                                            Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                        }
                                                        Transform.position(0.000000000017502443938610668, 0.21951666474342346, 0.000000005626346499809642).rotation_quat(-0.4693409502506256, 0.1355542242527008, 0.6648620367050171, 0.5650688409805298).scale(1.0, 0.9999999403953552, 1.0) {
                                                            name = "J_Bip_R_LowerArm"
                                                            BoneRestPose.translation(0.000000000017502443938610668, 0.21951666474342346, 0.000000005626346499809642)
                                                            Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                                                name = "viz:J_Bip_R_LowerArm"
                                                                SignalRouteUpward.new("update_transform", "transform") {
                                                                    name = "route_upward:viz:J_Bip_R_LowerArm"
                                                                }
                                                                overlay {
                                                                    name = "viz_overlay:J_Bip_R_LowerArm"
                                                                    Renderable.cube() {
                                                                        name = "viz_box:J_Bip_R_LowerArm"
                                                                        Raycastable.enabled()
                                                                        Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                                        Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                                                    }
                                                                }
                                                            }
                                                            Transform.position(0.0000000005020410753786564, 0.2079743891954422, 0.00000006581359457413782).rotation_quat(0.3268823027610779, -0.8753644824028015, -0.23776771128177643, 0.26523900032043457).scale(0.9999999403953552, 1.0, 1.0) {
                                                                name = "J_Bip_R_Hand"
                                                                BoneRestPose.translation(0.0000000005020410753786564, 0.2079743891954422, 0.00000006581359457413782)
                                                                Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                                                    name = "viz:J_Bip_R_Hand"
                                                                    SignalRouteUpward.new("update_transform", "transform") {
                                                                        name = "route_upward:viz:J_Bip_R_Hand"
                                                                    }
                                                                    overlay {
                                                                        name = "viz_overlay:J_Bip_R_Hand"
                                                                        Renderable.cube() {
                                                                            name = "viz_box:J_Bip_R_Hand"
                                                                            Raycastable.enabled()
                                                                            Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                                            Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                                                        }
                                                                    }
                                                                }
                                                                Transform.position(-0.02067512646317482, 0.06335747987031937, 0.004095223732292652).rotation_quat(-0.038334719836711884, 0.03757307678461075, 0.001451677642762661, 0.9985572695732117).scale(1.0000001192092896, 0.9999998807907104, 1.0) {
                                                                    name = "J_Bip_R_Index1"
                                                                    BoneRestPose.translation(-0.02067512646317482, 0.06335747987031937, 0.004095223732292652)
                                                                    Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                                                        name = "viz:J_Bip_R_Index1"
                                                                        SignalRouteUpward.new("update_transform", "transform") {
                                                                            name = "route_upward:viz:J_Bip_R_Index1"
                                                                        }
                                                                        overlay {
                                                                            name = "viz_overlay:J_Bip_R_Index1"
                                                                            Renderable.cube() {
                                                                                name = "viz_box:J_Bip_R_Index1"
                                                                                Raycastable.enabled()
                                                                                Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                                                Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                                                            }
                                                                        }
                                                                    }
                                                                    Transform.position(0.0000000009472387318965048, 0.03220309317111969, 0.000000003118396207923979).rotation_quat(-0.014766010455787182, 0.01447267085313797, -0.010086135007441044, 0.9997353553771973).scale(0.9999999403953552, 0.9999998807907104, 1.0) {
                                                                        name = "J_Bip_R_Index2"
                                                                        BoneRestPose.translation(0.0000000009472387318965048, 0.03220309317111969, 0.000000003118396207923979)
                                                                        Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                                                            name = "viz:J_Bip_R_Index2"
                                                                            SignalRouteUpward.new("update_transform", "transform") {
                                                                                name = "route_upward:viz:J_Bip_R_Index2"
                                                                            }
                                                                            overlay {
                                                                                name = "viz_overlay:J_Bip_R_Index2"
                                                                                Renderable.cube() {
                                                                                    name = "viz_box:J_Bip_R_Index2"
                                                                                    Raycastable.enabled()
                                                                                    Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                                                    Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                                                                }
                                                                            }
                                                                        }
                                                                        Transform.position(-0.0000000023283064365386963, 0.019828367978334427, 0.00000003680179361253977).rotation_quat(0.0000000004947651732756242, -0.000000004423782229423523, -0.00000007241033017635345, 1.0).scale(1.0, 0.9999999403953552, 0.9999999403953552) {
                                                                            name = "J_Bip_R_Index3"
                                                                            BoneRestPose.translation(-0.0000000023283064365386963, 0.019828367978334427, 0.00000003680179361253977)
                                                                            Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                                                                name = "viz:J_Bip_R_Index3"
                                                                                SignalRouteUpward.new("update_transform", "transform") {
                                                                                    name = "route_upward:viz:J_Bip_R_Index3"
                                                                                }
                                                                                overlay {
                                                                                    name = "viz_overlay:J_Bip_R_Index3"
                                                                                    Renderable.cube() {
                                                                                        name = "viz_box:J_Bip_R_Index3"
                                                                                        Raycastable.enabled()
                                                                                        Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                                                        Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                                                                    }
                                                                                }
                                                                            }
                                                                        }
                                                                    }
                                                                }
                                                                Transform.position(0.028109658509492874, 0.06010697782039642, 0.000669483095407486).rotation_quat(-0.0379520058631897, 0.03795204311609268, 0.011470513418316841, 0.9984927177429199).scale(1.0, 0.9999999403953552, 1.0) {
                                                                    name = "J_Bip_R_Little1"
                                                                    BoneRestPose.translation(0.028109658509492874, 0.06010697782039642, 0.000669483095407486)
                                                                    Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                                                        name = "viz:J_Bip_R_Little1"
                                                                        SignalRouteUpward.new("update_transform", "transform") {
                                                                            name = "route_upward:viz:J_Bip_R_Little1"
                                                                        }
                                                                        overlay {
                                                                            name = "viz_overlay:J_Bip_R_Little1"
                                                                            Renderable.cube() {
                                                                                name = "viz_box:J_Bip_R_Little1"
                                                                                Raycastable.enabled()
                                                                                Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                                                Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                                                            }
                                                                        }
                                                                    }
                                                                    Transform.position(0.000000004810942755284486, 0.030435634776949883, 0.000000025683860371827905).rotation_quat(-0.0000019557778614398558, 0.000001963229578905157, 0.00000009132114087151422, 1.0).scale(1.0, 1.0, 1.0) {
                                                                        name = "J_Bip_R_Little2"
                                                                        BoneRestPose.translation(0.000000004810942755284486, 0.030435634776949883, 0.000000025683860371827905)
                                                                        Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                                                            name = "viz:J_Bip_R_Little2"
                                                                            SignalRouteUpward.new("update_transform", "transform") {
                                                                                name = "route_upward:viz:J_Bip_R_Little2"
                                                                            }
                                                                            overlay {
                                                                                name = "viz_overlay:J_Bip_R_Little2"
                                                                                Renderable.cube() {
                                                                                    name = "viz_box:J_Bip_R_Little2"
                                                                                    Raycastable.enabled()
                                                                                    Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                                                    Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                                                                }
                                                                            }
                                                                        }
                                                                        Transform.position(0.000000003466094744553061, 0.017541248351335526, 0.000000014877969078952447).rotation_quat(0.00000000000002884727572376351, -0.0000000000004666000658765102, -0.00000010618668966344558, 1.0).scale(1.0, 1.0, 1.0) {
                                                                            name = "J_Bip_R_Little3"
                                                                            BoneRestPose.translation(0.000000003466094744553061, 0.017541248351335526, 0.000000014877969078952447)
                                                                            Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                                                                name = "viz:J_Bip_R_Little3"
                                                                                SignalRouteUpward.new("update_transform", "transform") {
                                                                                    name = "route_upward:viz:J_Bip_R_Little3"
                                                                                }
                                                                                overlay {
                                                                                    name = "viz_overlay:J_Bip_R_Little3"
                                                                                    Renderable.cube() {
                                                                                        name = "viz_box:J_Bip_R_Little3"
                                                                                        Raycastable.enabled()
                                                                                        Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                                                        Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                                                                    }
                                                                                }
                                                                            }
                                                                        }
                                                                    }
                                                                }
                                                                Transform.position(-0.00314424280077219, 0.06457097083330154, 0.0026807086542248726).rotation_quat(-0.03795568272471428, 0.03795560449361801, 0.011470085941255093, 0.9984924793243408).scale(1.0, 0.9999999403953552, 0.9999999403953552) {
                                                                    name = "J_Bip_R_Middle1"
                                                                    BoneRestPose.translation(-0.00314424280077219, 0.06457097083330154, 0.0026807086542248726)
                                                                    Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                                                        name = "viz:J_Bip_R_Middle1"
                                                                        SignalRouteUpward.new("update_transform", "transform") {
                                                                            name = "route_upward:viz:J_Bip_R_Middle1"
                                                                        }
                                                                        overlay {
                                                                            name = "viz_overlay:J_Bip_R_Middle1"
                                                                            Renderable.cube() {
                                                                                name = "viz_box:J_Bip_R_Middle1"
                                                                                Raycastable.enabled()
                                                                                Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                                                Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                                                            }
                                                                        }
                                                                    }
                                                                    Transform.position(0.0000000011322924819978653, 0.03586988151073456, 0.000000019181214838681626).rotation_quat(0.0000016689300537109375, -0.000001665204990786151, 0.00000028371152893669205, 1.0).scale(1.0, 1.0, 1.0) {
                                                                        name = "J_Bip_R_Middle2"
                                                                        BoneRestPose.translation(0.0000000011322924819978653, 0.03586988151073456, 0.000000019181214838681626)
                                                                        Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                                                            name = "viz:J_Bip_R_Middle2"
                                                                            SignalRouteUpward.new("update_transform", "transform") {
                                                                                name = "route_upward:viz:J_Bip_R_Middle2"
                                                                            }
                                                                            overlay {
                                                                                name = "viz_overlay:J_Bip_R_Middle2"
                                                                                Renderable.cube() {
                                                                                    name = "viz_box:J_Bip_R_Middle2"
                                                                                    Raycastable.enabled()
                                                                                    Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                                                    Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                                                                }
                                                                            }
                                                                        }
                                                                        Transform.position(0.0000000007533578760821058, 0.02212880179286003, 0.000000006101448679629584).rotation_quat(-0.0000000000002282618809679865, 0.00000000000022517870814503516, -0.00000000000005689886224940349, 1.0).scale(1.0, 1.0, 1.0) {
                                                                            name = "J_Bip_R_Middle3"
                                                                            BoneRestPose.translation(0.0000000007533578760821058, 0.02212880179286003, 0.000000006101448679629584)
                                                                            Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                                                                name = "viz:J_Bip_R_Middle3"
                                                                                SignalRouteUpward.new("update_transform", "transform") {
                                                                                    name = "route_upward:viz:J_Bip_R_Middle3"
                                                                                }
                                                                                overlay {
                                                                                    name = "viz_overlay:J_Bip_R_Middle3"
                                                                                    Renderable.cube() {
                                                                                        name = "viz_box:J_Bip_R_Middle3"
                                                                                        Raycastable.enabled()
                                                                                        Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                                                        Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                                                                    }
                                                                                }
                                                                            }
                                                                        }
                                                                    }
                                                                }
                                                                Transform.position(0.012459428049623966, 0.06415428966283798, 0.0015368720050901175).rotation_quat(-0.03795398399233818, 0.03795398026704788, 0.01147033367305994, 0.9984925985336304).scale(1.0000001192092896, 1.0, 1.0) {
                                                                    name = "J_Bip_R_Ring1"
                                                                    BoneRestPose.translation(0.012459428049623966, 0.06415428966283798, 0.0015368720050901175)
                                                                    Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                                                        name = "viz:J_Bip_R_Ring1"
                                                                        SignalRouteUpward.new("update_transform", "transform") {
                                                                            name = "route_upward:viz:J_Bip_R_Ring1"
                                                                        }
                                                                        overlay {
                                                                            name = "viz_overlay:J_Bip_R_Ring1"
                                                                            Renderable.cube() {
                                                                                name = "viz_box:J_Bip_R_Ring1"
                                                                                Raycastable.enabled()
                                                                                Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                                                Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                                                            }
                                                                        }
                                                                    }
                                                                    Transform.position(-0.0000000027092745824575104, 0.03327404335141182, 0.00000012545133643016015).rotation_quat(-0.000000003725198371995475, -0.000000011175857572709447, -0.0000003382672559837374, 1.0).scale(1.0, 1.0, 1.0) {
                                                                        name = "J_Bip_R_Ring2"
                                                                        BoneRestPose.translation(-0.0000000027092745824575104, 0.03327404335141182, 0.00000012545133643016015)
                                                                        Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                                                            name = "viz:J_Bip_R_Ring2"
                                                                            SignalRouteUpward.new("update_transform", "transform") {
                                                                                name = "route_upward:viz:J_Bip_R_Ring2"
                                                                            }
                                                                            overlay {
                                                                                name = "viz_overlay:J_Bip_R_Ring2"
                                                                                Renderable.cube() {
                                                                                    name = "viz_box:J_Bip_R_Ring2"
                                                                                    Raycastable.enabled()
                                                                                    Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                                                    Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                                                                }
                                                                            }
                                                                        }
                                                                        Transform.position(0.00000000007342102620322422, 0.019202357158064842, 0.0000001292182503220829).rotation_quat(0.00000000000000011101763054641595, 0.0000000000000013711910169380095, 0.0000000000000034000474250027012, 1.0).scale(1.0, 1.0, 1.0) {
                                                                            name = "J_Bip_R_Ring3"
                                                                            BoneRestPose.translation(0.00000000007342102620322422, 0.019202357158064842, 0.0000001292182503220829)
                                                                            Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                                                                name = "viz:J_Bip_R_Ring3"
                                                                                SignalRouteUpward.new("update_transform", "transform") {
                                                                                    name = "route_upward:viz:J_Bip_R_Ring3"
                                                                                }
                                                                                overlay {
                                                                                    name = "viz_overlay:J_Bip_R_Ring3"
                                                                                    Renderable.cube() {
                                                                                        name = "viz_box:J_Bip_R_Ring3"
                                                                                        Raycastable.enabled()
                                                                                        Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                                                        Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                                                                    }
                                                                                }
                                                                            }
                                                                        }
                                                                    }
                                                                }
                                                                Transform.position(-0.01674976944923401, 0.0023132069036364555, -0.008981507271528244).rotation_quat(-0.061219457536935806, 0.08708398044109344, 0.36263346672058105, 0.9258323311805725).scale(0.9999999403953552, 0.9999998807907104, 1.0000001192092896) {
                                                                    name = "J_Bip_R_Thumb1"
                                                                    BoneRestPose.translation(-0.01674976944923401, 0.0023132069036364555, -0.008981507271528244)
                                                                    Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                                                        name = "viz:J_Bip_R_Thumb1"
                                                                        SignalRouteUpward.new("update_transform", "transform") {
                                                                            name = "route_upward:viz:J_Bip_R_Thumb1"
                                                                        }
                                                                        overlay {
                                                                            name = "viz_overlay:J_Bip_R_Thumb1"
                                                                            Renderable.cube() {
                                                                                name = "viz_box:J_Bip_R_Thumb1"
                                                                                Raycastable.enabled()
                                                                                Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                                                Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                                                            }
                                                                        }
                                                                    }
                                                                    Transform.position(-0.000000007450580596923828, 0.04574252665042877, -0.000000051048118621110916).rotation_quat(0.007161366753280163, -0.012972640804946423, -0.02151000127196312, 0.9996588230133057).scale(1.0000001192092896, 1.0, 0.9999999403953552) {
                                                                        name = "J_Bip_R_Thumb2"
                                                                        BoneRestPose.translation(-0.000000007450580596923828, 0.04574252665042877, -0.000000051048118621110916)
                                                                        Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                                                            name = "viz:J_Bip_R_Thumb2"
                                                                            SignalRouteUpward.new("update_transform", "transform") {
                                                                                name = "route_upward:viz:J_Bip_R_Thumb2"
                                                                            }
                                                                            overlay {
                                                                                name = "viz_overlay:J_Bip_R_Thumb2"
                                                                                Renderable.cube() {
                                                                                    name = "viz_box:J_Bip_R_Thumb2"
                                                                                    Raycastable.enabled()
                                                                                    Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                                                    Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                                                                }
                                                                            }
                                                                        }
                                                                        Transform.position(-0.0000000391155481338501, 0.02809685468673706, 0.00000005948822945356369).rotation_quat(-0.0000000013969838619232178, -0.000000007450580596923828, 0.000000042957260859566304, 1.0).scale(0.9999999403953552, 0.9999999403953552, 1.0) {
                                                                            name = "J_Bip_R_Thumb3"
                                                                            BoneRestPose.translation(-0.0000000391155481338501, 0.02809685468673706, 0.00000005948822945356369)
                                                                            Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                                                                name = "viz:J_Bip_R_Thumb3"
                                                                                SignalRouteUpward.new("update_transform", "transform") {
                                                                                    name = "route_upward:viz:J_Bip_R_Thumb3"
                                                                                }
                                                                                overlay {
                                                                                    name = "viz_overlay:J_Bip_R_Thumb3"
                                                                                    Renderable.cube() {
                                                                                        name = "viz_box:J_Bip_R_Thumb3"
                                                                                        Raycastable.enabled()
                                                                                        Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                                                        Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                                                                    }
                                                                                }
                                                                            }
                                                                        }
                                                                    }
                                                                }
                                                                Transform.position(-0.0004913611337542534, 0.018994741141796112, -0.0014273935230448842).rotation_quat(0.46702757477760315, -0.4935111105442047, -0.49351105093955994, 0.5429354310035706).scale(1.0, 1.0000001192092896, 1.0000001192092896) {
                                                                    name = "J_Bip_R_Hand_collider_0.001"
                                                                    BoneRestPose.translation(-0.0004913611337542534, 0.018994741141796112, -0.0014273935230448842)
                                                                    Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                                                        name = "viz:J_Bip_R_Hand_collider_0.001"
                                                                        SignalRouteUpward.new("update_transform", "transform") {
                                                                            name = "route_upward:viz:J_Bip_R_Hand_collider_0.001"
                                                                        }
                                                                        overlay {
                                                                            name = "viz_overlay:J_Bip_R_Hand_collider_0.001"
                                                                            Renderable.cube() {
                                                                                name = "viz_box:J_Bip_R_Hand_collider_0.001"
                                                                                Raycastable.enabled()
                                                                                Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                                                Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                                                            }
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                            Transform.position(-0.00000000261206878349185, -0.000000016509147826582193, 0.00000018502373677620199).rotation_quat(0.49954143166542053, -0.5004522204399109, -0.5004520416259766, 0.49955350160598755).scale(1.0, 1.000000238418579, 0.9999999403953552) {
                                                                name = "J_Bip_R_LowerArm_collider_0.001"
                                                                BoneRestPose.translation(-0.00000000261206878349185, -0.000000016509147826582193, 0.00000018502373677620199)
                                                                Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                                                    name = "viz:J_Bip_R_LowerArm_collider_0.001"
                                                                    SignalRouteUpward.new("update_transform", "transform") {
                                                                        name = "route_upward:viz:J_Bip_R_LowerArm_collider_0.001"
                                                                    }
                                                                    overlay {
                                                                        name = "viz_overlay:J_Bip_R_LowerArm_collider_0.001"
                                                                        Renderable.cube() {
                                                                            name = "viz_box:J_Bip_R_LowerArm_collider_0.001"
                                                                            Raycastable.enabled()
                                                                            Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                                            Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                            Transform.position(0.00008618413266958669, 0.04763628542423248, -0.0000003893926532327896).rotation_quat(0.49954143166542053, -0.5004522204399109, -0.5004520416259766, 0.49955350160598755).scale(1.0, 1.000000238418579, 0.9999999403953552) {
                                                                name = "J_Bip_R_LowerArm_collider_1.001"
                                                                BoneRestPose.translation(0.00008618413266958669, 0.04763628542423248, -0.0000003893926532327896)
                                                                Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                                                    name = "viz:J_Bip_R_LowerArm_collider_1.001"
                                                                    SignalRouteUpward.new("update_transform", "transform") {
                                                                        name = "route_upward:viz:J_Bip_R_LowerArm_collider_1.001"
                                                                    }
                                                                    overlay {
                                                                        name = "viz_overlay:J_Bip_R_LowerArm_collider_1.001"
                                                                        Renderable.cube() {
                                                                            name = "viz_box:J_Bip_R_LowerArm_collider_1.001"
                                                                            Raycastable.enabled()
                                                                            Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                                            Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                            Transform.position(0.0001723711029626429, 0.0952727347612381, -0.0000009638104074838338).rotation_quat(0.49954143166542053, -0.5004522204399109, -0.5004520416259766, 0.49955350160598755).scale(1.0, 1.000000238418579, 0.9999999403953552) {
                                                                name = "J_Bip_R_LowerArm_collider_2.001"
                                                                BoneRestPose.translation(0.0001723711029626429, 0.0952727347612381, -0.0000009638104074838338)
                                                                Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                                                    name = "viz:J_Bip_R_LowerArm_collider_2.001"
                                                                    SignalRouteUpward.new("update_transform", "transform") {
                                                                        name = "route_upward:viz:J_Bip_R_LowerArm_collider_2.001"
                                                                    }
                                                                    overlay {
                                                                        name = "viz_overlay:J_Bip_R_LowerArm_collider_2.001"
                                                                        Renderable.cube() {
                                                                            name = "viz_box:J_Bip_R_LowerArm_collider_2.001"
                                                                            Raycastable.enabled()
                                                                            Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                                            Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                            Transform.position(0.00025855598505586386, 0.14290903508663177, -0.0000016574360870436067).rotation_quat(0.49954143166542053, -0.5004522204399109, -0.5004520416259766, 0.49955350160598755).scale(1.0, 1.000000238418579, 0.9999999403953552) {
                                                                name = "J_Bip_R_LowerArm_collider_3.001"
                                                                BoneRestPose.translation(0.00025855598505586386, 0.14290903508663177, -0.0000016574360870436067)
                                                                Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                                                    name = "viz:J_Bip_R_LowerArm_collider_3.001"
                                                                    SignalRouteUpward.new("update_transform", "transform") {
                                                                        name = "route_upward:viz:J_Bip_R_LowerArm_collider_3.001"
                                                                    }
                                                                    overlay {
                                                                        name = "viz_overlay:J_Bip_R_LowerArm_collider_3.001"
                                                                        Renderable.cube() {
                                                                            name = "viz_box:J_Bip_R_LowerArm_collider_3.001"
                                                                            Raycastable.enabled()
                                                                            Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                                            Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                        }
                                                        Transform.position(-0.0000000008176206378607276, -0.000000013684663180413281, -0.00952732004225254).rotation_quat(0.5, -0.5000000596046448, -0.5, 0.5).scale(1.0, 1.000000238418579, 1.0) {
                                                            name = "J_Bip_R_UpperArm_collider_0.001"
                                                            BoneRestPose.translation(-0.0000000008176206378607276, -0.000000013684663180413281, -0.00952732004225254)
                                                            Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                                                name = "viz:J_Bip_R_UpperArm_collider_0.001"
                                                                SignalRouteUpward.new("update_transform", "transform") {
                                                                    name = "route_upward:viz:J_Bip_R_UpperArm_collider_0.001"
                                                                }
                                                                overlay {
                                                                    name = "viz_overlay:J_Bip_R_UpperArm_collider_0.001"
                                                                    Renderable.cube() {
                                                                        name = "viz_box:J_Bip_R_UpperArm_collider_0.001"
                                                                        Raycastable.enabled()
                                                                        Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                                        Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                                                    }
                                                                }
                                                            }
                                                        }
                                                        Transform.position(0.000000004772353179305355, 0.07145462185144424, -0.00952732004225254).rotation_quat(0.5, -0.5000000596046448, -0.5, 0.5).scale(1.0, 1.000000238418579, 1.0) {
                                                            name = "J_Bip_R_UpperArm_collider_1.001"
                                                            BoneRestPose.translation(0.000000004772353179305355, 0.07145462185144424, -0.00952732004225254)
                                                            Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                                                name = "viz:J_Bip_R_UpperArm_collider_1.001"
                                                                SignalRouteUpward.new("update_transform", "transform") {
                                                                    name = "route_upward:viz:J_Bip_R_UpperArm_collider_1.001"
                                                                }
                                                                overlay {
                                                                    name = "viz_overlay:J_Bip_R_UpperArm_collider_1.001"
                                                                    Renderable.cube() {
                                                                        name = "viz_box:J_Bip_R_UpperArm_collider_1.001"
                                                                        Raycastable.enabled()
                                                                        Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                                        Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                                                    }
                                                                }
                                                            }
                                                        }
                                                        Transform.position(0.000000010362331437363537, 0.14290930330753326, -0.00952732004225254).rotation_quat(0.5, -0.5000000596046448, -0.5, 0.5).scale(1.0, 1.000000238418579, 1.0) {
                                                            name = "J_Bip_R_UpperArm_collider_2.001"
                                                            BoneRestPose.translation(0.000000010362331437363537, 0.14290930330753326, -0.00952732004225254)
                                                            Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                                                name = "viz:J_Bip_R_UpperArm_collider_2.001"
                                                                SignalRouteUpward.new("update_transform", "transform") {
                                                                    name = "route_upward:viz:J_Bip_R_UpperArm_collider_2.001"
                                                                }
                                                                overlay {
                                                                    name = "viz_overlay:J_Bip_R_UpperArm_collider_2.001"
                                                                    Renderable.cube() {
                                                                        name = "viz_box:J_Bip_R_UpperArm_collider_2.001"
                                                                        Raycastable.enabled()
                                                                        Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                                        Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                                Transform.position(-0.00000000023193841292012962, -0.002653376664966345, 0.009150399826467037).rotation_quat(0.14063535630702972, -0.000000010765523583700087, -0.000000010765528912770606, 0.9900614619255066).scale(1.0, 1.000000238418579, 1.0) {
                                                    name = "J_Bip_C_UpperChest_collider_0.001"
                                                    BoneRestPose.translation(-0.00000000023193841292012962, -0.002653376664966345, 0.009150399826467037)
                                                    Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                                        name = "viz:J_Bip_C_UpperChest_collider_0.001"
                                                        SignalRouteUpward.new("update_transform", "transform") {
                                                            name = "route_upward:viz:J_Bip_C_UpperChest_collider_0.001"
                                                        }
                                                        overlay {
                                                            name = "viz_overlay:J_Bip_C_UpperChest_collider_0.001"
                                                            Renderable.cube() {
                                                                name = "viz_box:J_Bip_C_UpperChest_collider_0.001"
                                                                Raycastable.enabled()
                                                                Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                                Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                                            }
                                                        }
                                                    }
                                                }
                                                Transform.position(0.047636426985263824, 0.062130920588970184, 0.008094812743365765).rotation_quat(0.14063535630702972, -0.000000010765523583700087, -0.000000010765528912770606, 0.9900614619255066).scale(1.0, 1.000000238418579, 1.0) {
                                                    name = "J_Bip_C_UpperChest_collider_1.001"
                                                    BoneRestPose.translation(0.047636426985263824, 0.062130920588970184, 0.008094812743365765)
                                                    Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                                        name = "viz:J_Bip_C_UpperChest_collider_1.001"
                                                        SignalRouteUpward.new("update_transform", "transform") {
                                                            name = "route_upward:viz:J_Bip_C_UpperChest_collider_1.001"
                                                        }
                                                        overlay {
                                                            name = "viz_overlay:J_Bip_C_UpperChest_collider_1.001"
                                                            Renderable.cube() {
                                                                name = "viz_box:J_Bip_C_UpperChest_collider_1.001"
                                                                Raycastable.enabled()
                                                                Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                                Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                                            }
                                                        }
                                                    }
                                                }
                                                Transform.position(-0.04763643443584442, 0.06213092431426048, 0.008094814606010914).rotation_quat(0.14063535630702972, -0.000000010765523583700087, -0.000000010765528912770606, 0.9900614619255066).scale(1.0, 1.000000238418579, 1.0) {
                                                    name = "J_Bip_C_UpperChest_collider_2.001"
                                                    BoneRestPose.translation(-0.04763643443584442, 0.06213092431426048, 0.008094814606010914)
                                                    Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                                        name = "viz:J_Bip_C_UpperChest_collider_2.001"
                                                        SignalRouteUpward.new("update_transform", "transform") {
                                                            name = "route_upward:viz:J_Bip_C_UpperChest_collider_2.001"
                                                        }
                                                        overlay {
                                                            name = "viz_overlay:J_Bip_C_UpperChest_collider_2.001"
                                                            Renderable.cube() {
                                                                name = "viz_box:J_Bip_C_UpperChest_collider_2.001"
                                                                Raycastable.enabled()
                                                                Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                                Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                                            }
                                                        }
                                                    }
                                                }
                                                overlay {
                                                    Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.02500000037252903, 0.02500000037252903, 0.02500000037252903) {
                                                        Renderable.cube() {
                                                            Color.rgba(0.20000000298023224, 0.20000000298023224, 0.8500000238418579, 0.8999999761581421)
                                                            Emissive.on()
                                                            Raycastable.enabled()
                                                            Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        Transform.position(0.00000000000000042674197509029455, -0.00000021324376575648785, 0.0000000009313225746154785).rotation_quat(-0.013489632867276669, 0.00000000119543186460902, 0.00000000119543186460902, 0.9999090433120728).scale(1.0, 1.0, 1.0) {
                                            name = "J_Bip_C_Spine_collider_0.001"
                                            BoneRestPose.translation(0.00000000000000042674197509029455, -0.00000021324376575648785, 0.0000000009313225746154785)
                                            Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                                name = "viz:J_Bip_C_Spine_collider_0.001"
                                                SignalRouteUpward.new("update_transform", "transform") {
                                                    name = "route_upward:viz:J_Bip_C_Spine_collider_0.001"
                                                }
                                                overlay {
                                                    name = "viz_overlay:J_Bip_C_Spine_collider_0.001"
                                                    Renderable.cube() {
                                                        name = "viz_box:J_Bip_C_Spine_collider_0.001"
                                                        Raycastable.enabled()
                                                        Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                        Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    Transform.position(0.07455381751060486, -0.039524298161268234, 0.005741197150200605).rotation_quat(0.9944294691085815, -0.000000002996247028264065, -0.000000028267857388186712, 0.10540422052145004).scale(1.0, 1.0, 1.0000030994415283) {
                                        name = "J_Bip_L_UpperLeg"
                                        BoneRestPose.translation(0.07455381751060486, -0.039524298161268234, 0.005741197150200605)
                                        Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                            name = "viz:J_Bip_L_UpperLeg"
                                            SignalRouteUpward.new("update_transform", "transform") {
                                                name = "route_upward:viz:J_Bip_L_UpperLeg"
                                            }
                                            overlay {
                                                name = "viz_overlay:J_Bip_L_UpperLeg"
                                                Renderable.cube() {
                                                    name = "viz_box:J_Bip_L_UpperLeg"
                                                    Raycastable.enabled()
                                                    Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                    Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                                }
                                            }
                                        }
                                        Transform.position(0.00022701379202771932, -0.0011518255341798067, 0.11812969297170639).rotation_quat(0.0722658783197403, 0.0451790913939476, -0.04630981758236885, 0.9952847957611084).scale(0.9999999403953552, 1.0, 1.0000001192092896) {
                                            name = "J_Sec_L_TopsUpperLegBack_01"
                                            BoneRestPose.translation(0.00022701379202771932, -0.0011518255341798067, 0.11812969297170639)
                                            Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                                name = "viz:J_Sec_L_TopsUpperLegBack_01"
                                                SignalRouteUpward.new("update_transform", "transform") {
                                                    name = "route_upward:viz:J_Sec_L_TopsUpperLegBack_01"
                                                }
                                                overlay {
                                                    name = "viz_overlay:J_Sec_L_TopsUpperLegBack_01"
                                                    Renderable.cube() {
                                                        name = "viz_box:J_Sec_L_TopsUpperLegBack_01"
                                                        Raycastable.enabled()
                                                        Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                        Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                                    }
                                                }
                                            }
                                        }
                                        Transform.position(0.00022699774126522243, 0.012814415618777275, -0.13200128078460693).rotation_quat(-0.01065906323492527, 0.04539693146944046, -0.04653294384479523, 0.9978277683258057).scale(0.9999999403953552, 0.9999999403953552, 1.0) {
                                            name = "J_Sec_L_TopsUpperLegFront_01"
                                            BoneRestPose.translation(0.00022699774126522243, 0.012814415618777275, -0.13200128078460693)
                                            Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                                name = "viz:J_Sec_L_TopsUpperLegFront_01"
                                                SignalRouteUpward.new("update_transform", "transform") {
                                                    name = "route_upward:viz:J_Sec_L_TopsUpperLegFront_01"
                                                }
                                                overlay {
                                                    name = "viz_overlay:J_Sec_L_TopsUpperLegFront_01"
                                                    Renderable.cube() {
                                                        name = "viz_box:J_Sec_L_TopsUpperLegFront_01"
                                                        Raycastable.enabled()
                                                        Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                        Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                                    }
                                                }
                                            }
                                        }
                                        Transform.position(0.10917824506759644, 0.009220153093338013, -0.0003699928929563612).rotation_quat(0.009821973741054535, 0.10618488490581512, -0.10884226113557816, 0.9883226752281189).scale(1.0, 0.9999999403953552, 1.0) {
                                            name = "J_Sec_L_TopsUpperLegSide_01"
                                            BoneRestPose.translation(0.10917824506759644, 0.009220153093338013, -0.0003699928929563612)
                                            Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                                name = "viz:J_Sec_L_TopsUpperLegSide_01"
                                                SignalRouteUpward.new("update_transform", "transform") {
                                                    name = "route_upward:viz:J_Sec_L_TopsUpperLegSide_01"
                                                }
                                                overlay {
                                                    name = "viz_overlay:J_Sec_L_TopsUpperLegSide_01"
                                                    Renderable.cube() {
                                                        name = "viz_box:J_Sec_L_TopsUpperLegSide_01"
                                                        Raycastable.enabled()
                                                        Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                        Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                                    }
                                                }
                                            }
                                        }
                                        Transform.position(0.0000000007298387449949928, 0.40624988079071045, -0.0000000008052016831072706).rotation_quat(0.01623724028468132, -0.00004646868910640478, 0.00004763673496199772, 0.9998681545257568).scale(1.0, 1.0, 1.0) {
                                            name = "J_Bip_L_LowerLeg"
                                            BoneRestPose.translation(0.0000000007298387449949928, 0.40624988079071045, -0.0000000008052016831072706)
                                            Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                                name = "viz:J_Bip_L_LowerLeg"
                                                SignalRouteUpward.new("update_transform", "transform") {
                                                    name = "route_upward:viz:J_Bip_L_LowerLeg"
                                                }
                                                overlay {
                                                    name = "viz_overlay:J_Bip_L_LowerLeg"
                                                    Renderable.cube() {
                                                        name = "viz_box:J_Bip_L_LowerLeg"
                                                        Raycastable.enabled()
                                                        Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                        Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                                    }
                                                }
                                            }
                                            Transform.position(-0.000000003344212018419057, 0.4619063436985016, -0.00000000034424374462105334).rotation_quat(-0.515365719795227, 0.00006419036071747541, -0.000018003323930315673, 0.8569703698158264).scale(1.0, 0.9999999403953552, 0.9999999403953552) {
                                                name = "J_Bip_L_Foot"
                                                BoneRestPose.translation(-0.000000003344212018419057, 0.4619063436985016, -0.00000000034424374462105334)
                                                Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                                    name = "viz:J_Bip_L_Foot"
                                                    SignalRouteUpward.new("update_transform", "transform") {
                                                        name = "route_upward:viz:J_Bip_L_Foot"
                                                    }
                                                    overlay {
                                                        name = "viz_overlay:J_Bip_L_Foot"
                                                        Renderable.cube() {
                                                            name = "viz_box:J_Bip_L_Foot"
                                                            Raycastable.enabled()
                                                            Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                            Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                                        }
                                                    }
                                                }
                                                Transform.position(-0.0000000031450344550876252, 0.12113362550735474, 0.000000012663969428672317).rotation_quat(-0.000000014901159417490817, 0.00000000004662669902955052, 0.0000000000055573319383828146, 1.0).scale(1.0, 1.0000001192092896, 1.0000001192092896) {
                                                    name = "J_Bip_L_ToeBase"
                                                    BoneRestPose.translation(-0.0000000031450344550876252, 0.12113362550735474, 0.000000012663969428672317)
                                                    Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                                        name = "viz:J_Bip_L_ToeBase"
                                                        SignalRouteUpward.new("update_transform", "transform") {
                                                            name = "route_upward:viz:J_Bip_L_ToeBase"
                                                        }
                                                        overlay {
                                                            name = "viz_overlay:J_Bip_L_ToeBase"
                                                            Renderable.cube() {
                                                                name = "viz_box:J_Bip_L_ToeBase"
                                                                Raycastable.enabled()
                                                                Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                                Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        Transform.position(-0.00000000214441064905202, -0.000000033949469013805356, -0.000000003730137088098218).rotation_quat(0.9999236464500427, -0.00000001248547931709254, -0.000000018047806804588618, 0.012357138097286224).scale(1.0, 1.0000001192092896, 0.9999970197677612) {
                                            name = "J_Bip_L_UpperLeg_collider_0.001"
                                            BoneRestPose.translation(-0.00000000214441064905202, -0.000000033949469013805356, -0.000000003730137088098218)
                                            Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                                name = "viz:J_Bip_L_UpperLeg_collider_0.001"
                                                SignalRouteUpward.new("update_transform", "transform") {
                                                    name = "route_upward:viz:J_Bip_L_UpperLeg_collider_0.001"
                                                }
                                                overlay {
                                                    name = "viz_overlay:J_Bip_L_UpperLeg_collider_0.001"
                                                    Renderable.cube() {
                                                        name = "viz_box:J_Bip_L_UpperLeg_collider_0.001"
                                                        Raycastable.enabled()
                                                        Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                        Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                                    }
                                                }
                                            }
                                        }
                                        Transform.position(0.0000000006592276724504131, 0.11429240554571152, -0.0028252797201275826).rotation_quat(0.9999236464500427, -0.00000001248547931709254, -0.000000018047806804588618, 0.012357138097286224).scale(1.0, 1.0000001192092896, 0.9999970197677612) {
                                            name = "J_Bip_L_UpperLeg_collider_1.001"
                                            BoneRestPose.translation(0.0000000006592276724504131, 0.11429240554571152, -0.0028252797201275826)
                                            Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                                name = "viz:J_Bip_L_UpperLeg_collider_1.001"
                                                SignalRouteUpward.new("update_transform", "transform") {
                                                    name = "route_upward:viz:J_Bip_L_UpperLeg_collider_1.001"
                                                }
                                                overlay {
                                                    name = "viz_overlay:J_Bip_L_UpperLeg_collider_1.001"
                                                    Renderable.cube() {
                                                        name = "viz_box:J_Bip_L_UpperLeg_collider_1.001"
                                                        Raycastable.enabled()
                                                        Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                        Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                                    }
                                                }
                                            }
                                        }
                                        Transform.position(0.0000000029955951053040053, 0.20953622460365295, -0.005179678555577993).rotation_quat(0.9999236464500427, -0.00000001248547931709254, -0.000000018047806804588618, 0.012357138097286224).scale(1.0, 1.0000001192092896, 0.9999970197677612) {
                                            name = "J_Bip_L_UpperLeg_collider_2.001"
                                            BoneRestPose.translation(0.0000000029955951053040053, 0.20953622460365295, -0.005179678555577993)
                                            Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                                name = "viz:J_Bip_L_UpperLeg_collider_2.001"
                                                SignalRouteUpward.new("update_transform", "transform") {
                                                    name = "route_upward:viz:J_Bip_L_UpperLeg_collider_2.001"
                                                }
                                                overlay {
                                                    name = "viz_overlay:J_Bip_L_UpperLeg_collider_2.001"
                                                    Renderable.cube() {
                                                        name = "viz_box:J_Bip_L_UpperLeg_collider_2.001"
                                                        Raycastable.enabled()
                                                        Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                        Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    Transform.position(-0.07455381751060486, -0.039524298161268234, 0.005741210654377937).rotation_quat(0.9944294691085815, 0.00000005737459929378019, 0.000000019309457144345288, 0.10540422052145004).scale(1.0, 1.0, 1.0000033378601074) {
                                        name = "J_Bip_R_UpperLeg"
                                        BoneRestPose.translation(-0.07455381751060486, -0.039524298161268234, 0.005741210654377937)
                                        Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                            name = "viz:J_Bip_R_UpperLeg"
                                            SignalRouteUpward.new("update_transform", "transform") {
                                                name = "route_upward:viz:J_Bip_R_UpperLeg"
                                            }
                                            overlay {
                                                name = "viz_overlay:J_Bip_R_UpperLeg"
                                                Renderable.cube() {
                                                    name = "viz_box:J_Bip_R_UpperLeg"
                                                    Raycastable.enabled()
                                                    Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                    Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                                }
                                            }
                                        }
                                        Transform.position(-0.00022697461827192456, -0.0011514686048030853, 0.11812964081764221).rotation_quat(0.0722658634185791, -0.04517919197678566, 0.046309903264045715, 0.9952847957611084).scale(1.0, 1.0, 1.0000001192092896) {
                                            name = "J_Sec_R_TopsUpperLegBack_01"
                                            BoneRestPose.translation(-0.00022697461827192456, -0.0011514686048030853, 0.11812964081764221)
                                            Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                                name = "viz:J_Sec_R_TopsUpperLegBack_01"
                                                SignalRouteUpward.new("update_transform", "transform") {
                                                    name = "route_upward:viz:J_Sec_R_TopsUpperLegBack_01"
                                                }
                                                overlay {
                                                    name = "viz_overlay:J_Sec_R_TopsUpperLegBack_01"
                                                    Renderable.cube() {
                                                        name = "viz_box:J_Sec_R_TopsUpperLegBack_01"
                                                        Raycastable.enabled()
                                                        Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                        Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                                    }
                                                }
                                            }
                                        }
                                        Transform.position(-0.0002269057440571487, 0.01281464472413063, -0.13200126588344574).rotation_quat(-0.010659180581569672, -0.04539692401885986, 0.04653295874595642, 0.9978277087211609).scale(1.0, 0.9999998807907104, 1.0) {
                                            name = "J_Sec_R_TopsUpperLegFront_01"
                                            BoneRestPose.translation(-0.0002269057440571487, 0.01281464472413063, -0.13200126588344574)
                                            Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                                name = "viz:J_Sec_R_TopsUpperLegFront_01"
                                                SignalRouteUpward.new("update_transform", "transform") {
                                                    name = "route_upward:viz:J_Sec_R_TopsUpperLegFront_01"
                                                }
                                                overlay {
                                                    name = "viz_overlay:J_Sec_R_TopsUpperLegFront_01"
                                                    Renderable.cube() {
                                                        name = "viz_box:J_Sec_R_TopsUpperLegFront_01"
                                                        Raycastable.enabled()
                                                        Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                        Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                                    }
                                                }
                                            }
                                        }
                                        Transform.position(-0.10917817801237106, 0.009220379404723644, -0.00037002970930188894).rotation_quat(0.009821966290473938, -0.10618500411510468, 0.10884230583906174, 0.9883226752281189).scale(0.9999999403953552, 0.9999998807907104, 1.0) {
                                            name = "J_Sec_R_TopsUpperLegSide_01"
                                            BoneRestPose.translation(-0.10917817801237106, 0.009220379404723644, -0.00037002970930188894)
                                            Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                                name = "viz:J_Sec_R_TopsUpperLegSide_01"
                                                SignalRouteUpward.new("update_transform", "transform") {
                                                    name = "route_upward:viz:J_Sec_R_TopsUpperLegSide_01"
                                                }
                                                overlay {
                                                    name = "viz_overlay:J_Sec_R_TopsUpperLegSide_01"
                                                    Renderable.cube() {
                                                        name = "viz_box:J_Sec_R_TopsUpperLegSide_01"
                                                        Raycastable.enabled()
                                                        Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                        Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                                    }
                                                }
                                            }
                                        }
                                        Transform.position(0.000000011097267105242281, 0.40624988079071045, 0.00000000025248736434946295).rotation_quat(0.016237253323197365, 0.00004645128137781285, -0.00004760684896609746, 0.9998681545257568).scale(1.0, 1.0, 1.0) {
                                            name = "J_Bip_R_LowerLeg"
                                            BoneRestPose.translation(0.000000011097267105242281, 0.40624988079071045, 0.00000000025248736434946295)
                                            Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                                name = "viz:J_Bip_R_LowerLeg"
                                                SignalRouteUpward.new("update_transform", "transform") {
                                                    name = "route_upward:viz:J_Bip_R_LowerLeg"
                                                }
                                                overlay {
                                                    name = "viz_overlay:J_Bip_R_LowerLeg"
                                                    Renderable.cube() {
                                                        name = "viz_box:J_Bip_R_LowerLeg"
                                                        Raycastable.enabled()
                                                        Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                        Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                                    }
                                                }
                                            }
                                            Transform.position(0.0000000033455762604717165, 0.4619063436985016, -0.0000000026702764444053173).rotation_quat(-0.5153656601905823, -0.00006403344741556793, 0.000017837193809100427, 0.8569703698158264).scale(1.0, 0.9999998807907104, 0.9999999403953552) {
                                                name = "J_Bip_R_Foot"
                                                BoneRestPose.translation(0.0000000033455762604717165, 0.4619063436985016, -0.0000000026702764444053173)
                                                Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                                    name = "viz:J_Bip_R_Foot"
                                                    SignalRouteUpward.new("update_transform", "transform") {
                                                        name = "route_upward:viz:J_Bip_R_Foot"
                                                    }
                                                    overlay {
                                                        name = "viz_overlay:J_Bip_R_Foot"
                                                        Renderable.cube() {
                                                            name = "viz_box:J_Bip_R_Foot"
                                                            Raycastable.enabled()
                                                            Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                            Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                                        }
                                                    }
                                                }
                                                Transform.position(0.000000004331037750660016, 0.12113359570503235, -0.000000005233108169022671).rotation_quat(0.0, -0.00000000008801581685702331, -0.0000000000017275070263167436, 1.0).scale(1.0, 1.0, 1.0) {
                                                    name = "J_Bip_R_ToeBase"
                                                    BoneRestPose.translation(0.000000004331037750660016, 0.12113359570503235, -0.000000005233108169022671)
                                                    Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                                        name = "viz:J_Bip_R_ToeBase"
                                                        SignalRouteUpward.new("update_transform", "transform") {
                                                            name = "route_upward:viz:J_Bip_R_ToeBase"
                                                        }
                                                        overlay {
                                                            name = "viz_overlay:J_Bip_R_ToeBase"
                                                            Renderable.cube() {
                                                                name = "viz_box:J_Bip_R_ToeBase"
                                                                Raycastable.enabled()
                                                                Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                                Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        Transform.position(0.0000000156735939782493, 0.000000021017118712052252, -0.0000000020030617164934483).rotation_quat(0.9999236464500427, 0.00000004186784963167156, 0.00000003630376355090448, 0.012357138097286224).scale(1.0, 1.0000001192092896, 0.9999967813491821) {
                                            name = "J_Bip_R_UpperLeg_collider_0.001"
                                            BoneRestPose.translation(0.0000000156735939782493, 0.000000021017118712052252, -0.0000000020030617164934483)
                                            Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                                name = "viz:J_Bip_R_UpperLeg_collider_0.001"
                                                SignalRouteUpward.new("update_transform", "transform") {
                                                    name = "route_upward:viz:J_Bip_R_UpperLeg_collider_0.001"
                                                }
                                                overlay {
                                                    name = "viz_overlay:J_Bip_R_UpperLeg_collider_0.001"
                                                    Renderable.cube() {
                                                        name = "viz_box:J_Bip_R_UpperLeg_collider_0.001"
                                                        Raycastable.enabled()
                                                        Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                        Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                                    }
                                                }
                                            }
                                        }
                                        Transform.position(0.000000013654450903288762, 0.11429240554571152, -0.002825272036716342).rotation_quat(0.9999236464500427, 0.00000004186784963167156, 0.00000003630376355090448, 0.012357138097286224).scale(1.0, 1.0000001192092896, 0.9999967813491821) {
                                            name = "J_Bip_R_UpperLeg_collider_1.001"
                                            BoneRestPose.translation(0.000000013654450903288762, 0.11429240554571152, -0.002825272036716342)
                                            Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                                name = "viz:J_Bip_R_UpperLeg_collider_1.001"
                                                SignalRouteUpward.new("update_transform", "transform") {
                                                    name = "route_upward:viz:J_Bip_R_UpperLeg_collider_1.001"
                                                }
                                                overlay {
                                                    name = "viz_overlay:J_Bip_R_UpperLeg_collider_1.001"
                                                    Renderable.cube() {
                                                        name = "viz_box:J_Bip_R_UpperLeg_collider_1.001"
                                                        Raycastable.enabled()
                                                        Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                        Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                                    }
                                                }
                                            }
                                        }
                                        Transform.position(0.000000005763000299197074, 0.20953626930713654, -0.005179675295948982).rotation_quat(0.9999236464500427, 0.00000004186784963167156, 0.00000003630376355090448, 0.012357138097286224).scale(1.0, 1.0000001192092896, 0.9999967813491821) {
                                            name = "J_Bip_R_UpperLeg_collider_2.001"
                                            BoneRestPose.translation(0.000000005763000299197074, 0.20953626930713654, -0.005179675295948982)
                                            Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                                name = "viz:J_Bip_R_UpperLeg_collider_2.001"
                                                SignalRouteUpward.new("update_transform", "transform") {
                                                    name = "route_upward:viz:J_Bip_R_UpperLeg_collider_2.001"
                                                }
                                                overlay {
                                                    name = "viz_overlay:J_Bip_R_UpperLeg_collider_2.001"
                                                    Renderable.cube() {
                                                        name = "viz_box:J_Bip_R_UpperLeg_collider_2.001"
                                                        Raycastable.enabled()
                                                        Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                        Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            Transform.position(-0.0035162754356861115, 1.0131535530090332, -0.07173405587673187).rotation_quat(-0.5425776839256287, -0.4363461434841156, 0.35898181796073914, 0.6215654611587524).scale(1.0, 1.0, 0.9999999403953552) {
                                name = "J_Bip_C_Hips.002"
                                BoneRestPose.translation(-0.0035162754356861115, 1.0131535530090332, -0.07173405587673187)
                                Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                    name = "viz:J_Bip_C_Hips.002"
                                    SignalRouteUpward.new("update_transform", "transform") {
                                        name = "route_upward:viz:J_Bip_C_Hips.002"
                                    }
                                    overlay {
                                        name = "viz_overlay:J_Bip_C_Hips.002"
                                        Renderable.cube() {
                                            name = "viz_box:J_Bip_C_Hips.002"
                                            Raycastable.enabled()
                                            Color.rgba(1.0, 1.0, 1.0, 1.0)
                                            Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                        }
                                    }
                                }
                                Transform.position(-0.000000024301698431372643, 0.09696473181247711, 0.0000000050640664994716644).rotation_quat(0.09146814793348312, 0.00000005359836308116428, -0.2116624414920807, 0.9730532765388489).scale(1.0, 1.0, 1.0) {
                                    name = "J_Bip_C_Hips.003"
                                    BoneRestPose.translation(-0.000000024301698431372643, 0.09696473181247711, 0.0000000050640664994716644)
                                    Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                        name = "viz:J_Bip_C_Hips.003"
                                        SignalRouteUpward.new("update_transform", "transform") {
                                            name = "route_upward:viz:J_Bip_C_Hips.003"
                                        }
                                        overlay {
                                            name = "viz_overlay:J_Bip_C_Hips.003"
                                            Renderable.cube() {
                                                name = "viz_box:J_Bip_C_Hips.003"
                                                Raycastable.enabled()
                                                Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                            }
                                        }
                                    }
                                    Transform.position(0.00000003294553607702255, 0.10032026469707489, -0.00000000021827872842550278).rotation_quat(0.14072030782699585, 0.039676737040281296, -0.3642677068710327, 0.9197459816932678).scale(1.0000001192092896, 0.9999998211860657, 1.0) {
                                        name = "J_Bip_C_Hips.004"
                                        BoneRestPose.translation(0.00000003294553607702255, 0.10032026469707489, -0.00000000021827872842550278)
                                        Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                            name = "viz:J_Bip_C_Hips.004"
                                            SignalRouteUpward.new("update_transform", "transform") {
                                                name = "route_upward:viz:J_Bip_C_Hips.004"
                                            }
                                            overlay {
                                                name = "viz_overlay:J_Bip_C_Hips.004"
                                                Renderable.cube() {
                                                    name = "viz_box:J_Bip_C_Hips.004"
                                                    Raycastable.enabled()
                                                    Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                    Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                                }
                                            }
                                        }
                                        Transform.position(0.000000005005858838558197, 0.1325453817844391, -0.00000000651925802230835).rotation_quat(0.11026202142238617, 0.04826664924621582, -0.20581553876399994, 0.9711604118347168).scale(1.0000001192092896, 1.0, 1.0) {
                                            name = "J_Bip_C_Hips.005"
                                            BoneRestPose.translation(0.000000005005858838558197, 0.1325453817844391, -0.00000000651925802230835)
                                            Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                                name = "viz:J_Bip_C_Hips.005"
                                                SignalRouteUpward.new("update_transform", "transform") {
                                                    name = "route_upward:viz:J_Bip_C_Hips.005"
                                                }
                                                overlay {
                                                    name = "viz_overlay:J_Bip_C_Hips.005"
                                                    Renderable.cube() {
                                                        name = "viz_box:J_Bip_C_Hips.005"
                                                        Raycastable.enabled()
                                                        Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                        Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                                    }
                                                }
                                            }
                                            Transform.position(0.00000006891787052154541, 0.07893557846546173, -0.000000005122274160385132).rotation_quat(0.1884234994649887, 0.046660929918289185, -0.22987623512744904, 0.9536646604537964).scale(1.0, 1.0, 1.0) {
                                                name = "J_Bip_C_Hips.006"
                                                BoneRestPose.translation(0.00000006891787052154541, 0.07893557846546173, -0.000000005122274160385132)
                                                Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                                    name = "viz:J_Bip_C_Hips.006"
                                                    SignalRouteUpward.new("update_transform", "transform") {
                                                        name = "route_upward:viz:J_Bip_C_Hips.006"
                                                    }
                                                    overlay {
                                                        name = "viz_overlay:J_Bip_C_Hips.006"
                                                        Renderable.cube() {
                                                            name = "viz_box:J_Bip_C_Hips.006"
                                                            Raycastable.enabled()
                                                            Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                            Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
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
                IKChain.two_bone_ik([-1.0, 0.0, -1.0], true).weight(1.0)
                IKChain.two_bone_ik([1.0, 0.0, -1.0], true).weight(1.0)
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
            Transform.position(0.000000010490732726964325, -0.07999999821186066, 0.11999999731779099).rotation_quat(0.0, 1.0, 0.0, -0.00000004371138828673793).scale(1.0, 1.0, 1.0) {
                Transform.position(0.0, 0.0, 0.0).rotation_quat(-0.06361286342144012, 0.000000007718105266008024, 0.000000007718106154186444, 0.997974693775177).scale(1.0, 0.9999999403953552, 0.9999999403953552) {
                    name = "J_Bip_C_Head"
                    BoneRestPose.translation(0.000000000000002525757381022231, 0.07545606046915054, -0.000000005587935447692871)
                    Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                        name = "viz:J_Bip_C_Head"
                        SignalRouteUpward.new("update_transform", "transform") {
                            name = "route_upward:viz:J_Bip_C_Head"
                        }
                        overlay {
                            name = "viz_overlay:J_Bip_C_Head"
                            Renderable.cube() {
                                name = "viz_box:J_Bip_C_Head"
                                Raycastable.enabled()
                                Color.rgba(1.0, 1.0, 1.0, 1.0)
                                Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                            }
                        }
                    }
                    Transform.position(0.017693273723125458, 0.05699765682220459, 0.023379161953926086).rotation_quat(0.000000030908619663705394, -0.7071066498756409, -0.70710688829422, 0.00000003090861255827804).scale(1.0, 1.0, 1.0) {
                        name = "J_Adj_L_FaceEye"
                        BoneRestPose.translation(0.017693273723125458, 0.05699765682220459, 0.023379161953926086)
                        Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                            name = "viz:J_Adj_L_FaceEye"
                            SignalRouteUpward.new("update_transform", "transform") {
                                name = "route_upward:viz:J_Adj_L_FaceEye"
                            }
                            overlay {
                                name = "viz_overlay:J_Adj_L_FaceEye"
                                Renderable.cube() {
                                    name = "viz_box:J_Adj_L_FaceEye"
                                    Raycastable.enabled()
                                    Color.rgba(1.0, 1.0, 1.0, 1.0)
                                    Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                }
                            }
                        }
                    }
                    Transform.position(-0.017693255096673965, 0.05699765682220459, 0.023379186168313026).rotation_quat(0.000000030908619663705394, -0.7071066498756409, -0.70710688829422, 0.00000003090861255827804).scale(1.0, 1.0, 1.0) {
                        name = "J_Adj_R_FaceEye"
                        BoneRestPose.translation(-0.017693255096673965, 0.05699765682220459, 0.023379186168313026)
                        Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                            name = "viz:J_Adj_R_FaceEye"
                            SignalRouteUpward.new("update_transform", "transform") {
                                name = "route_upward:viz:J_Adj_R_FaceEye"
                            }
                            overlay {
                                name = "viz_overlay:J_Adj_R_FaceEye"
                                Renderable.cube() {
                                    name = "viz_box:J_Adj_R_FaceEye"
                                    Raycastable.enabled()
                                    Color.rgba(1.0, 1.0, 1.0, 1.0)
                                    Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                }
                            }
                        }
                    }
                    Transform.position(0.11174584925174713, 0.056130301207304, -0.015113974921405315).rotation_quat(-0.9896597266197205, -0.03876161202788353, -0.03876166045665741, 0.13254696130752563).scale(1.0000001192092896, 1.0, 1.0000005960464478) {
                        name = "J_Sec_Hair1_01"
                        BoneRestPose.translation(0.11174584925174713, 0.056130301207304, -0.015113974921405315)
                        Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                            name = "viz:J_Sec_Hair1_01"
                            SignalRouteUpward.new("update_transform", "transform") {
                                name = "route_upward:viz:J_Sec_Hair1_01"
                            }
                            overlay {
                                name = "viz_overlay:J_Sec_Hair1_01"
                                Renderable.cube() {
                                    name = "viz_box:J_Sec_Hair1_01"
                                    Raycastable.enabled()
                                    Color.rgba(1.0, 1.0, 1.0, 1.0)
                                    Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                }
                            }
                        }
                        Transform.position(0.00000001210719347000122, 0.07899798452854156, 0.0000000130385160446167).rotation_quat(0.09158118069171906, 0.021403353661298752, -0.01973990723490715, 0.9953718781471252).scale(1.0, 0.9999998807907104, 0.9999999403953552) {
                            name = "J_Sec_Hair2_01"
                            BoneRestPose.translation(0.00000001210719347000122, 0.07899798452854156, 0.0000000130385160446167)
                            Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                name = "viz:J_Sec_Hair2_01"
                                SignalRouteUpward.new("update_transform", "transform") {
                                    name = "route_upward:viz:J_Sec_Hair2_01"
                                }
                                overlay {
                                    name = "viz_overlay:J_Sec_Hair2_01"
                                    Renderable.cube() {
                                        name = "viz_box:J_Sec_Hair2_01"
                                        Raycastable.enabled()
                                        Color.rgba(1.0, 1.0, 1.0, 1.0)
                                        Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                    }
                                }
                            }
                            Transform.position(-0.0000000046566128730773926, 0.07881726324558258, -0.000000040046870708465576).rotation_quat(-0.20442073047161102, -0.08930875360965729, 0.11029597371816635, 0.9685406684875488).scale(0.9999999403953552, 0.9999999403953552, 0.9999999403953552) {
                                name = "J_Sec_Hair3_01"
                                BoneRestPose.translation(-0.0000000046566128730773926, 0.07881726324558258, -0.000000040046870708465576)
                                Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                    name = "viz:J_Sec_Hair3_01"
                                    SignalRouteUpward.new("update_transform", "transform") {
                                        name = "route_upward:viz:J_Sec_Hair3_01"
                                    }
                                    overlay {
                                        name = "viz_overlay:J_Sec_Hair3_01"
                                        Renderable.cube() {
                                            name = "viz_box:J_Sec_Hair3_01"
                                            Raycastable.enabled()
                                            Color.rgba(1.0, 1.0, 1.0, 1.0)
                                            Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Transform.position(-0.10024680942296982, 0.04904758557677269, 0.03316301852464676).rotation_quat(0.9972400665283203, 0.021478401497006416, 0.021478908136487007, 0.06774625927209854).scale(0.9999996423721313, 0.9999998807907104, 0.9999966025352478) {
                        name = "J_Sec_Hair1_02"
                        BoneRestPose.translation(-0.10024680942296982, 0.04904758557677269, 0.03316301852464676)
                        Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                            name = "viz:J_Sec_Hair1_02"
                            SignalRouteUpward.new("update_transform", "transform") {
                                name = "route_upward:viz:J_Sec_Hair1_02"
                            }
                            overlay {
                                name = "viz_overlay:J_Sec_Hair1_02"
                                Renderable.cube() {
                                    name = "viz_box:J_Sec_Hair1_02"
                                    Raycastable.enabled()
                                    Color.rgba(1.0, 1.0, 1.0, 1.0)
                                    Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                }
                            }
                        }
                        Transform.position(-0.000000006402842700481415, 0.08105187863111496, 0.00000000651925802230835).rotation_quat(-0.12128197401762009, 0.14568814635276794, -0.1320449858903885, 0.9729490280151367).scale(0.9999999403953552, 1.0, 1.0) {
                            name = "J_Sec_Hair2_02"
                            BoneRestPose.translation(-0.000000006402842700481415, 0.08105187863111496, 0.00000000651925802230835)
                            Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                name = "viz:J_Sec_Hair2_02"
                                SignalRouteUpward.new("update_transform", "transform") {
                                    name = "route_upward:viz:J_Sec_Hair2_02"
                                }
                                overlay {
                                    name = "viz_overlay:J_Sec_Hair2_02"
                                    Renderable.cube() {
                                        name = "viz_box:J_Sec_Hair2_02"
                                        Raycastable.enabled()
                                        Color.rgba(1.0, 1.0, 1.0, 1.0)
                                        Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                    }
                                }
                            }
                            Transform.position(-0.00000001955777406692505, 0.07654430717229843, 0.00000009499490261077881).rotation_quat(0.000000027939677238464355, -0.0000004023313522338867, 0.00000005960464477539063, 1.0).scale(1.0, 1.0, 1.0) {
                                name = "J_Sec_Hair3_02"
                                BoneRestPose.translation(-0.00000001955777406692505, 0.07654430717229843, 0.00000009499490261077881)
                                Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                    name = "viz:J_Sec_Hair3_02"
                                    SignalRouteUpward.new("update_transform", "transform") {
                                        name = "route_upward:viz:J_Sec_Hair3_02"
                                    }
                                    overlay {
                                        name = "viz_overlay:J_Sec_Hair3_02"
                                        Renderable.cube() {
                                            name = "viz_box:J_Sec_Hair3_02"
                                            Raycastable.enabled()
                                            Color.rgba(1.0, 1.0, 1.0, 1.0)
                                            Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Transform.position(0.09273407608270645, 0.04598581790924072, 0.0186495129019022).rotation_quat(0.9919909238815308, -0.03449639678001404, -0.03449678793549538, 0.11650791019201279).scale(0.9999998211860657, 0.9999999403953552, 0.9999968409538269) {
                        name = "J_Sec_Hair1_03"
                        BoneRestPose.translation(0.09273407608270645, 0.04598581790924072, 0.0186495129019022)
                        Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                            name = "viz:J_Sec_Hair1_03"
                            SignalRouteUpward.new("update_transform", "transform") {
                                name = "route_upward:viz:J_Sec_Hair1_03"
                            }
                            overlay {
                                name = "viz_overlay:J_Sec_Hair1_03"
                                Renderable.cube() {
                                    name = "viz_box:J_Sec_Hair1_03"
                                    Raycastable.enabled()
                                    Color.rgba(1.0, 1.0, 1.0, 1.0)
                                    Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                }
                            }
                        }
                        Transform.position(-0.0000000017462298274040222, 0.07334796339273453, -0.000000007450580596923828).rotation_quat(-0.01025098841637373, -0.20461712777614594, 0.16224278509616852, 0.9652482271194458).scale(0.9999999403953552, 0.9999999403953552, 1.0000001192092896) {
                            name = "J_Sec_Hair2_03"
                            BoneRestPose.translation(-0.0000000017462298274040222, 0.07334796339273453, -0.000000007450580596923828)
                            Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                name = "viz:J_Sec_Hair2_03"
                                SignalRouteUpward.new("update_transform", "transform") {
                                    name = "route_upward:viz:J_Sec_Hair2_03"
                                }
                                overlay {
                                    name = "viz_overlay:J_Sec_Hair2_03"
                                    Renderable.cube() {
                                        name = "viz_box:J_Sec_Hair2_03"
                                        Raycastable.enabled()
                                        Color.rgba(1.0, 1.0, 1.0, 1.0)
                                        Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                    }
                                }
                            }
                            Transform.position(-0.000000021420419216156006, 0.07032468914985657, 0.0).rotation_quat(0.00000003725290298461914, 0.0000006044283509254456, 0.00000002421438694000244, 1.0).scale(1.0, 1.0000001192092896, 1.0) {
                                name = "J_Sec_Hair3_03"
                                BoneRestPose.translation(-0.000000021420419216156006, 0.07032468914985657, 0.0)
                                Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                    name = "viz:J_Sec_Hair3_03"
                                    SignalRouteUpward.new("update_transform", "transform") {
                                        name = "route_upward:viz:J_Sec_Hair3_03"
                                    }
                                    overlay {
                                        name = "viz_overlay:J_Sec_Hair3_03"
                                        Renderable.cube() {
                                            name = "viz_box:J_Sec_Hair3_03"
                                            Raycastable.enabled()
                                            Color.rgba(1.0, 1.0, 1.0, 1.0)
                                            Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Transform.position(0.05775437504053116, 0.05876603722572327, -0.09727230668067932).rotation_quat(-0.9994255900382996, 0.015554415062069893, 0.015555133111774921, 0.02578064426779747).scale(1.0000009536743164, 1.0, 1.0000025033950806) {
                        name = "J_Sec_Hair1_04"
                        BoneRestPose.translation(0.05775437504053116, 0.05876603722572327, -0.09727230668067932)
                        Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                            name = "viz:J_Sec_Hair1_04"
                            SignalRouteUpward.new("update_transform", "transform") {
                                name = "route_upward:viz:J_Sec_Hair1_04"
                            }
                            overlay {
                                name = "viz_overlay:J_Sec_Hair1_04"
                                Renderable.cube() {
                                    name = "viz_box:J_Sec_Hair1_04"
                                    Raycastable.enabled()
                                    Color.rgba(1.0, 1.0, 1.0, 1.0)
                                    Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                }
                            }
                        }
                        Transform.position(0.000000004190951585769653, 0.07723179459571838, -0.0000000183936208486557).rotation_quat(0.0729152038693428, 0.038830164819955826, -0.043216198682785034, 0.9956445097923279).scale(1.0000001192092896, 1.0, 0.9999999403953552) {
                            name = "J_Sec_Hair2_04"
                            BoneRestPose.translation(0.000000004190951585769653, 0.07723179459571838, -0.0000000183936208486557)
                            Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                name = "viz:J_Sec_Hair2_04"
                                SignalRouteUpward.new("update_transform", "transform") {
                                    name = "route_upward:viz:J_Sec_Hair2_04"
                                }
                                overlay {
                                    name = "viz_overlay:J_Sec_Hair2_04"
                                    Renderable.cube() {
                                        name = "viz_box:J_Sec_Hair2_04"
                                        Raycastable.enabled()
                                        Color.rgba(1.0, 1.0, 1.0, 1.0)
                                        Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                    }
                                }
                            }
                            Transform.position(-0.000000013969838619232178, 0.0771825984120369, -0.000000047031790018081665).rotation_quat(-0.11955088376998901, -0.061812978237867355, 0.06859636306762695, 0.9885247945785522).scale(0.9999998211860657, 0.9999997615814209, 0.9999998807907104) {
                                name = "J_Sec_Hair3_04"
                                BoneRestPose.translation(-0.000000013969838619232178, 0.0771825984120369, -0.000000047031790018081665)
                                Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                    name = "viz:J_Sec_Hair3_04"
                                    SignalRouteUpward.new("update_transform", "transform") {
                                        name = "route_upward:viz:J_Sec_Hair3_04"
                                    }
                                    overlay {
                                        name = "viz_overlay:J_Sec_Hair3_04"
                                        Renderable.cube() {
                                            name = "viz_box:J_Sec_Hair3_04"
                                            Raycastable.enabled()
                                            Color.rgba(1.0, 1.0, 1.0, 1.0)
                                            Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Transform.position(-0.0028769492637366056, 0.06074896827340126, -0.11308134347200394).rotation_quat(-0.9991214275360107, 0.028724271804094315, 0.02872477099299431, 0.010306017473340034).scale(1.0000030994415283, 1.000000238418579, 1.0000003576278687) {
                        name = "J_Sec_Hair1_05"
                        BoneRestPose.translation(-0.0028769492637366056, 0.06074896827340126, -0.11308134347200394)
                        Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                            name = "viz:J_Sec_Hair1_05"
                            SignalRouteUpward.new("update_transform", "transform") {
                                name = "route_upward:viz:J_Sec_Hair1_05"
                            }
                            overlay {
                                name = "viz_overlay:J_Sec_Hair1_05"
                                Renderable.cube() {
                                    name = "viz_box:J_Sec_Hair1_05"
                                    Raycastable.enabled()
                                    Color.rgba(1.0, 1.0, 1.0, 1.0)
                                    Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                }
                            }
                        }
                        Transform.position(0.000000011175870895385742, 0.08037541806697845, 0.0000000034924596548080444).rotation_quat(0.08725152164697647, 0.004355656914412975, -0.009515614248812199, 0.9961313605308533).scale(1.0, 0.9999999403953552, 0.9999999403953552) {
                            name = "J_Sec_Hair2_05"
                            BoneRestPose.translation(0.000000011175870895385742, 0.08037541806697845, 0.0000000034924596548080444)
                            Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                name = "viz:J_Sec_Hair2_05"
                                SignalRouteUpward.new("update_transform", "transform") {
                                    name = "route_upward:viz:J_Sec_Hair2_05"
                                }
                                overlay {
                                    name = "viz_overlay:J_Sec_Hair2_05"
                                    Renderable.cube() {
                                        name = "viz_box:J_Sec_Hair2_05"
                                        Raycastable.enabled()
                                        Color.rgba(1.0, 1.0, 1.0, 1.0)
                                        Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                    }
                                }
                            }
                            Transform.position(-0.000000006984919309616089, 0.08031963557004929, -0.00000000628642737865448).rotation_quat(-0.17912085354328156, 0.05940227955579758, -0.06367727369070053, 0.9799654483795166).scale(0.9999999403953552, 0.9999998807907104, 1.0) {
                                name = "J_Sec_Hair3_05"
                                BoneRestPose.translation(-0.000000006984919309616089, 0.08031963557004929, -0.00000000628642737865448)
                                Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                    name = "viz:J_Sec_Hair3_05"
                                    SignalRouteUpward.new("update_transform", "transform") {
                                        name = "route_upward:viz:J_Sec_Hair3_05"
                                    }
                                    overlay {
                                        name = "viz_overlay:J_Sec_Hair3_05"
                                        Renderable.cube() {
                                            name = "viz_box:J_Sec_Hair3_05"
                                            Raycastable.enabled()
                                            Color.rgba(1.0, 1.0, 1.0, 1.0)
                                            Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Transform.position(-0.051171641796827316, 0.06461062282323837, -0.07558320462703705).rotation_quat(-0.9923526644706726, 0.04074593633413315, 0.04074477031826973, 0.10915955901145935).scale(0.9999990463256836, 0.9999998807907104, 0.9999948143959045) {
                        name = "J_Sec_Hair1_06"
                        BoneRestPose.translation(-0.051171641796827316, 0.06461062282323837, -0.07558320462703705)
                        Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                            name = "viz:J_Sec_Hair1_06"
                            SignalRouteUpward.new("update_transform", "transform") {
                                name = "route_upward:viz:J_Sec_Hair1_06"
                            }
                            overlay {
                                name = "viz_overlay:J_Sec_Hair1_06"
                                Renderable.cube() {
                                    name = "viz_box:J_Sec_Hair1_06"
                                    Raycastable.enabled()
                                    Color.rgba(1.0, 1.0, 1.0, 1.0)
                                    Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                }
                            }
                        }
                        Transform.position(0.0000000027939677238464355, 0.0797741711139679, 0.00000000605359673500061).rotation_quat(-0.06721168756484985, 0.03836575150489807, -0.04164793714880943, 0.9961305856704712).scale(0.9999998807907104, 0.9999998807907104, 0.9999998807907104) {
                            name = "J_Sec_Hair2_06"
                            BoneRestPose.translation(0.0000000027939677238464355, 0.0797741711139679, 0.00000000605359673500061)
                            Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                name = "viz:J_Sec_Hair2_06"
                                SignalRouteUpward.new("update_transform", "transform") {
                                    name = "route_upward:viz:J_Sec_Hair2_06"
                                }
                                overlay {
                                    name = "viz_overlay:J_Sec_Hair2_06"
                                    Renderable.cube() {
                                        name = "viz_box:J_Sec_Hair2_06"
                                        Raycastable.enabled()
                                        Color.rgba(1.0, 1.0, 1.0, 1.0)
                                        Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                    }
                                }
                            }
                            Transform.position(0.000000003958120942115784, 0.07999581098556519, 0.00000001968874130398035).rotation_quat(-0.0017642054008319974, 0.0010977956699207425, -0.0011921789264306426, 0.9999971389770508).scale(1.0, 1.0, 1.0000001192092896) {
                                name = "J_Sec_Hair3_06"
                                BoneRestPose.translation(0.000000003958120942115784, 0.07999581098556519, 0.00000001968874130398035)
                                Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                    name = "viz:J_Sec_Hair3_06"
                                    SignalRouteUpward.new("update_transform", "transform") {
                                        name = "route_upward:viz:J_Sec_Hair3_06"
                                    }
                                    overlay {
                                        name = "viz_overlay:J_Sec_Hair3_06"
                                        Renderable.cube() {
                                            name = "viz_box:J_Sec_Hair3_06"
                                            Raycastable.enabled()
                                            Color.rgba(1.0, 1.0, 1.0, 1.0)
                                            Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Transform.position(-0.11171482503414154, 0.056617267429828644, -0.015013906173408031).rotation_quat(-0.9901385307312012, 0.03865485638380051, 0.0386546328663826, 0.1289859116077423).scale(0.9999998211860657, 0.9999999403953552, 0.9999991655349731) {
                        name = "J_Sec_Hair1_07"
                        BoneRestPose.translation(-0.11171482503414154, 0.056617267429828644, -0.015013906173408031)
                        Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                            name = "viz:J_Sec_Hair1_07"
                            SignalRouteUpward.new("update_transform", "transform") {
                                name = "route_upward:viz:J_Sec_Hair1_07"
                            }
                            overlay {
                                name = "viz_overlay:J_Sec_Hair1_07"
                                Renderable.cube() {
                                    name = "viz_box:J_Sec_Hair1_07"
                                    Raycastable.enabled()
                                    Color.rgba(1.0, 1.0, 1.0, 1.0)
                                    Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                }
                            }
                        }
                        Transform.position(0.000000017695128917694092, 0.07887183874845505, -0.000000005587935447692871).rotation_quat(0.06676793843507767, -0.028905993327498436, 0.03157102316617966, 0.9968499541282654).scale(1.0, 1.0, 1.0) {
                            name = "J_Sec_Hair2_07"
                            BoneRestPose.translation(0.000000017695128917694092, 0.07887183874845505, -0.000000005587935447692871)
                            Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                name = "viz:J_Sec_Hair2_07"
                                SignalRouteUpward.new("update_transform", "transform") {
                                    name = "route_upward:viz:J_Sec_Hair2_07"
                                }
                                overlay {
                                    name = "viz_overlay:J_Sec_Hair2_07"
                                    Renderable.cube() {
                                        name = "viz_box:J_Sec_Hair2_07"
                                        Raycastable.enabled()
                                        Color.rgba(1.0, 1.0, 1.0, 1.0)
                                        Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                    }
                                }
                            }
                            Transform.position(0.000000014901161193847656, 0.07882347702980042, -0.000000027939677238464355).rotation_quat(-0.14141309261322021, 0.10468214750289917, -0.13196951150894165, 0.9755142331123352).scale(1.0000001192092896, 1.0000001192092896, 1.0000001192092896) {
                                name = "J_Sec_Hair3_07"
                                BoneRestPose.translation(0.000000014901161193847656, 0.07882347702980042, -0.000000027939677238464355)
                                Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                    name = "viz:J_Sec_Hair3_07"
                                    SignalRouteUpward.new("update_transform", "transform") {
                                        name = "route_upward:viz:J_Sec_Hair3_07"
                                    }
                                    overlay {
                                        name = "viz_overlay:J_Sec_Hair3_07"
                                        Renderable.cube() {
                                            name = "viz_box:J_Sec_Hair3_07"
                                            Raycastable.enabled()
                                            Color.rgba(1.0, 1.0, 1.0, 1.0)
                                            Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Transform.position(-0.007058793678879738, 0.11198315769433975, 0.11563409864902496).rotation_quat(-0.9981877207756042, 0.003983431961387396, 0.003983012866228819, 0.05991331860423088).scale(0.9999998211860657, 0.9999999403953552, 0.999989926815033) {
                        name = "J_Sec_Hair1_08"
                        BoneRestPose.translation(-0.007058793678879738, 0.11198315769433975, 0.11563409864902496)
                        Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                            name = "viz:J_Sec_Hair1_08"
                            SignalRouteUpward.new("update_transform", "transform") {
                                name = "route_upward:viz:J_Sec_Hair1_08"
                            }
                            overlay {
                                name = "viz_overlay:J_Sec_Hair1_08"
                                Renderable.cube() {
                                    name = "viz_box:J_Sec_Hair1_08"
                                    Raycastable.enabled()
                                    Color.rgba(1.0, 1.0, 1.0, 1.0)
                                    Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                }
                            }
                        }
                        Transform.position(0.000000002153683453798294, 0.034030649811029434, -0.00000001189255272038281).rotation_quat(0.08789399266242981, 0.11670789122581482, -0.13235904276371002, 0.9803749322891235).scale(1.0000001192092896, 1.0, 1.0) {
                            name = "J_Sec_Hair2_08"
                            BoneRestPose.translation(0.000000002153683453798294, 0.034030649811029434, -0.00000001189255272038281)
                            Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                name = "viz:J_Sec_Hair2_08"
                                SignalRouteUpward.new("update_transform", "transform") {
                                    name = "route_upward:viz:J_Sec_Hair2_08"
                                }
                                overlay {
                                    name = "viz_overlay:J_Sec_Hair2_08"
                                    Renderable.cube() {
                                        name = "viz_box:J_Sec_Hair2_08"
                                        Raycastable.enabled()
                                        Color.rgba(1.0, 1.0, 1.0, 1.0)
                                        Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                    }
                                }
                            }
                            Transform.position(0.00000003282912075519562, 0.03354514762759209, 0.00000008899951353669167).rotation_quat(0.00000006798654794692993, -0.000001173466444015503, -0.00000001862645149230957, 1.0).scale(1.0, 1.0, 1.0) {
                                name = "J_Sec_Hair3_08"
                                BoneRestPose.translation(0.00000003282912075519562, 0.03354514762759209, 0.00000008899951353669167)
                                Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                    name = "viz:J_Sec_Hair3_08"
                                    SignalRouteUpward.new("update_transform", "transform") {
                                        name = "route_upward:viz:J_Sec_Hair3_08"
                                    }
                                    overlay {
                                        name = "viz_overlay:J_Sec_Hair3_08"
                                        Renderable.cube() {
                                            name = "viz_box:J_Sec_Hair3_08"
                                            Raycastable.enabled()
                                            Color.rgba(1.0, 1.0, 1.0, 1.0)
                                            Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                        }
                                    }
                                }
                            }
                            overlay {
                                Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.02500000037252903, 0.02500000037252903, 0.02500000037252903) {
                                    Renderable.cube() {
                                        Color.rgba(1.0, 0.4000000059604645, 0.4000000059604645, 1.0)
                                        Emissive.on()
                                        Raycastable.enabled()
                                        Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                    }
                                }
                            }
                        }
                        overlay {
                            Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.02500000037252903, 0.02500000037252903, 0.02500000037252903) {
                                Renderable.cube() {
                                    Color.rgba(1.0, 0.0, 0.0, 1.0)
                                    Emissive.on()
                                    Raycastable.enabled()
                                    Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                }
                            }
                        }
                    }
                    Transform.position(-0.08358767628669739, 0.08850668370723724, 0.07411164045333862).rotation_quat(0.9925668239593506, 0.08458180725574493, 0.08458101749420166, 0.022430405020713806).scale(1.000003695487976, 1.000000238418579, 1.0000004768371582) {
                        name = "J_Sec_Hair1_09"
                        BoneRestPose.translation(-0.08358767628669739, 0.08850668370723724, 0.07411164045333862)
                        Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                            name = "viz:J_Sec_Hair1_09"
                            SignalRouteUpward.new("update_transform", "transform") {
                                name = "route_upward:viz:J_Sec_Hair1_09"
                            }
                            overlay {
                                name = "viz_overlay:J_Sec_Hair1_09"
                                Renderable.cube() {
                                    name = "viz_box:J_Sec_Hair1_09"
                                    Raycastable.enabled()
                                    Color.rgba(1.0, 1.0, 1.0, 1.0)
                                    Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                }
                            }
                        }
                        Transform.position(0.00000001862645149230957, 0.0464143231511116, -0.000000003725290298461914).rotation_quat(0.016597265377640724, 0.4017496407032013, -0.3812257647514343, 0.8324594497680664).scale(1.0, 0.9999998211860657, 0.9999998211860657) {
                            name = "J_Sec_Hair2_09"
                            BoneRestPose.translation(0.00000001862645149230957, 0.0464143231511116, -0.000000003725290298461914)
                            Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                name = "viz:J_Sec_Hair2_09"
                                SignalRouteUpward.new("update_transform", "transform") {
                                    name = "route_upward:viz:J_Sec_Hair2_09"
                                }
                                overlay {
                                    name = "viz_overlay:J_Sec_Hair2_09"
                                    Renderable.cube() {
                                        name = "viz_box:J_Sec_Hair2_09"
                                        Raycastable.enabled()
                                        Color.rgba(1.0, 1.0, 1.0, 1.0)
                                        Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                    }
                                }
                            }
                            Transform.position(-0.00000006938353180885315, 0.04461003839969635, -0.000000011175870895385742).rotation_quat(0.00000005587935802964239, 0.0000003818422840140556, 0.00000009872020712009544, 1.0).scale(0.9999999403953552, 0.9999998807907104, 1.0) {
                                name = "J_Sec_Hair3_09"
                                BoneRestPose.translation(-0.00000006938353180885315, 0.04461003839969635, -0.000000011175870895385742)
                                Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                    name = "viz:J_Sec_Hair3_09"
                                    SignalRouteUpward.new("update_transform", "transform") {
                                        name = "route_upward:viz:J_Sec_Hair3_09"
                                    }
                                    overlay {
                                        name = "viz_overlay:J_Sec_Hair3_09"
                                        Renderable.cube() {
                                            name = "viz_box:J_Sec_Hair3_09"
                                            Raycastable.enabled()
                                            Color.rgba(1.0, 1.0, 1.0, 1.0)
                                            Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Transform.position(0.08275497704744339, 0.08366523683071136, 0.07357272505760193).rotation_quat(0.9903537631034851, -0.09597223252058029, -0.0959738940000534, 0.027887973934412003).scale(0.9999938011169434, 0.9999995231628418, 0.9999990463256836) {
                        name = "J_Sec_Hair1_10"
                        BoneRestPose.translation(0.08275497704744339, 0.08366523683071136, 0.07357272505760193)
                        Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                            name = "viz:J_Sec_Hair1_10"
                            SignalRouteUpward.new("update_transform", "transform") {
                                name = "route_upward:viz:J_Sec_Hair1_10"
                            }
                            overlay {
                                name = "viz_overlay:J_Sec_Hair1_10"
                                Renderable.cube() {
                                    name = "viz_box:J_Sec_Hair1_10"
                                    Raycastable.enabled()
                                    Color.rgba(1.0, 1.0, 1.0, 1.0)
                                    Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                }
                            }
                        }
                        Transform.position(0.00000003073364496231079, 0.043959975242614746, 0.0000000027939677238464355).rotation_quat(0.02782750129699707, -0.4103987514972687, 0.3826744258403778, 0.8272597789764404).scale(0.9999998807907104, 0.9999998807907104, 0.9999999403953552) {
                            name = "J_Sec_Hair2_10"
                            BoneRestPose.translation(0.00000003073364496231079, 0.043959975242614746, 0.0000000027939677238464355)
                            Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                name = "viz:J_Sec_Hair2_10"
                                SignalRouteUpward.new("update_transform", "transform") {
                                    name = "route_upward:viz:J_Sec_Hair2_10"
                                }
                                overlay {
                                    name = "viz_overlay:J_Sec_Hair2_10"
                                    Renderable.cube() {
                                        name = "viz_box:J_Sec_Hair2_10"
                                        Raycastable.enabled()
                                        Color.rgba(1.0, 1.0, 1.0, 1.0)
                                        Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                    }
                                }
                            }
                            Transform.position(-0.000000005122274160385132, 0.04236309975385666, 0.0000000130385160446167).rotation_quat(-0.00000007450581307466564, 0.0000006705522537231445, 0.00000012479723920932884, 1.0).scale(0.9999999403953552, 0.9999998807907104, 1.0) {
                                name = "J_Sec_Hair3_10"
                                BoneRestPose.translation(-0.000000005122274160385132, 0.04236309975385666, 0.0000000130385160446167)
                                Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                    name = "viz:J_Sec_Hair3_10"
                                    SignalRouteUpward.new("update_transform", "transform") {
                                        name = "route_upward:viz:J_Sec_Hair3_10"
                                    }
                                    overlay {
                                        name = "viz_overlay:J_Sec_Hair3_10"
                                        Renderable.cube() {
                                            name = "viz_box:J_Sec_Hair3_10"
                                            Raycastable.enabled()
                                            Color.rgba(1.0, 1.0, 1.0, 1.0)
                                            Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Transform.position(0.08867958188056946, 0.045503728091716766, 0.04951021447777748).rotation_quat(-0.9979470372200012, 0.04472944885492325, 0.04473059996962547, 0.010007224045693874).scale(1.0000169277191162, 0.9999998211860657, 1.0000004768371582) {
                        name = "J_Sec_Hair1_11"
                        BoneRestPose.translation(0.08867958188056946, 0.045503728091716766, 0.04951021447777748)
                        Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                            name = "viz:J_Sec_Hair1_11"
                            SignalRouteUpward.new("update_transform", "transform") {
                                name = "route_upward:viz:J_Sec_Hair1_11"
                            }
                            overlay {
                                name = "viz_overlay:J_Sec_Hair1_11"
                                Renderable.cube() {
                                    name = "viz_box:J_Sec_Hair1_11"
                                    Raycastable.enabled()
                                    Color.rgba(1.0, 1.0, 1.0, 1.0)
                                    Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                }
                            }
                        }
                        Transform.position(-0.000000023283064365386963, 0.04866280034184456, 0.0000000004656612873077393).rotation_quat(-0.06175978109240532, -0.12815970182418823, 0.13634620606899261, 0.9803930521011353).scale(1.0, 1.0, 1.0) {
                            name = "J_Sec_Hair2_11"
                            BoneRestPose.translation(-0.000000023283064365386963, 0.04866280034184456, 0.0000000004656612873077393)
                            Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                name = "viz:J_Sec_Hair2_11"
                                SignalRouteUpward.new("update_transform", "transform") {
                                    name = "route_upward:viz:J_Sec_Hair2_11"
                                }
                                overlay {
                                    name = "viz_overlay:J_Sec_Hair2_11"
                                    Renderable.cube() {
                                        name = "viz_box:J_Sec_Hair2_11"
                                        Raycastable.enabled()
                                        Color.rgba(1.0, 1.0, 1.0, 1.0)
                                        Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                    }
                                }
                            }
                            Transform.position(0.00000001955777406692505, 0.047128669917583466, -0.000000003725290298461914).rotation_quat(-0.00000012852251529693604, 0.0000020917505025863647, 0.0000000996515154838562, 1.0).scale(1.0, 1.0, 1.0000001192092896) {
                                name = "J_Sec_Hair3_11"
                                BoneRestPose.translation(0.00000001955777406692505, 0.047128669917583466, -0.000000003725290298461914)
                                Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                    name = "viz:J_Sec_Hair3_11"
                                    SignalRouteUpward.new("update_transform", "transform") {
                                        name = "route_upward:viz:J_Sec_Hair3_11"
                                    }
                                    overlay {
                                        name = "viz_overlay:J_Sec_Hair3_11"
                                        Renderable.cube() {
                                            name = "viz_box:J_Sec_Hair3_11"
                                            Raycastable.enabled()
                                            Color.rgba(1.0, 1.0, 1.0, 1.0)
                                            Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Transform.position(-0.08904066681861877, 0.04880201071500778, 0.04947853833436966).rotation_quat(-0.9978671073913574, -0.04568617418408394, -0.04568726569414139, 0.00931266974657774).scale(1.0000183582305908, 0.9999998211860657, 1.0000003576278687) {
                        name = "J_Sec_Hair1_12"
                        BoneRestPose.translation(-0.08904066681861877, 0.04880201071500778, 0.04947853833436966)
                        Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                            name = "viz:J_Sec_Hair1_12"
                            SignalRouteUpward.new("update_transform", "transform") {
                                name = "route_upward:viz:J_Sec_Hair1_12"
                            }
                            overlay {
                                name = "viz_overlay:J_Sec_Hair1_12"
                                Renderable.cube() {
                                    name = "viz_box:J_Sec_Hair1_12"
                                    Raycastable.enabled()
                                    Color.rgba(1.0, 1.0, 1.0, 1.0)
                                    Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                }
                            }
                        }
                        Transform.position(-0.00000001979060471057892, 0.05032097175717354, 0.000000001862645149230957).rotation_quat(-0.05983920395374298, 0.12236123532056808, -0.13019536435604095, 0.9820876717567444).scale(1.0000001192092896, 1.0000001192092896, 1.0000001192092896) {
                            name = "J_Sec_Hair2_12"
                            BoneRestPose.translation(-0.00000001979060471057892, 0.05032097175717354, 0.000000001862645149230957)
                            Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                name = "viz:J_Sec_Hair2_12"
                                SignalRouteUpward.new("update_transform", "transform") {
                                    name = "route_upward:viz:J_Sec_Hair2_12"
                                }
                                overlay {
                                    name = "viz_overlay:J_Sec_Hair2_12"
                                    Renderable.cube() {
                                        name = "viz_box:J_Sec_Hair2_12"
                                        Raycastable.enabled()
                                        Color.rgba(1.0, 1.0, 1.0, 1.0)
                                        Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                    }
                                }
                            }
                            Transform.position(0.000000017695128917694092, 0.0487089641392231, -0.0000000130385160446167).rotation_quat(-0.000000013969838619232178, -0.0000019995495676994324, -0.00000008381903171539307, 1.0).scale(1.0, 1.0, 1.0) {
                                name = "J_Sec_Hair3_12"
                                BoneRestPose.translation(0.000000017695128917694092, 0.0487089641392231, -0.0000000130385160446167)
                                Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                    name = "viz:J_Sec_Hair3_12"
                                    SignalRouteUpward.new("update_transform", "transform") {
                                        name = "route_upward:viz:J_Sec_Hair3_12"
                                    }
                                    overlay {
                                        name = "viz_overlay:J_Sec_Hair3_12"
                                        Renderable.cube() {
                                            name = "viz_box:J_Sec_Hair3_12"
                                            Raycastable.enabled()
                                            Color.rgba(1.0, 1.0, 1.0, 1.0)
                                            Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Transform.position(-0.05130218714475632, 0.18171869218349457, 0.09606000781059265).rotation_quat(0.9786875247955322, -0.11947628110647202, -0.11947618424892426, 0.11671140789985657).scale(1.000000238418579, 1.0, 1.0000003576278687) {
                        name = "J_Sec_Hair1_13"
                        BoneRestPose.translation(-0.05130218714475632, 0.18171869218349457, 0.09606000781059265)
                        Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                            name = "viz:J_Sec_Hair1_13"
                            SignalRouteUpward.new("update_transform", "transform") {
                                name = "route_upward:viz:J_Sec_Hair1_13"
                            }
                            overlay {
                                name = "viz_overlay:J_Sec_Hair1_13"
                                Renderable.cube() {
                                    name = "viz_box:J_Sec_Hair1_13"
                                    Raycastable.enabled()
                                    Color.rgba(1.0, 1.0, 1.0, 1.0)
                                    Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                }
                            }
                        }
                        Transform.position(-0.00000006146728992462158, 0.09258615970611572, 0.000000033527612686157227).rotation_quat(0.12215698510408401, 0.11500194668769836, -0.11714327335357666, 0.9788410067558289).scale(0.9999998211860657, 1.0, 0.9999998807907104) {
                            name = "J_Sec_Hair2_13"
                            BoneRestPose.translation(-0.00000006146728992462158, 0.09258615970611572, 0.000000033527612686157227)
                            Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                name = "viz:J_Sec_Hair2_13"
                                SignalRouteUpward.new("update_transform", "transform") {
                                    name = "route_upward:viz:J_Sec_Hair2_13"
                                }
                                overlay {
                                    name = "viz_overlay:J_Sec_Hair2_13"
                                    Renderable.cube() {
                                        name = "viz_box:J_Sec_Hair2_13"
                                        Raycastable.enabled()
                                        Color.rgba(1.0, 1.0, 1.0, 1.0)
                                        Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                    }
                                }
                            }
                            Transform.position(-0.000000005587935447692871, 0.09302911162376404, 0.0000000010477378964424133).rotation_quat(0.22164660692214966, 0.2266806811094284, -0.23076748847961426, 0.9199103713035583).scale(1.0000001192092896, 0.9999999403953552, 1.0) {
                                name = "J_Sec_Hair3_13"
                                BoneRestPose.translation(-0.000000005587935447692871, 0.09302911162376404, 0.0000000010477378964424133)
                                Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                    name = "viz:J_Sec_Hair3_13"
                                    SignalRouteUpward.new("update_transform", "transform") {
                                        name = "route_upward:viz:J_Sec_Hair3_13"
                                    }
                                    overlay {
                                        name = "viz_overlay:J_Sec_Hair3_13"
                                        Renderable.cube() {
                                            name = "viz_box:J_Sec_Hair3_13"
                                            Raycastable.enabled()
                                            Color.rgba(1.0, 1.0, 1.0, 1.0)
                                            Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                        }
                                    }
                                }
                                Transform.position(-0.000000005587935447692871, 0.08398182690143585, -0.00000003632158041000366).rotation_quat(0.00000005587935447692871, -0.00000001862645149230957, -0.00000011175871605928478, 1.0).scale(0.9999999403953552, 1.0, 1.0) {
                                    name = "J_Sec_Hair4_13"
                                    BoneRestPose.translation(-0.000000005587935447692871, 0.08398182690143585, -0.00000003632158041000366)
                                    Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                        name = "viz:J_Sec_Hair4_13"
                                        SignalRouteUpward.new("update_transform", "transform") {
                                            name = "route_upward:viz:J_Sec_Hair4_13"
                                        }
                                        overlay {
                                            name = "viz_overlay:J_Sec_Hair4_13"
                                            Renderable.cube() {
                                                name = "viz_box:J_Sec_Hair4_13"
                                                Raycastable.enabled()
                                                Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Transform.position(0.05293871462345123, 0.17373059689998627, 0.10183283686637878).rotation_quat(0.9844417572021484, 0.11162451654672623, 0.11162564158439636, 0.07716255635023117).scale(0.999997615814209, 0.9999996423721313, 0.999998152256012) {
                        name = "J_Sec_Hair1_14"
                        BoneRestPose.translation(0.05293871462345123, 0.17373059689998627, 0.10183283686637878)
                        Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                            name = "viz:J_Sec_Hair1_14"
                            SignalRouteUpward.new("update_transform", "transform") {
                                name = "route_upward:viz:J_Sec_Hair1_14"
                            }
                            overlay {
                                name = "viz_overlay:J_Sec_Hair1_14"
                                Renderable.cube() {
                                    name = "viz_box:J_Sec_Hair1_14"
                                    Raycastable.enabled()
                                    Color.rgba(1.0, 1.0, 1.0, 1.0)
                                    Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                }
                            }
                        }
                        Transform.position(-0.0000000130385160446167, 0.10978473722934723, 0.000000011175870895385742).rotation_quat(0.22548213601112366, -0.20231997966766357, 0.22032663226127625, 0.9271896481513977).scale(1.0, 0.9999998807907104, 0.9999999403953552) {
                            name = "J_Sec_Hair2_14"
                            BoneRestPose.translation(-0.0000000130385160446167, 0.10978473722934723, 0.000000011175870895385742)
                            Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                name = "viz:J_Sec_Hair2_14"
                                SignalRouteUpward.new("update_transform", "transform") {
                                    name = "route_upward:viz:J_Sec_Hair2_14"
                                }
                                overlay {
                                    name = "viz_overlay:J_Sec_Hair2_14"
                                    Renderable.cube() {
                                        name = "viz_box:J_Sec_Hair2_14"
                                        Raycastable.enabled()
                                        Color.rgba(1.0, 1.0, 1.0, 1.0)
                                        Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                    }
                                }
                            }
                            Transform.position(-0.00000003166496753692627, 0.10610432922840118, 0.000000007450580596923828).rotation_quat(0.00000006612390279769897, -0.0000001210719347000122, -0.000000029802322387695313, 1.0).scale(1.0, 1.0, 1.0) {
                                name = "J_Sec_Hair3_14"
                                BoneRestPose.translation(-0.00000003166496753692627, 0.10610432922840118, 0.000000007450580596923828)
                                Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                    name = "viz:J_Sec_Hair3_14"
                                    SignalRouteUpward.new("update_transform", "transform") {
                                        name = "route_upward:viz:J_Sec_Hair3_14"
                                    }
                                    overlay {
                                        name = "viz_overlay:J_Sec_Hair3_14"
                                        Renderable.cube() {
                                            name = "viz_box:J_Sec_Hair3_14"
                                            Raycastable.enabled()
                                            Color.rgba(1.0, 1.0, 1.0, 1.0)
                                            Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Transform.position(-0.00000000000000004964834320366573, 0.07545603066682816, -0.0000000004672155995422145).rotation_quat(0.04433169960975647, -0.010782935656607151, -0.2066466510295868, 0.9773513078689575).scale(1.0, 0.9999998807907104, 1.0) {
                        name = "J_Bip_C_Head.001"
                        BoneRestPose.translation(-0.00000000000000004964834320366573, 0.07545603066682816, -0.0000000004672155995422145)
                        Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                            name = "viz:J_Bip_C_Head.001"
                            SignalRouteUpward.new("update_transform", "transform") {
                                name = "route_upward:viz:J_Bip_C_Head.001"
                            }
                            overlay {
                                name = "viz_overlay:J_Bip_C_Head.001"
                                Renderable.cube() {
                                    name = "viz_box:J_Bip_C_Head.001"
                                    Raycastable.enabled()
                                    Color.rgba(1.0, 1.0, 1.0, 1.0)
                                    Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                }
                            }
                        }
                        Transform.position(0.00000005313086148817092, 0.15969547629356384, -0.000000010011717677116394).rotation_quat(0.044633638113737106, 0.0004974596085958183, -0.6732215881347656, 0.7380923628807068).scale(1.0, 1.0, 1.0) {
                            name = "J_Bip_C_Head.006"
                            BoneRestPose.translation(0.00000005313086148817092, 0.15969547629356384, -0.000000010011717677116394)
                            Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                name = "viz:J_Bip_C_Head.006"
                                SignalRouteUpward.new("update_transform", "transform") {
                                    name = "route_upward:viz:J_Bip_C_Head.006"
                                }
                                overlay {
                                    name = "viz_overlay:J_Bip_C_Head.006"
                                    Renderable.cube() {
                                        name = "viz_box:J_Bip_C_Head.006"
                                        Raycastable.enabled()
                                        Color.rgba(1.0, 1.0, 1.0, 1.0)
                                        Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                    }
                                }
                            }
                            Transform.position(-0.00000005192123353481293, 0.07980231195688248, -0.00000001955777406692505).rotation_quat(0.07593245059251785, -0.00000012605215715666418, -0.5336707234382629, 0.8422765731811523).scale(0.9999998807907104, 0.9999999403953552, 1.0) {
                                name = "J_Bip_C_Head.007"
                                BoneRestPose.translation(-0.00000005192123353481293, 0.07980231195688248, -0.00000001955777406692505)
                                Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                    name = "viz:J_Bip_C_Head.007"
                                    SignalRouteUpward.new("update_transform", "transform") {
                                        name = "route_upward:viz:J_Bip_C_Head.007"
                                    }
                                    overlay {
                                        name = "viz_overlay:J_Bip_C_Head.007"
                                        Renderable.cube() {
                                            name = "viz_box:J_Bip_C_Head.007"
                                            Raycastable.enabled()
                                            Color.rgba(1.0, 1.0, 1.0, 1.0)
                                            Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                        }
                                    }
                                }
                                Transform.position(0.000000010652001947164536, 0.06436300277709961, 0.000000002444721758365631).rotation_quat(-0.005851480178534985, -0.00000007644982247256848, -0.10582751035690308, 0.9943673014640808).scale(0.9999999403953552, 1.0000001192092896, 1.0) {
                                    name = "J_Bip_C_Head.008"
                                    BoneRestPose.translation(0.000000010652001947164536, 0.06436300277709961, 0.000000002444721758365631)
                                    Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                        name = "viz:J_Bip_C_Head.008"
                                        SignalRouteUpward.new("update_transform", "transform") {
                                            name = "route_upward:viz:J_Bip_C_Head.008"
                                        }
                                        overlay {
                                            name = "viz_overlay:J_Bip_C_Head.008"
                                            Renderable.cube() {
                                                name = "viz_box:J_Bip_C_Head.008"
                                                Raycastable.enabled()
                                                Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Transform.position(-0.00000000000000004964834320366573, 0.07545603066682816, -0.0000000004672155995422145).rotation_quat(0.06940913945436478, 0.018047302961349487, 0.22672538459300995, 0.9713147282600403).scale(1.0, 1.0, 0.9999999403953552) {
                        name = "J_Bip_C_Head.002"
                        BoneRestPose.translation(-0.00000000000000004964834320366573, 0.07545603066682816, -0.0000000004672155995422145)
                        Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                            name = "viz:J_Bip_C_Head.002"
                            SignalRouteUpward.new("update_transform", "transform") {
                                name = "route_upward:viz:J_Bip_C_Head.002"
                            }
                            overlay {
                                name = "viz_overlay:J_Bip_C_Head.002"
                                Renderable.cube() {
                                    name = "viz_box:J_Bip_C_Head.002"
                                    Raycastable.enabled()
                                    Color.rgba(1.0, 1.0, 1.0, 1.0)
                                    Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                }
                            }
                        }
                        Transform.position(0.00000007440621629939415, 0.1585032343864441, 0.000000013853423297405243).rotation_quat(0.11151903867721558, -0.020000005140900612, 0.6386624574661255, 0.7611004114151001).scale(0.9999999403953552, 1.0, 1.0) {
                            name = "J_Bip_C_Head.003"
                            BoneRestPose.translation(0.00000007440621629939415, 0.1585032343864441, 0.000000013853423297405243)
                            Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                name = "viz:J_Bip_C_Head.003"
                                SignalRouteUpward.new("update_transform", "transform") {
                                    name = "route_upward:viz:J_Bip_C_Head.003"
                                }
                                overlay {
                                    name = "viz_overlay:J_Bip_C_Head.003"
                                    Renderable.cube() {
                                        name = "viz_box:J_Bip_C_Head.003"
                                        Raycastable.enabled()
                                        Color.rgba(1.0, 1.0, 1.0, 1.0)
                                        Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                    }
                                }
                            }
                            Transform.position(0.0000001962762326002121, 0.06420013308525085, -0.00000000651925802230835).rotation_quat(0.17900004982948303, 0.02834058366715908, 0.36833998560905457, 0.9118560552597046).scale(1.0, 1.0, 1.0) {
                                name = "J_Bip_C_Head.004"
                                BoneRestPose.translation(0.0000001962762326002121, 0.06420013308525085, -0.00000000651925802230835)
                                Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                    name = "viz:J_Bip_C_Head.004"
                                    SignalRouteUpward.new("update_transform", "transform") {
                                        name = "route_upward:viz:J_Bip_C_Head.004"
                                    }
                                    overlay {
                                        name = "viz_overlay:J_Bip_C_Head.004"
                                        Renderable.cube() {
                                            name = "viz_box:J_Bip_C_Head.004"
                                            Raycastable.enabled()
                                            Color.rgba(1.0, 1.0, 1.0, 1.0)
                                            Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                        }
                                    }
                                }
                                Transform.position(-0.00000003632158041000366, 0.041886430233716965, 0.000000010710209608078003).rotation_quat(0.0630379319190979, 0.0048378813080489635, 0.17921769618988037, 0.9817758798599243).scale(0.9999999403953552, 1.0, 1.0) {
                                    name = "J_Bip_C_Head.005"
                                    BoneRestPose.translation(-0.00000003632158041000366, 0.041886430233716965, 0.000000010710209608078003)
                                    Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                                        name = "viz:J_Bip_C_Head.005"
                                        SignalRouteUpward.new("update_transform", "transform") {
                                            name = "route_upward:viz:J_Bip_C_Head.005"
                                        }
                                        overlay {
                                            name = "viz_overlay:J_Bip_C_Head.005"
                                            Renderable.cube() {
                                                name = "viz_box:J_Bip_C_Head.005"
                                                Raycastable.enabled()
                                                Color.rgba(1.0, 1.0, 1.0, 1.0)
                                                Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Transform.position(-0.00000000000000004474080305211928, 0.10378933697938919, -0.014945661649107933).rotation_quat(0.00000009735359185469861, 0.00000000000000033306674856886935, -0.0000000000000011102232363833934, 1.0).scale(1.0, 1.000000238418579, 1.0) {
                        name = "J_Bip_C_Head_collider_0.001"
                        BoneRestPose.translation(-0.00000000000000004474080305211928, 0.10378933697938919, -0.014945661649107933)
                        Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.029999999329447746, 0.029999999329447746, 0.029999999329447746) {
                            name = "viz:J_Bip_C_Head_collider_0.001"
                            SignalRouteUpward.new("update_transform", "transform") {
                                name = "route_upward:viz:J_Bip_C_Head_collider_0.001"
                            }
                            overlay {
                                name = "viz_overlay:J_Bip_C_Head_collider_0.001"
                                Renderable.cube() {
                                    name = "viz_box:J_Bip_C_Head_collider_0.001"
                                    Raycastable.enabled()
                                    Color.rgba(1.0, 1.0, 1.0, 1.0)
                                    Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                                }
                            }
                        }
                    }
                    overlay {
                        Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.02500000037252903, 0.02500000037252903, 0.02500000037252903) {
                            Renderable.cube() {
                                Color.rgba(0.8500000238418579, 0.20000000298023224, 0.8500000238418579, 0.8999999761581421)
                                Emissive.on()
                                Raycastable.enabled()
                                Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                            }
                        }
                    }
                    Transform.position(0.0, 0.07999999821186066, 0.11999999731779099).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                        CameraXR.on() {
                            Pointer {
                                Raycast.event_driven().max_distance(200.0)
                            }
                        }
                    }
                }
            }
        }
    }
}

InputXR.on() {
    Transform.position(-0.9250506162643433, 1.6984316110610962, -0.17940136790275574).rotation_quat(0.4078806936740875, -0.18548893928527832, -0.13585136830806732, 0.8836128115653992).scale(1.0, 1.0, 1.0) {
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
    Transform.position(3.0, 1.2000000476837158, 3.5).rotation_quat(-0.11753141134977341, 0.013396346010267735, 0.001585626625455916, 0.9929775595664978).scale(1.0, 1.0, 1.0) {
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
    name = "editor_runtime_ui_root"
    Transform.position(-0.699999988079071, 1.600000023841858, -1.2000000476837158).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
        name = "editor_panel_layout_mount"
        overlay {
            LayoutRoot.width(53.5).height(60.5).unit_scale(0.07999999821186066) {
                name = "editor_panel_layout_root"
                Transform.position(0.0, -0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                    name = "editor_world_panel_shell"
                    Style {}
                    Transform.position(0.03999999910593033, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                        name = "world_panel_root"
                        Style {}
                        Transform.position(0.035999998450279236, -0.019999999552965164, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                            name = "save_status_wrap"
                            Style {}
                            Transform.position(0.0, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                                name = "panel_status_root"
                                Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                                    Text {
                                        "idle"
                                        name = "panel_status_value"
                                        Transform.position(0.03999999910593033, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                            Renderable.square() {
                                                UV.uv(0.5625, 0.4375).uv(0.625, 0.4375).uv(0.625, 0.375).uv(0.5625, 0.375)
                                                Texture.from_dds("assets/textures/font_system.dds")
                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                            }
                                        }
                                        Transform.position(0.11999999731779099, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                            Renderable.square() {
                                                UV.uv(0.25, 0.4375).uv(0.3125, 0.4375).uv(0.3125, 0.375).uv(0.25, 0.375)
                                                Texture.from_dds("assets/textures/font_system.dds")
                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                            }
                                        }
                                        Transform.position(0.19999998807907104, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                            Renderable.square() {
                                                UV.uv(0.75, 0.4375).uv(0.8125, 0.4375).uv(0.8125, 0.375).uv(0.75, 0.375)
                                                Texture.from_dds("assets/textures/font_system.dds")
                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                            }
                                        }
                                        Transform.position(0.2800000011920929, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                            Renderable.square() {
                                                UV.uv(0.3125, 0.4375).uv(0.375, 0.4375).uv(0.375, 0.375).uv(0.3125, 0.375)
                                                Texture.from_dds("assets/textures/font_system.dds")
                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                            }
                                        }
                                    }
                                }
                            }
                            Color.rgba(0.9200000166893005, 1.0, 0.9200000166893005, 1.0) {
                                name = "__text_color"
                            }
                            Transform.position(1.1039999723434448, -0.07999999821186066, -0.02500000037252903).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(2.2799997329711914, 0.19999998807907104, 1.0) {
                                name = "__bg"
                                Color.rgba(0.07999999821186066, 0.23999999463558197, 0.10999999940395355, 0.9200000166893005) {
                                    Renderable.square() {
                                        Opacity.opacity(0.9200000166893005)
                                        Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                    }
                                }
                            }
                        }
                        Transform.position(0.0, -0.19999998807907104, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                            name = "title_bar"
                            Style {}
                            Transform.position(0.019999999552965164, -0.019999999552965164, 0.05000000074505806).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                                name = "title_label_wrap"
                                Style {}
                                Transform.position(0.0, -0.059999994933605194, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                                    Text {
                                        "World"
                                        name = "title_label"
                                        Transform.position(0.03999999910593033, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                            Renderable.square() {
                                                UV.uv(0.4375, 0.375).uv(0.5, 0.375).uv(0.5, 0.3125).uv(0.4375, 0.3125)
                                                Texture.from_dds("assets/textures/font_system.dds")
                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                            }
                                        }
                                        Transform.position(0.11999999731779099, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                            Renderable.square() {
                                                UV.uv(0.9375, 0.4375).uv(1.0, 0.4375).uv(1.0, 0.375).uv(0.9375, 0.375)
                                                Texture.from_dds("assets/textures/font_system.dds")
                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                            }
                                        }
                                        Transform.position(0.19999998807907104, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                            Renderable.square() {
                                                UV.uv(0.125, 0.5).uv(0.1875, 0.5).uv(0.1875, 0.4375).uv(0.125, 0.4375)
                                                Texture.from_dds("assets/textures/font_system.dds")
                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                            }
                                        }
                                        Transform.position(0.2800000011920929, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                            Renderable.square() {
                                                UV.uv(0.75, 0.4375).uv(0.8125, 0.4375).uv(0.8125, 0.375).uv(0.75, 0.375)
                                                Texture.from_dds("assets/textures/font_system.dds")
                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                            }
                                        }
                                        Transform.position(0.35999998450279236, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                            Renderable.square() {
                                                UV.uv(0.25, 0.4375).uv(0.3125, 0.4375).uv(0.3125, 0.375).uv(0.25, 0.375)
                                                Texture.from_dds("assets/textures/font_system.dds")
                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                            }
                                        }
                                    }
                                }
                                Color.rgba(0.8999999761581421, 1.0, 0.9200000166893005, 1.0) {
                                    name = "__text_color"
                                }
                            }
                            Transform.position(1.1959999799728394, -0.0, 0.05000000074505806).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                                name = "save_button"
                                Raycastable.enabled()
                                Style {}
                                Transform.position(0.07899999618530273, -0.0560000017285347, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                                    Text {
                                        "Save"
                                        Transform.position(0.03999999910593033, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                            Renderable.square() {
                                                UV.uv(0.1875, 0.375).uv(0.25, 0.375).uv(0.25, 0.3125).uv(0.1875, 0.3125)
                                                Texture.from_dds("assets/textures/font_system.dds")
                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                            }
                                        }
                                        Transform.position(0.11999999731779099, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                            Renderable.square() {
                                                UV.uv(0.0625, 0.4375).uv(0.125, 0.4375).uv(0.125, 0.375).uv(0.0625, 0.375)
                                                Texture.from_dds("assets/textures/font_system.dds")
                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                            }
                                        }
                                        Transform.position(0.19999998807907104, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                            Renderable.square() {
                                                UV.uv(0.375, 0.5).uv(0.4375, 0.5).uv(0.4375, 0.4375).uv(0.375, 0.4375)
                                                Texture.from_dds("assets/textures/font_system.dds")
                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                            }
                                        }
                                        Transform.position(0.2800000011920929, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                            Renderable.square() {
                                                UV.uv(0.3125, 0.4375).uv(0.375, 0.4375).uv(0.375, 0.375).uv(0.3125, 0.375)
                                                Texture.from_dds("assets/textures/font_system.dds")
                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                            }
                                        }
                                    }
                                }
                                Color.rgba(0.75, 1.0, 0.44999998807907104, 1.0) {
                                    name = "__text_color"
                                }
                                Transform.position(0.23899999260902405, -0.09600000083446503, -0.02500000037252903).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.550000011920929, 0.19200000166893005, 1.0) {
                                    name = "__bg"
                                    Color.rgba(0.10000000149011612, 0.550000011920929, 0.18000000715255737, 1.0) {
                                        Renderable.square() {
                                            Raycastable.enabled() {
                                                name = "__bg_raycastable"
                                            }
                                            RaycastableShape.quad_2d() {
                                                name = "__bg_raycastable_shape"
                                            }
                                            Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                        }
                                    }
                                }
                            }
                            Transform.position(1.746000051498413, -0.0, 0.05000000074505806).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                                name = "load_button"
                                Raycastable.enabled()
                                Style {}
                                Transform.position(0.07899999618530273, -0.0560000017285347, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                                    Text {
                                        "Load"
                                        Transform.position(0.03999999910593033, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                            Renderable.square() {
                                                UV.uv(0.75, 0.3125).uv(0.8125, 0.3125).uv(0.8125, 0.25).uv(0.75, 0.25)
                                                Texture.from_dds("assets/textures/font_system.dds")
                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                            }
                                        }
                                        Transform.position(0.11999999731779099, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                            Renderable.square() {
                                                UV.uv(0.9375, 0.4375).uv(1.0, 0.4375).uv(1.0, 0.375).uv(0.9375, 0.375)
                                                Texture.from_dds("assets/textures/font_system.dds")
                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                            }
                                        }
                                        Transform.position(0.19999998807907104, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                            Renderable.square() {
                                                UV.uv(0.0625, 0.4375).uv(0.125, 0.4375).uv(0.125, 0.375).uv(0.0625, 0.375)
                                                Texture.from_dds("assets/textures/font_system.dds")
                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                            }
                                        }
                                        Transform.position(0.2800000011920929, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                            Renderable.square() {
                                                UV.uv(0.25, 0.4375).uv(0.3125, 0.4375).uv(0.3125, 0.375).uv(0.25, 0.375)
                                                Texture.from_dds("assets/textures/font_system.dds")
                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                            }
                                        }
                                    }
                                }
                                Color.rgba(0.75, 1.0, 0.44999998807907104, 1.0) {
                                    name = "__text_color"
                                }
                                Transform.position(0.23899999260902405, -0.09600000083446503, -0.02500000037252903).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.550000011920929, 0.19200000166893005, 1.0) {
                                    name = "__bg"
                                    Color.rgba(0.10000000149011612, 0.550000011920929, 0.18000000715255737, 1.0) {
                                        Renderable.square() {
                                            Raycastable.enabled() {
                                                name = "__bg_raycastable"
                                            }
                                            RaycastableShape.quad_2d() {
                                                name = "__bg_raycastable_shape"
                                            }
                                            Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                        }
                                    }
                                }
                            }
                            Transform.position(1.1399999856948853, -0.11999999731779099, -0.02500000037252903).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(2.2799999713897705, 0.23999999463558197, 1.0) {
                                name = "__bg"
                                Color.rgba(0.18000000715255737, 0.7799999713897705, 0.2199999988079071, 0.949999988079071) {
                                    Renderable.square() {
                                        Opacity.opacity(0.949999988079071)
                                        Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                    }
                                }
                            }
                        }
                        Transform.position(0.0, -0.4399999976158142, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                            name = "content_slot"
                            Style {}
                            Transform.position(1.1399999856948853, -2.1599998474121094, -0.02500000037252903).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(2.2799999713897705, 4.319999694824219, 1.0) {
                                name = "__bg"
                                Color.rgba(0.9599999785423279, 0.9200000166893005, 0.18000000715255737, 0.800000011920929) {
                                    Renderable.square() {
                                        Opacity.opacity(0.800000011920929)
                                        Raycastable.drag_only() {
                                            name = "__scroll_drag_raycastable"
                                        }
                                        RaycastableShape.quad_2d() {
                                            name = "__scroll_drag_shape"
                                        }
                                        Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                    }
                                }
                            }
                            StencilClip {
                                name = "__layout_stencil_clip"
                            }
                            Scrolling.new(54.0, 107.39996337890625) {
                                name = "__scroll"
                                Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                                    name = "__scroll_track"
                                    Transform.position(0.0, -0.0, 0.05000000074505806).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                                        name = "world_panel_content_root"
                                        Style {
                                            name = "world_panel_content_root_style"
                                        }
                                        Transform.position(0.0, -0.0, 0.05000000074505806).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                                            name = "rows_mount"
                                            Style {
                                                name = "rows_mount_style"
                                            }
                                            Transform.position(0.05199999734759331, -0.06400000303983688, 0.05000000074505806).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                                                name = "item_0"
                                                Style {
                                                    name = "item_0_style"
                                                }
                                                Transform.position(0.0, 0.0, 0.019999999552965164).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                                                    name = "item_0_text_root"
                                                    Text {
                                                        "Editor { id=10v1 }"
                                                        name = "item_0_text"
                                                        Color.rgba(0.0, 0.0, 0.0, 1.0) {
                                                            name = "item_0_text_color"
                                                        }
                                                        Transform.position(0.03999999910593033, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.3125, 0.3125).uv(0.375, 0.3125).uv(0.375, 0.25).uv(0.3125, 0.25)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.11999999731779099, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.25, 0.4375).uv(0.3125, 0.4375).uv(0.3125, 0.375).uv(0.25, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.19999998807907104, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.5625, 0.4375).uv(0.625, 0.4375).uv(0.625, 0.375).uv(0.5625, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.2800000011920929, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.25, 0.5).uv(0.3125, 0.5).uv(0.3125, 0.4375).uv(0.25, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.35999998450279236, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.9375, 0.4375).uv(1.0, 0.4375).uv(1.0, 0.375).uv(0.9375, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.4399999976158142, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.125, 0.5).uv(0.1875, 0.5).uv(0.1875, 0.4375).uv(0.125, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.5999999642372131, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.6875, 0.5).uv(0.75, 0.5).uv(0.75, 0.4375).uv(0.6875, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.7599999904632568, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.5625, 0.4375).uv(0.625, 0.4375).uv(0.625, 0.375).uv(0.5625, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.8399999737739563, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.25, 0.4375).uv(0.3125, 0.4375).uv(0.3125, 0.375).uv(0.25, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.9199999570846558, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.8125, 0.25).uv(0.875, 0.25).uv(0.875, 0.1875).uv(0.8125, 0.1875)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.0, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.0625, 0.25).uv(0.125, 0.25).uv(0.125, 0.1875).uv(0.0625, 0.1875)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.0799999237060547, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.0, 0.25).uv(0.0625, 0.25).uv(0.0625, 0.1875).uv(0.0, 0.1875)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.159999966621399, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.375, 0.5).uv(0.4375, 0.5).uv(0.4375, 0.4375).uv(0.375, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.2400000095367432, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.0625, 0.25).uv(0.125, 0.25).uv(0.125, 0.1875).uv(0.0625, 0.1875)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.399999976158142, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.8125, 0.5).uv(0.875, 0.5).uv(0.875, 0.4375).uv(0.8125, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                    }
                                                }
                                                Color.rgba(0.0, 0.0, 0.0, 1.0) {
                                                    name = "__text_color"
                                                }
                                                Transform.position(1.087999939918518, -0.03999999538064003, -0.02500000037252903).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(2.247999906539917, 0.1679999828338623, 1.0) {
                                                    name = "__bg"
                                                    Color.rgba(0.18000000715255737, 0.7799999713897705, 0.2199999988079071, 0.949999988079071) {
                                                        Renderable.square() {
                                                            Opacity.opacity(0.949999988079071)
                                                            Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                        }
                                                    }
                                                }
                                            }
                                            Transform.position(0.05199999734759331, -0.2719999849796295, 0.05000000074505806).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                                                name = "item_1"
                                                Raycastable.click_only() {
                                                    name = "item_1_raycastable"
                                                }
                                                Style {
                                                    name = "item_1_style"
                                                }
                                                Transform.position(0.0, 0.0, 0.019999999552965164).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                                                    name = "item_1_text_root"
                                                    Text {
                                                        "editor_auto_raycastable"
                                                        name = "item_1_text"
                                                        Color.rgba(0.05999999865889549, 0.09000000357627869, 0.07999999821186066, 1.0) {
                                                            name = "item_1_text_color"
                                                        }
                                                        Transform.position(0.03999999910593033, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.3125, 0.4375).uv(0.375, 0.4375).uv(0.375, 0.375).uv(0.3125, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.11999999731779099, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.25, 0.4375).uv(0.3125, 0.4375).uv(0.3125, 0.375).uv(0.25, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.19999998807907104, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.5625, 0.4375).uv(0.625, 0.4375).uv(0.625, 0.375).uv(0.5625, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.2800000011920929, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.25, 0.5).uv(0.3125, 0.5).uv(0.3125, 0.4375).uv(0.25, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.35999998450279236, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.9375, 0.4375).uv(1.0, 0.4375).uv(1.0, 0.375).uv(0.9375, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.4399999976158142, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.125, 0.5).uv(0.1875, 0.5).uv(0.1875, 0.4375).uv(0.125, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.5199999809265137, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.9375, 0.375).uv(1.0, 0.375).uv(1.0, 0.3125).uv(0.9375, 0.3125)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.5999999642372131, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.0625, 0.4375).uv(0.125, 0.4375).uv(0.125, 0.375).uv(0.0625, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.6800000071525574, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.3125, 0.5).uv(0.375, 0.5).uv(0.375, 0.4375).uv(0.3125, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.7599999904632568, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.25, 0.5).uv(0.3125, 0.5).uv(0.3125, 0.4375).uv(0.25, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.8399999737739563, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.9375, 0.4375).uv(1.0, 0.4375).uv(1.0, 0.375).uv(0.9375, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.9199999570846558, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.9375, 0.375).uv(1.0, 0.375).uv(1.0, 0.3125).uv(0.9375, 0.3125)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.0, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.125, 0.5).uv(0.1875, 0.5).uv(0.1875, 0.4375).uv(0.125, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.0799999237060547, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.0625, 0.4375).uv(0.125, 0.4375).uv(0.125, 0.375).uv(0.0625, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.159999966621399, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.5625, 0.5).uv(0.625, 0.5).uv(0.625, 0.4375).uv(0.5625, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.2400000095367432, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.1875, 0.4375).uv(0.25, 0.4375).uv(0.25, 0.375).uv(0.1875, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.3199999332427979, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.0625, 0.4375).uv(0.125, 0.4375).uv(0.125, 0.375).uv(0.0625, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.399999976158142, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.1875, 0.5).uv(0.25, 0.5).uv(0.25, 0.4375).uv(0.1875, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.4800000190734863, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.25, 0.5).uv(0.3125, 0.5).uv(0.3125, 0.4375).uv(0.25, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.559999942779541, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.0625, 0.4375).uv(0.125, 0.4375).uv(0.125, 0.375).uv(0.0625, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.6399999856948853, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.125, 0.4375).uv(0.1875, 0.4375).uv(0.1875, 0.375).uv(0.125, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.71999990940094, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.75, 0.4375).uv(0.8125, 0.4375).uv(0.8125, 0.375).uv(0.75, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.7999999523162842, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.3125, 0.4375).uv(0.375, 0.4375).uv(0.375, 0.375).uv(0.3125, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                    }
                                                }
                                                Color.rgba(0.05999999865889549, 0.09000000357627869, 0.07999999821186066, 1.0) {
                                                    name = "__text_color"
                                                }
                                                Transform.position(1.087999939918518, -0.03999999538064003, -0.02500000037252903).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(2.247999906539917, 0.1679999828338623, 1.0) {
                                                    name = "__bg"
                                                    Color.rgba(0.9200000166893005, 0.9700000286102295, 0.9200000166893005, 1.0) {
                                                        Renderable.square() {
                                                            Raycastable.click_only() {
                                                                name = "__bg_raycastable"
                                                            }
                                                            RaycastableShape.quad_2d() {
                                                                name = "__bg_raycastable_shape"
                                                            }
                                                            Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                        }
                                                    }
                                                }
                                            }
                                            Transform.position(0.05199999734759331, -0.47999998927116394, 0.05000000074505806).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                                                name = "item_2"
                                                Raycastable.click_only() {
                                                    name = "item_2_raycastable"
                                                }
                                                Style {
                                                    name = "item_2_style"
                                                }
                                                Transform.position(0.0, 0.0, 0.019999999552965164).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                                                    name = "item_2_text_root"
                                                    Text {
                                                        "  transform"
                                                        name = "item_2_text"
                                                        Color.rgba(0.05999999865889549, 0.09000000357627869, 0.07999999821186066, 1.0) {
                                                            name = "item_2_text_color"
                                                        }
                                                        Transform.position(0.19999998807907104, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.25, 0.5).uv(0.3125, 0.5).uv(0.3125, 0.4375).uv(0.25, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.2800000011920929, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.125, 0.5).uv(0.1875, 0.5).uv(0.1875, 0.4375).uv(0.125, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.35999998450279236, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.0625, 0.4375).uv(0.125, 0.4375).uv(0.125, 0.375).uv(0.0625, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.4399999976158142, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.875, 0.4375).uv(0.9375, 0.4375).uv(0.9375, 0.375).uv(0.875, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.5199999809265137, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.1875, 0.5).uv(0.25, 0.5).uv(0.25, 0.4375).uv(0.1875, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.5999999642372131, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.375, 0.4375).uv(0.4375, 0.4375).uv(0.4375, 0.375).uv(0.375, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.6800000071525574, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.9375, 0.4375).uv(1.0, 0.4375).uv(1.0, 0.375).uv(0.9375, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.7599999904632568, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.125, 0.5).uv(0.1875, 0.5).uv(0.1875, 0.4375).uv(0.125, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.8399999737739563, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.8125, 0.4375).uv(0.875, 0.4375).uv(0.875, 0.375).uv(0.8125, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                    }
                                                }
                                                Color.rgba(0.05999999865889549, 0.09000000357627869, 0.07999999821186066, 1.0) {
                                                    name = "__text_color"
                                                }
                                                Transform.position(1.087999939918518, -0.03999999538064003, -0.02500000037252903).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(2.247999906539917, 0.1679999828338623, 1.0) {
                                                    name = "__bg"
                                                    Color.rgba(0.9200000166893005, 0.9700000286102295, 0.9200000166893005, 1.0) {
                                                        Renderable.square() {
                                                            Raycastable.click_only() {
                                                                name = "__bg_raycastable"
                                                            }
                                                            RaycastableShape.quad_2d() {
                                                                name = "__bg_raycastable_shape"
                                                            }
                                                            Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                        }
                                                    }
                                                }
                                            }
                                            Transform.position(0.05199999734759331, -0.687999963760376, 0.05000000074505806).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                                                name = "item_3"
                                                Raycastable.click_only() {
                                                    name = "item_3_raycastable"
                                                }
                                                Style {
                                                    name = "item_3_style"
                                                }
                                                Transform.position(0.0, 0.0, 0.019999999552965164).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                                                    name = "item_3_text_root"
                                                    Text {
                                                        "    renderable"
                                                        name = "item_3_text"
                                                        Color.rgba(0.05999999865889549, 0.09000000357627869, 0.07999999821186066, 1.0) {
                                                            name = "item_3_text_color"
                                                        }
                                                        Transform.position(0.35999998450279236, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.125, 0.5).uv(0.1875, 0.5).uv(0.1875, 0.4375).uv(0.125, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.4399999976158142, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.3125, 0.4375).uv(0.375, 0.4375).uv(0.375, 0.375).uv(0.3125, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.5199999809265137, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.875, 0.4375).uv(0.9375, 0.4375).uv(0.9375, 0.375).uv(0.875, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.5999999642372131, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.25, 0.4375).uv(0.3125, 0.4375).uv(0.3125, 0.375).uv(0.25, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.6800000071525574, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.3125, 0.4375).uv(0.375, 0.4375).uv(0.375, 0.375).uv(0.3125, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.7599999904632568, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.125, 0.5).uv(0.1875, 0.5).uv(0.1875, 0.4375).uv(0.125, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.8399999737739563, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.0625, 0.4375).uv(0.125, 0.4375).uv(0.125, 0.375).uv(0.0625, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.9199999570846558, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.125, 0.4375).uv(0.1875, 0.4375).uv(0.1875, 0.375).uv(0.125, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.0, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.75, 0.4375).uv(0.8125, 0.4375).uv(0.8125, 0.375).uv(0.75, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.0799999237060547, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.3125, 0.4375).uv(0.375, 0.4375).uv(0.375, 0.375).uv(0.3125, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                    }
                                                }
                                                Color.rgba(0.05999999865889549, 0.09000000357627869, 0.07999999821186066, 1.0) {
                                                    name = "__text_color"
                                                }
                                                Transform.position(1.087999939918518, -0.03999999538064003, -0.02500000037252903).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(2.247999906539917, 0.1679999828338623, 1.0) {
                                                    name = "__bg"
                                                    Color.rgba(0.9200000166893005, 0.9700000286102295, 0.9200000166893005, 1.0) {
                                                        Renderable.square() {
                                                            Raycastable.click_only() {
                                                                name = "__bg_raycastable"
                                                            }
                                                            RaycastableShape.quad_2d() {
                                                                name = "__bg_raycastable_shape"
                                                            }
                                                            Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                        }
                                                    }
                                                }
                                            }
                                            Transform.position(0.05199999734759331, -0.8959999680519104, 0.05000000074505806).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                                                name = "item_4"
                                                Raycastable.click_only() {
                                                    name = "item_4_raycastable"
                                                }
                                                Style {
                                                    name = "item_4_style"
                                                }
                                                Transform.position(0.0, 0.0, 0.019999999552965164).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                                                    name = "item_4_text_root"
                                                    Text {
                                                        "      color"
                                                        name = "item_4_text"
                                                        Color.rgba(0.05999999865889549, 0.09000000357627869, 0.07999999821186066, 1.0) {
                                                            name = "item_4_text_color"
                                                        }
                                                        Transform.position(0.5199999809265137, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.1875, 0.4375).uv(0.25, 0.4375).uv(0.25, 0.375).uv(0.1875, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.5999999642372131, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.9375, 0.4375).uv(1.0, 0.4375).uv(1.0, 0.375).uv(0.9375, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.6800000071525574, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.75, 0.4375).uv(0.8125, 0.4375).uv(0.8125, 0.375).uv(0.75, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.7599999904632568, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.9375, 0.4375).uv(1.0, 0.4375).uv(1.0, 0.375).uv(0.9375, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.8399999737739563, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.125, 0.5).uv(0.1875, 0.5).uv(0.1875, 0.4375).uv(0.125, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                    }
                                                }
                                                Color.rgba(0.05999999865889549, 0.09000000357627869, 0.07999999821186066, 1.0) {
                                                    name = "__text_color"
                                                }
                                                Transform.position(1.087999939918518, -0.03999999538064003, -0.02500000037252903).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(2.247999906539917, 0.1679999828338623, 1.0) {
                                                    name = "__bg"
                                                    Color.rgba(0.9200000166893005, 0.9700000286102295, 0.9200000166893005, 1.0) {
                                                        Renderable.square() {
                                                            Raycastable.click_only() {
                                                                name = "__bg_raycastable"
                                                            }
                                                            RaycastableShape.quad_2d() {
                                                                name = "__bg_raycastable_shape"
                                                            }
                                                            Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                        }
                                                    }
                                                }
                                            }
                                            Transform.position(0.05199999734759331, -1.1039999723434448, 0.05000000074505806).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                                                name = "item_5"
                                                Raycastable.click_only() {
                                                    name = "item_5_raycastable"
                                                }
                                                Style {
                                                    name = "item_5_style"
                                                }
                                                Transform.position(0.0, 0.0, 0.019999999552965164).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                                                    name = "item_5_text_root"
                                                    Text {
                                                        "editor_auto_raycastable"
                                                        name = "item_5_text"
                                                        Color.rgba(0.05999999865889549, 0.09000000357627869, 0.07999999821186066, 1.0) {
                                                            name = "item_5_text_color"
                                                        }
                                                        Transform.position(0.03999999910593033, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.3125, 0.4375).uv(0.375, 0.4375).uv(0.375, 0.375).uv(0.3125, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.11999999731779099, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.25, 0.4375).uv(0.3125, 0.4375).uv(0.3125, 0.375).uv(0.25, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.19999998807907104, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.5625, 0.4375).uv(0.625, 0.4375).uv(0.625, 0.375).uv(0.5625, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.2800000011920929, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.25, 0.5).uv(0.3125, 0.5).uv(0.3125, 0.4375).uv(0.25, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.35999998450279236, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.9375, 0.4375).uv(1.0, 0.4375).uv(1.0, 0.375).uv(0.9375, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.4399999976158142, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.125, 0.5).uv(0.1875, 0.5).uv(0.1875, 0.4375).uv(0.125, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.5199999809265137, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.9375, 0.375).uv(1.0, 0.375).uv(1.0, 0.3125).uv(0.9375, 0.3125)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.5999999642372131, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.0625, 0.4375).uv(0.125, 0.4375).uv(0.125, 0.375).uv(0.0625, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.6800000071525574, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.3125, 0.5).uv(0.375, 0.5).uv(0.375, 0.4375).uv(0.3125, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.7599999904632568, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.25, 0.5).uv(0.3125, 0.5).uv(0.3125, 0.4375).uv(0.25, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.8399999737739563, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.9375, 0.4375).uv(1.0, 0.4375).uv(1.0, 0.375).uv(0.9375, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.9199999570846558, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.9375, 0.375).uv(1.0, 0.375).uv(1.0, 0.3125).uv(0.9375, 0.3125)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.0, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.125, 0.5).uv(0.1875, 0.5).uv(0.1875, 0.4375).uv(0.125, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.0799999237060547, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.0625, 0.4375).uv(0.125, 0.4375).uv(0.125, 0.375).uv(0.0625, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.159999966621399, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.5625, 0.5).uv(0.625, 0.5).uv(0.625, 0.4375).uv(0.5625, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.2400000095367432, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.1875, 0.4375).uv(0.25, 0.4375).uv(0.25, 0.375).uv(0.1875, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.3199999332427979, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.0625, 0.4375).uv(0.125, 0.4375).uv(0.125, 0.375).uv(0.0625, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.399999976158142, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.1875, 0.5).uv(0.25, 0.5).uv(0.25, 0.4375).uv(0.1875, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.4800000190734863, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.25, 0.5).uv(0.3125, 0.5).uv(0.3125, 0.4375).uv(0.25, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.559999942779541, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.0625, 0.4375).uv(0.125, 0.4375).uv(0.125, 0.375).uv(0.0625, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.6399999856948853, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.125, 0.4375).uv(0.1875, 0.4375).uv(0.1875, 0.375).uv(0.125, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.71999990940094, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.75, 0.4375).uv(0.8125, 0.4375).uv(0.8125, 0.375).uv(0.75, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.7999999523162842, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.3125, 0.4375).uv(0.375, 0.4375).uv(0.375, 0.375).uv(0.3125, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                    }
                                                }
                                                Color.rgba(0.05999999865889549, 0.09000000357627869, 0.07999999821186066, 1.0) {
                                                    name = "__text_color"
                                                }
                                                Transform.position(1.087999939918518, -0.03999999538064003, -0.02500000037252903).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(2.247999906539917, 0.1679999828338623, 1.0) {
                                                    name = "__bg"
                                                    Color.rgba(0.9200000166893005, 0.9700000286102295, 0.9200000166893005, 1.0) {
                                                        Renderable.square() {
                                                            Raycastable.click_only() {
                                                                name = "__bg_raycastable"
                                                            }
                                                            RaycastableShape.quad_2d() {
                                                                name = "__bg_raycastable_shape"
                                                            }
                                                            Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                        }
                                                    }
                                                }
                                            }
                                            Transform.position(0.05199999734759331, -1.3119999170303345, 0.05000000074505806).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                                                name = "item_6"
                                                Raycastable.click_only() {
                                                    name = "item_6_raycastable"
                                                }
                                                Style {
                                                    name = "item_6_style"
                                                }
                                                Transform.position(0.0, 0.0, 0.019999999552965164).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                                                    name = "item_6_text_root"
                                                    Text {
                                                        "  transform"
                                                        name = "item_6_text"
                                                        Color.rgba(0.05999999865889549, 0.09000000357627869, 0.07999999821186066, 1.0) {
                                                            name = "item_6_text_color"
                                                        }
                                                        Transform.position(0.19999998807907104, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.25, 0.5).uv(0.3125, 0.5).uv(0.3125, 0.4375).uv(0.25, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.2800000011920929, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.125, 0.5).uv(0.1875, 0.5).uv(0.1875, 0.4375).uv(0.125, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.35999998450279236, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.0625, 0.4375).uv(0.125, 0.4375).uv(0.125, 0.375).uv(0.0625, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.4399999976158142, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.875, 0.4375).uv(0.9375, 0.4375).uv(0.9375, 0.375).uv(0.875, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.5199999809265137, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.1875, 0.5).uv(0.25, 0.5).uv(0.25, 0.4375).uv(0.1875, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.5999999642372131, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.375, 0.4375).uv(0.4375, 0.4375).uv(0.4375, 0.375).uv(0.375, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.6800000071525574, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.9375, 0.4375).uv(1.0, 0.4375).uv(1.0, 0.375).uv(0.9375, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.7599999904632568, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.125, 0.5).uv(0.1875, 0.5).uv(0.1875, 0.4375).uv(0.125, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.8399999737739563, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.8125, 0.4375).uv(0.875, 0.4375).uv(0.875, 0.375).uv(0.8125, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                    }
                                                }
                                                Color.rgba(0.05999999865889549, 0.09000000357627869, 0.07999999821186066, 1.0) {
                                                    name = "__text_color"
                                                }
                                                Transform.position(1.087999939918518, -0.03999999538064003, -0.02500000037252903).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(2.247999906539917, 0.1679999828338623, 1.0) {
                                                    name = "__bg"
                                                    Color.rgba(0.9200000166893005, 0.9700000286102295, 0.9200000166893005, 1.0) {
                                                        Renderable.square() {
                                                            Raycastable.click_only() {
                                                                name = "__bg_raycastable"
                                                            }
                                                            RaycastableShape.quad_2d() {
                                                                name = "__bg_raycastable_shape"
                                                            }
                                                            Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                        }
                                                    }
                                                }
                                            }
                                            Transform.position(0.05199999734759331, -1.5199999809265137, 0.05000000074505806).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                                                name = "item_7"
                                                Raycastable.click_only() {
                                                    name = "item_7_raycastable"
                                                }
                                                Style {
                                                    name = "item_7_style"
                                                }
                                                Transform.position(0.0, 0.0, 0.019999999552965164).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                                                    name = "item_7_text_root"
                                                    Text {
                                                        "    renderable"
                                                        name = "item_7_text"
                                                        Color.rgba(0.05999999865889549, 0.09000000357627869, 0.07999999821186066, 1.0) {
                                                            name = "item_7_text_color"
                                                        }
                                                        Transform.position(0.35999998450279236, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.125, 0.5).uv(0.1875, 0.5).uv(0.1875, 0.4375).uv(0.125, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.4399999976158142, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.3125, 0.4375).uv(0.375, 0.4375).uv(0.375, 0.375).uv(0.3125, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.5199999809265137, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.875, 0.4375).uv(0.9375, 0.4375).uv(0.9375, 0.375).uv(0.875, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.5999999642372131, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.25, 0.4375).uv(0.3125, 0.4375).uv(0.3125, 0.375).uv(0.25, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.6800000071525574, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.3125, 0.4375).uv(0.375, 0.4375).uv(0.375, 0.375).uv(0.3125, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.7599999904632568, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.125, 0.5).uv(0.1875, 0.5).uv(0.1875, 0.4375).uv(0.125, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.8399999737739563, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.0625, 0.4375).uv(0.125, 0.4375).uv(0.125, 0.375).uv(0.0625, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.9199999570846558, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.125, 0.4375).uv(0.1875, 0.4375).uv(0.1875, 0.375).uv(0.125, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.0, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.75, 0.4375).uv(0.8125, 0.4375).uv(0.8125, 0.375).uv(0.75, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.0799999237060547, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.3125, 0.4375).uv(0.375, 0.4375).uv(0.375, 0.375).uv(0.3125, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                    }
                                                }
                                                Color.rgba(0.05999999865889549, 0.09000000357627869, 0.07999999821186066, 1.0) {
                                                    name = "__text_color"
                                                }
                                                Transform.position(1.087999939918518, -0.03999999538064003, -0.02500000037252903).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(2.247999906539917, 0.1679999828338623, 1.0) {
                                                    name = "__bg"
                                                    Color.rgba(0.9200000166893005, 0.9700000286102295, 0.9200000166893005, 1.0) {
                                                        Renderable.square() {
                                                            Raycastable.click_only() {
                                                                name = "__bg_raycastable"
                                                            }
                                                            RaycastableShape.quad_2d() {
                                                                name = "__bg_raycastable_shape"
                                                            }
                                                            Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                        }
                                                    }
                                                }
                                            }
                                            Transform.position(0.05199999734759331, -1.7280000448226929, 0.05000000074505806).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                                                name = "item_8"
                                                Raycastable.click_only() {
                                                    name = "item_8_raycastable"
                                                }
                                                Style {
                                                    name = "item_8_style"
                                                }
                                                Transform.position(0.0, 0.0, 0.019999999552965164).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                                                    name = "item_8_text_root"
                                                    Text {
                                                        "      color"
                                                        name = "item_8_text"
                                                        Color.rgba(0.05999999865889549, 0.09000000357627869, 0.07999999821186066, 1.0) {
                                                            name = "item_8_text_color"
                                                        }
                                                        Transform.position(0.5199999809265137, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.1875, 0.4375).uv(0.25, 0.4375).uv(0.25, 0.375).uv(0.1875, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.5999999642372131, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.9375, 0.4375).uv(1.0, 0.4375).uv(1.0, 0.375).uv(0.9375, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.6800000071525574, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.75, 0.4375).uv(0.8125, 0.4375).uv(0.8125, 0.375).uv(0.75, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.7599999904632568, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.9375, 0.4375).uv(1.0, 0.4375).uv(1.0, 0.375).uv(0.9375, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.8399999737739563, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.125, 0.5).uv(0.1875, 0.5).uv(0.1875, 0.4375).uv(0.125, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                    }
                                                }
                                                Color.rgba(0.05999999865889549, 0.09000000357627869, 0.07999999821186066, 1.0) {
                                                    name = "__text_color"
                                                }
                                                Transform.position(1.087999939918518, -0.03999999538064003, -0.02500000037252903).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(2.247999906539917, 0.1679999828338623, 1.0) {
                                                    name = "__bg"
                                                    Color.rgba(0.9200000166893005, 0.9700000286102295, 0.9200000166893005, 1.0) {
                                                        Renderable.square() {
                                                            Raycastable.click_only() {
                                                                name = "__bg_raycastable"
                                                            }
                                                            RaycastableShape.quad_2d() {
                                                                name = "__bg_raycastable_shape"
                                                            }
                                                            Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                        }
                                                    }
                                                }
                                            }
                                            Transform.position(0.05199999734759331, -1.9359999895095825, 0.05000000074505806).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                                                name = "item_9"
                                                Raycastable.click_only() {
                                                    name = "item_9_raycastable"
                                                }
                                                Style {
                                                    name = "item_9_style"
                                                }
                                                Transform.position(0.0, 0.0, 0.019999999552965164).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                                                    name = "item_9_text_root"
                                                    Text {
                                                        "editor_auto_raycastable"
                                                        name = "item_9_text"
                                                        Color.rgba(0.05999999865889549, 0.09000000357627869, 0.07999999821186066, 1.0) {
                                                            name = "item_9_text_color"
                                                        }
                                                        Transform.position(0.03999999910593033, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.3125, 0.4375).uv(0.375, 0.4375).uv(0.375, 0.375).uv(0.3125, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.11999999731779099, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.25, 0.4375).uv(0.3125, 0.4375).uv(0.3125, 0.375).uv(0.25, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.19999998807907104, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.5625, 0.4375).uv(0.625, 0.4375).uv(0.625, 0.375).uv(0.5625, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.2800000011920929, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.25, 0.5).uv(0.3125, 0.5).uv(0.3125, 0.4375).uv(0.25, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.35999998450279236, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.9375, 0.4375).uv(1.0, 0.4375).uv(1.0, 0.375).uv(0.9375, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.4399999976158142, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.125, 0.5).uv(0.1875, 0.5).uv(0.1875, 0.4375).uv(0.125, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.5199999809265137, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.9375, 0.375).uv(1.0, 0.375).uv(1.0, 0.3125).uv(0.9375, 0.3125)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.5999999642372131, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.0625, 0.4375).uv(0.125, 0.4375).uv(0.125, 0.375).uv(0.0625, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.6800000071525574, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.3125, 0.5).uv(0.375, 0.5).uv(0.375, 0.4375).uv(0.3125, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.7599999904632568, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.25, 0.5).uv(0.3125, 0.5).uv(0.3125, 0.4375).uv(0.25, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.8399999737739563, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.9375, 0.4375).uv(1.0, 0.4375).uv(1.0, 0.375).uv(0.9375, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.9199999570846558, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.9375, 0.375).uv(1.0, 0.375).uv(1.0, 0.3125).uv(0.9375, 0.3125)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.0, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.125, 0.5).uv(0.1875, 0.5).uv(0.1875, 0.4375).uv(0.125, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.0799999237060547, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.0625, 0.4375).uv(0.125, 0.4375).uv(0.125, 0.375).uv(0.0625, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.159999966621399, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.5625, 0.5).uv(0.625, 0.5).uv(0.625, 0.4375).uv(0.5625, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.2400000095367432, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.1875, 0.4375).uv(0.25, 0.4375).uv(0.25, 0.375).uv(0.1875, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.3199999332427979, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.0625, 0.4375).uv(0.125, 0.4375).uv(0.125, 0.375).uv(0.0625, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.399999976158142, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.1875, 0.5).uv(0.25, 0.5).uv(0.25, 0.4375).uv(0.1875, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.4800000190734863, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.25, 0.5).uv(0.3125, 0.5).uv(0.3125, 0.4375).uv(0.25, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.559999942779541, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.0625, 0.4375).uv(0.125, 0.4375).uv(0.125, 0.375).uv(0.0625, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.6399999856948853, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.125, 0.4375).uv(0.1875, 0.4375).uv(0.1875, 0.375).uv(0.125, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.71999990940094, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.75, 0.4375).uv(0.8125, 0.4375).uv(0.8125, 0.375).uv(0.75, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.7999999523162842, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.3125, 0.4375).uv(0.375, 0.4375).uv(0.375, 0.375).uv(0.3125, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                    }
                                                }
                                                Color.rgba(0.05999999865889549, 0.09000000357627869, 0.07999999821186066, 1.0) {
                                                    name = "__text_color"
                                                }
                                                Transform.position(1.087999939918518, -0.03999999538064003, -0.02500000037252903).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(2.247999906539917, 0.1679999828338623, 1.0) {
                                                    name = "__bg"
                                                    Color.rgba(0.9200000166893005, 0.9700000286102295, 0.9200000166893005, 1.0) {
                                                        Renderable.square() {
                                                            Raycastable.click_only() {
                                                                name = "__bg_raycastable"
                                                            }
                                                            RaycastableShape.quad_2d() {
                                                                name = "__bg_raycastable_shape"
                                                            }
                                                            Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                        }
                                                    }
                                                }
                                            }
                                            Transform.position(0.05199999734759331, -2.1440000534057617, 0.05000000074505806).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                                                name = "item_10"
                                                Raycastable.click_only() {
                                                    name = "item_10_raycastable"
                                                }
                                                Style {
                                                    name = "item_10_style"
                                                }
                                                Transform.position(0.0, 0.0, 0.019999999552965164).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                                                    name = "item_10_text_root"
                                                    Text {
                                                        "  transform"
                                                        name = "item_10_text"
                                                        Color.rgba(0.05999999865889549, 0.09000000357627869, 0.07999999821186066, 1.0) {
                                                            name = "item_10_text_color"
                                                        }
                                                        Transform.position(0.19999998807907104, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.25, 0.5).uv(0.3125, 0.5).uv(0.3125, 0.4375).uv(0.25, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.2800000011920929, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.125, 0.5).uv(0.1875, 0.5).uv(0.1875, 0.4375).uv(0.125, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.35999998450279236, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.0625, 0.4375).uv(0.125, 0.4375).uv(0.125, 0.375).uv(0.0625, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.4399999976158142, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.875, 0.4375).uv(0.9375, 0.4375).uv(0.9375, 0.375).uv(0.875, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.5199999809265137, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.1875, 0.5).uv(0.25, 0.5).uv(0.25, 0.4375).uv(0.1875, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.5999999642372131, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.375, 0.4375).uv(0.4375, 0.4375).uv(0.4375, 0.375).uv(0.375, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.6800000071525574, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.9375, 0.4375).uv(1.0, 0.4375).uv(1.0, 0.375).uv(0.9375, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.7599999904632568, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.125, 0.5).uv(0.1875, 0.5).uv(0.1875, 0.4375).uv(0.125, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.8399999737739563, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.8125, 0.4375).uv(0.875, 0.4375).uv(0.875, 0.375).uv(0.8125, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                    }
                                                }
                                                Color.rgba(0.05999999865889549, 0.09000000357627869, 0.07999999821186066, 1.0) {
                                                    name = "__text_color"
                                                }
                                                Transform.position(1.087999939918518, -0.03999999538064003, -0.02500000037252903).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(2.247999906539917, 0.1679999828338623, 1.0) {
                                                    name = "__bg"
                                                    Color.rgba(0.9200000166893005, 0.9700000286102295, 0.9200000166893005, 1.0) {
                                                        Renderable.square() {
                                                            Raycastable.click_only() {
                                                                name = "__bg_raycastable"
                                                            }
                                                            RaycastableShape.quad_2d() {
                                                                name = "__bg_raycastable_shape"
                                                            }
                                                            Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                        }
                                                    }
                                                }
                                            }
                                            Transform.position(0.05199999734759331, -2.3519999980926514, 0.05000000074505806).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                                                name = "item_11"
                                                Raycastable.click_only() {
                                                    name = "item_11_raycastable"
                                                }
                                                Style {
                                                    name = "item_11_style"
                                                }
                                                Transform.position(0.0, 0.0, 0.019999999552965164).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                                                    name = "item_11_text_root"
                                                    Text {
                                                        "    transform"
                                                        name = "item_11_text"
                                                        Color.rgba(0.05999999865889549, 0.09000000357627869, 0.07999999821186066, 1.0) {
                                                            name = "item_11_text_color"
                                                        }
                                                        Transform.position(0.35999998450279236, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.25, 0.5).uv(0.3125, 0.5).uv(0.3125, 0.4375).uv(0.25, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.4399999976158142, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.125, 0.5).uv(0.1875, 0.5).uv(0.1875, 0.4375).uv(0.125, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.5199999809265137, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.0625, 0.4375).uv(0.125, 0.4375).uv(0.125, 0.375).uv(0.0625, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.5999999642372131, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.875, 0.4375).uv(0.9375, 0.4375).uv(0.9375, 0.375).uv(0.875, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.6800000071525574, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.1875, 0.5).uv(0.25, 0.5).uv(0.25, 0.4375).uv(0.1875, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.7599999904632568, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.375, 0.4375).uv(0.4375, 0.4375).uv(0.4375, 0.375).uv(0.375, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.8399999737739563, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.9375, 0.4375).uv(1.0, 0.4375).uv(1.0, 0.375).uv(0.9375, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.9199999570846558, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.125, 0.5).uv(0.1875, 0.5).uv(0.1875, 0.4375).uv(0.125, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.0, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.8125, 0.4375).uv(0.875, 0.4375).uv(0.875, 0.375).uv(0.8125, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                    }
                                                }
                                                Color.rgba(0.05999999865889549, 0.09000000357627869, 0.07999999821186066, 1.0) {
                                                    name = "__text_color"
                                                }
                                                Transform.position(1.087999939918518, -0.03999999538064003, -0.02500000037252903).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(2.247999906539917, 0.1679999828338623, 1.0) {
                                                    name = "__bg"
                                                    Color.rgba(0.9200000166893005, 0.9700000286102295, 0.9200000166893005, 1.0) {
                                                        Renderable.square() {
                                                            Raycastable.click_only() {
                                                                name = "__bg_raycastable"
                                                            }
                                                            RaycastableShape.quad_2d() {
                                                                name = "__bg_raycastable_shape"
                                                            }
                                                            Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                        }
                                                    }
                                                }
                                            }
                                            Transform.position(0.05199999734759331, -2.56000018119812, 0.05000000074505806).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                                                name = "item_12"
                                                Raycastable.click_only() {
                                                    name = "item_12_raycastable"
                                                }
                                                Style {
                                                    name = "item_12_style"
                                                }
                                                Transform.position(0.0, 0.0, 0.019999999552965164).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                                                    name = "item_12_text_root"
                                                    Text {
                                                        "      renderable"
                                                        name = "item_12_text"
                                                        Color.rgba(0.05999999865889549, 0.09000000357627869, 0.07999999821186066, 1.0) {
                                                            name = "item_12_text_color"
                                                        }
                                                        Transform.position(0.5199999809265137, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.125, 0.5).uv(0.1875, 0.5).uv(0.1875, 0.4375).uv(0.125, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.5999999642372131, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.3125, 0.4375).uv(0.375, 0.4375).uv(0.375, 0.375).uv(0.3125, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.6800000071525574, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.875, 0.4375).uv(0.9375, 0.4375).uv(0.9375, 0.375).uv(0.875, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.7599999904632568, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.25, 0.4375).uv(0.3125, 0.4375).uv(0.3125, 0.375).uv(0.25, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.8399999737739563, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.3125, 0.4375).uv(0.375, 0.4375).uv(0.375, 0.375).uv(0.3125, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.9199999570846558, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.125, 0.5).uv(0.1875, 0.5).uv(0.1875, 0.4375).uv(0.125, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.0, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.0625, 0.4375).uv(0.125, 0.4375).uv(0.125, 0.375).uv(0.0625, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.0799999237060547, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.125, 0.4375).uv(0.1875, 0.4375).uv(0.1875, 0.375).uv(0.125, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.159999966621399, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.75, 0.4375).uv(0.8125, 0.4375).uv(0.8125, 0.375).uv(0.75, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.2400000095367432, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.3125, 0.4375).uv(0.375, 0.4375).uv(0.375, 0.375).uv(0.3125, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                    }
                                                }
                                                Color.rgba(0.05999999865889549, 0.09000000357627869, 0.07999999821186066, 1.0) {
                                                    name = "__text_color"
                                                }
                                                Transform.position(1.087999939918518, -0.03999999538064003, -0.02500000037252903).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(2.247999906539917, 0.1679999828338623, 1.0) {
                                                    name = "__bg"
                                                    Color.rgba(0.9200000166893005, 0.9700000286102295, 0.9200000166893005, 1.0) {
                                                        Renderable.square() {
                                                            Raycastable.click_only() {
                                                                name = "__bg_raycastable"
                                                            }
                                                            RaycastableShape.quad_2d() {
                                                                name = "__bg_raycastable_shape"
                                                            }
                                                            Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                        }
                                                    }
                                                }
                                            }
                                            Transform.position(0.05199999734759331, -2.7680001258850098, 0.05000000074505806).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                                                name = "item_13"
                                                Raycastable.click_only() {
                                                    name = "item_13_raycastable"
                                                }
                                                Style {
                                                    name = "item_13_style"
                                                }
                                                Transform.position(0.0, 0.0, 0.019999999552965164).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                                                    name = "item_13_text_root"
                                                    Text {
                                                        "        color"
                                                        name = "item_13_text"
                                                        Color.rgba(0.05999999865889549, 0.09000000357627869, 0.07999999821186066, 1.0) {
                                                            name = "item_13_text_color"
                                                        }
                                                        Transform.position(0.6800000071525574, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.1875, 0.4375).uv(0.25, 0.4375).uv(0.25, 0.375).uv(0.1875, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.7599999904632568, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.9375, 0.4375).uv(1.0, 0.4375).uv(1.0, 0.375).uv(0.9375, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.8399999737739563, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.75, 0.4375).uv(0.8125, 0.4375).uv(0.8125, 0.375).uv(0.75, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.9199999570846558, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.9375, 0.4375).uv(1.0, 0.4375).uv(1.0, 0.375).uv(0.9375, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.0, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.125, 0.5).uv(0.1875, 0.5).uv(0.1875, 0.4375).uv(0.125, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                    }
                                                }
                                                Color.rgba(0.05999999865889549, 0.09000000357627869, 0.07999999821186066, 1.0) {
                                                    name = "__text_color"
                                                }
                                                Transform.position(1.087999939918518, -0.03999999538064003, -0.02500000037252903).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(2.247999906539917, 0.1679999828338623, 1.0) {
                                                    name = "__bg"
                                                    Color.rgba(0.9200000166893005, 0.9700000286102295, 0.9200000166893005, 1.0) {
                                                        Renderable.square() {
                                                            Raycastable.click_only() {
                                                                name = "__bg_raycastable"
                                                            }
                                                            RaycastableShape.quad_2d() {
                                                                name = "__bg_raycastable_shape"
                                                            }
                                                            Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                        }
                                                    }
                                                }
                                            }
                                            Transform.position(0.05199999734759331, -2.9760000705718994, 0.05000000074505806).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                                                name = "item_14"
                                                Raycastable.click_only() {
                                                    name = "item_14_raycastable"
                                                }
                                                Style {
                                                    name = "item_14_style"
                                                }
                                                Transform.position(0.0, 0.0, 0.019999999552965164).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                                                    name = "item_14_text_root"
                                                    Text {
                                                        "    transform"
                                                        name = "item_14_text"
                                                        Color.rgba(0.05999999865889549, 0.09000000357627869, 0.07999999821186066, 1.0) {
                                                            name = "item_14_text_color"
                                                        }
                                                        Transform.position(0.35999998450279236, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.25, 0.5).uv(0.3125, 0.5).uv(0.3125, 0.4375).uv(0.25, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.4399999976158142, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.125, 0.5).uv(0.1875, 0.5).uv(0.1875, 0.4375).uv(0.125, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.5199999809265137, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.0625, 0.4375).uv(0.125, 0.4375).uv(0.125, 0.375).uv(0.0625, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.5999999642372131, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.875, 0.4375).uv(0.9375, 0.4375).uv(0.9375, 0.375).uv(0.875, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.6800000071525574, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.1875, 0.5).uv(0.25, 0.5).uv(0.25, 0.4375).uv(0.1875, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.7599999904632568, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.375, 0.4375).uv(0.4375, 0.4375).uv(0.4375, 0.375).uv(0.375, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.8399999737739563, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.9375, 0.4375).uv(1.0, 0.4375).uv(1.0, 0.375).uv(0.9375, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.9199999570846558, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.125, 0.5).uv(0.1875, 0.5).uv(0.1875, 0.4375).uv(0.125, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.0, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.8125, 0.4375).uv(0.875, 0.4375).uv(0.875, 0.375).uv(0.8125, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                    }
                                                }
                                                Color.rgba(0.05999999865889549, 0.09000000357627869, 0.07999999821186066, 1.0) {
                                                    name = "__text_color"
                                                }
                                                Transform.position(1.087999939918518, -0.03999999538064003, -0.02500000037252903).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(2.247999906539917, 0.1679999828338623, 1.0) {
                                                    name = "__bg"
                                                    Color.rgba(0.9200000166893005, 0.9700000286102295, 0.9200000166893005, 1.0) {
                                                        Renderable.square() {
                                                            Raycastable.click_only() {
                                                                name = "__bg_raycastable"
                                                            }
                                                            RaycastableShape.quad_2d() {
                                                                name = "__bg_raycastable_shape"
                                                            }
                                                            Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                        }
                                                    }
                                                }
                                            }
                                            Transform.position(0.05199999734759331, -3.18399977684021, 0.05000000074505806).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                                                name = "item_15"
                                                Raycastable.click_only() {
                                                    name = "item_15_raycastable"
                                                }
                                                Style {
                                                    name = "item_15_style"
                                                }
                                                Transform.position(0.0, 0.0, 0.019999999552965164).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                                                    name = "item_15_text_root"
                                                    Text {
                                                        "      renderable"
                                                        name = "item_15_text"
                                                        Color.rgba(0.05999999865889549, 0.09000000357627869, 0.07999999821186066, 1.0) {
                                                            name = "item_15_text_color"
                                                        }
                                                        Transform.position(0.5199999809265137, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.125, 0.5).uv(0.1875, 0.5).uv(0.1875, 0.4375).uv(0.125, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.5999999642372131, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.3125, 0.4375).uv(0.375, 0.4375).uv(0.375, 0.375).uv(0.3125, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.6800000071525574, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.875, 0.4375).uv(0.9375, 0.4375).uv(0.9375, 0.375).uv(0.875, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.7599999904632568, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.25, 0.4375).uv(0.3125, 0.4375).uv(0.3125, 0.375).uv(0.25, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.8399999737739563, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.3125, 0.4375).uv(0.375, 0.4375).uv(0.375, 0.375).uv(0.3125, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.9199999570846558, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.125, 0.5).uv(0.1875, 0.5).uv(0.1875, 0.4375).uv(0.125, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.0, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.0625, 0.4375).uv(0.125, 0.4375).uv(0.125, 0.375).uv(0.0625, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.0799999237060547, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.125, 0.4375).uv(0.1875, 0.4375).uv(0.1875, 0.375).uv(0.125, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.159999966621399, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.75, 0.4375).uv(0.8125, 0.4375).uv(0.8125, 0.375).uv(0.75, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.2400000095367432, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.3125, 0.4375).uv(0.375, 0.4375).uv(0.375, 0.375).uv(0.3125, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                    }
                                                }
                                                Color.rgba(0.05999999865889549, 0.09000000357627869, 0.07999999821186066, 1.0) {
                                                    name = "__text_color"
                                                }
                                                Transform.position(1.087999939918518, -0.03999999538064003, -0.02500000037252903).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(2.247999906539917, 0.1679999828338623, 1.0) {
                                                    name = "__bg"
                                                    Color.rgba(0.9200000166893005, 0.9700000286102295, 0.9200000166893005, 1.0) {
                                                        Renderable.square() {
                                                            Raycastable.click_only() {
                                                                name = "__bg_raycastable"
                                                            }
                                                            RaycastableShape.quad_2d() {
                                                                name = "__bg_raycastable_shape"
                                                            }
                                                            Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                        }
                                                    }
                                                }
                                            }
                                            Transform.position(0.05199999734759331, -3.3919997215270996, 0.05000000074505806).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                                                name = "item_16"
                                                Raycastable.click_only() {
                                                    name = "item_16_raycastable"
                                                }
                                                Style {
                                                    name = "item_16_style"
                                                }
                                                Transform.position(0.0, 0.0, 0.019999999552965164).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                                                    name = "item_16_text_root"
                                                    Text {
                                                        "        color"
                                                        name = "item_16_text"
                                                        Color.rgba(0.05999999865889549, 0.09000000357627869, 0.07999999821186066, 1.0) {
                                                            name = "item_16_text_color"
                                                        }
                                                        Transform.position(0.6800000071525574, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.1875, 0.4375).uv(0.25, 0.4375).uv(0.25, 0.375).uv(0.1875, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.7599999904632568, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.9375, 0.4375).uv(1.0, 0.4375).uv(1.0, 0.375).uv(0.9375, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.8399999737739563, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.75, 0.4375).uv(0.8125, 0.4375).uv(0.8125, 0.375).uv(0.75, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.9199999570846558, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.9375, 0.4375).uv(1.0, 0.4375).uv(1.0, 0.375).uv(0.9375, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.0, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.125, 0.5).uv(0.1875, 0.5).uv(0.1875, 0.4375).uv(0.125, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                    }
                                                }
                                                Color.rgba(0.05999999865889549, 0.09000000357627869, 0.07999999821186066, 1.0) {
                                                    name = "__text_color"
                                                }
                                                Transform.position(1.087999939918518, -0.03999999538064003, -0.02500000037252903).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(2.247999906539917, 0.1679999828338623, 1.0) {
                                                    name = "__bg"
                                                    Color.rgba(0.9200000166893005, 0.9700000286102295, 0.9200000166893005, 1.0) {
                                                        Renderable.square() {
                                                            Raycastable.click_only() {
                                                                name = "__bg_raycastable"
                                                            }
                                                            RaycastableShape.quad_2d() {
                                                                name = "__bg_raycastable_shape"
                                                            }
                                                            Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                        }
                                                    }
                                                }
                                            }
                                            Transform.position(0.05199999734759331, -3.5999996662139893, 0.05000000074505806).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                                                name = "item_17"
                                                Raycastable.click_only() {
                                                    name = "item_17_raycastable"
                                                }
                                                Style {
                                                    name = "item_17_style"
                                                }
                                                Transform.position(0.0, 0.0, 0.019999999552965164).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                                                    name = "item_17_text_root"
                                                    Text {
                                                        "    transform"
                                                        name = "item_17_text"
                                                        Color.rgba(0.05999999865889549, 0.09000000357627869, 0.07999999821186066, 1.0) {
                                                            name = "item_17_text_color"
                                                        }
                                                        Transform.position(0.35999998450279236, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.25, 0.5).uv(0.3125, 0.5).uv(0.3125, 0.4375).uv(0.25, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.4399999976158142, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.125, 0.5).uv(0.1875, 0.5).uv(0.1875, 0.4375).uv(0.125, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.5199999809265137, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.0625, 0.4375).uv(0.125, 0.4375).uv(0.125, 0.375).uv(0.0625, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.5999999642372131, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.875, 0.4375).uv(0.9375, 0.4375).uv(0.9375, 0.375).uv(0.875, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.6800000071525574, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.1875, 0.5).uv(0.25, 0.5).uv(0.25, 0.4375).uv(0.1875, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.7599999904632568, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.375, 0.4375).uv(0.4375, 0.4375).uv(0.4375, 0.375).uv(0.375, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.8399999737739563, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.9375, 0.4375).uv(1.0, 0.4375).uv(1.0, 0.375).uv(0.9375, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.9199999570846558, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.125, 0.5).uv(0.1875, 0.5).uv(0.1875, 0.4375).uv(0.125, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.0, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.8125, 0.4375).uv(0.875, 0.4375).uv(0.875, 0.375).uv(0.8125, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                    }
                                                }
                                                Color.rgba(0.05999999865889549, 0.09000000357627869, 0.07999999821186066, 1.0) {
                                                    name = "__text_color"
                                                }
                                                Transform.position(1.087999939918518, -0.03999999538064003, -0.02500000037252903).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(2.247999906539917, 0.1679999828338623, 1.0) {
                                                    name = "__bg"
                                                    Color.rgba(0.9200000166893005, 0.9700000286102295, 0.9200000166893005, 1.0) {
                                                        Renderable.square() {
                                                            Raycastable.click_only() {
                                                                name = "__bg_raycastable"
                                                            }
                                                            RaycastableShape.quad_2d() {
                                                                name = "__bg_raycastable_shape"
                                                            }
                                                            Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                        }
                                                    }
                                                }
                                            }
                                            Transform.position(0.05199999734759331, -3.8079993724823, 0.05000000074505806).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                                                name = "item_18"
                                                Raycastable.click_only() {
                                                    name = "item_18_raycastable"
                                                }
                                                Style {
                                                    name = "item_18_style"
                                                }
                                                Transform.position(0.0, 0.0, 0.019999999552965164).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                                                    name = "item_18_text_root"
                                                    Text {
                                                        "      renderable"
                                                        name = "item_18_text"
                                                        Color.rgba(0.05999999865889549, 0.09000000357627869, 0.07999999821186066, 1.0) {
                                                            name = "item_18_text_color"
                                                        }
                                                        Transform.position(0.5199999809265137, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.125, 0.5).uv(0.1875, 0.5).uv(0.1875, 0.4375).uv(0.125, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.5999999642372131, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.3125, 0.4375).uv(0.375, 0.4375).uv(0.375, 0.375).uv(0.3125, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.6800000071525574, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.875, 0.4375).uv(0.9375, 0.4375).uv(0.9375, 0.375).uv(0.875, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.7599999904632568, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.25, 0.4375).uv(0.3125, 0.4375).uv(0.3125, 0.375).uv(0.25, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.8399999737739563, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.3125, 0.4375).uv(0.375, 0.4375).uv(0.375, 0.375).uv(0.3125, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.9199999570846558, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.125, 0.5).uv(0.1875, 0.5).uv(0.1875, 0.4375).uv(0.125, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.0, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.0625, 0.4375).uv(0.125, 0.4375).uv(0.125, 0.375).uv(0.0625, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.0799999237060547, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.125, 0.4375).uv(0.1875, 0.4375).uv(0.1875, 0.375).uv(0.125, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.159999966621399, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.75, 0.4375).uv(0.8125, 0.4375).uv(0.8125, 0.375).uv(0.75, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.2400000095367432, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.3125, 0.4375).uv(0.375, 0.4375).uv(0.375, 0.375).uv(0.3125, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                    }
                                                }
                                                Color.rgba(0.05999999865889549, 0.09000000357627869, 0.07999999821186066, 1.0) {
                                                    name = "__text_color"
                                                }
                                                Transform.position(1.087999939918518, -0.03999999538064003, -0.02500000037252903).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(2.247999906539917, 0.1679999828338623, 1.0) {
                                                    name = "__bg"
                                                    Color.rgba(0.9200000166893005, 0.9700000286102295, 0.9200000166893005, 1.0) {
                                                        Renderable.square() {
                                                            Raycastable.click_only() {
                                                                name = "__bg_raycastable"
                                                            }
                                                            RaycastableShape.quad_2d() {
                                                                name = "__bg_raycastable_shape"
                                                            }
                                                            Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                        }
                                                    }
                                                }
                                            }
                                            Transform.position(0.05199999734759331, -4.0159993171691895, 0.05000000074505806).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                                                name = "item_19"
                                                Raycastable.click_only() {
                                                    name = "item_19_raycastable"
                                                }
                                                Style {
                                                    name = "item_19_style"
                                                }
                                                Transform.position(0.0, 0.0, 0.019999999552965164).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                                                    name = "item_19_text_root"
                                                    Text {
                                                        "        color"
                                                        name = "item_19_text"
                                                        Color.rgba(0.05999999865889549, 0.09000000357627869, 0.07999999821186066, 1.0) {
                                                            name = "item_19_text_color"
                                                        }
                                                        Transform.position(0.6800000071525574, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.1875, 0.4375).uv(0.25, 0.4375).uv(0.25, 0.375).uv(0.1875, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.7599999904632568, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.9375, 0.4375).uv(1.0, 0.4375).uv(1.0, 0.375).uv(0.9375, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.8399999737739563, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.75, 0.4375).uv(0.8125, 0.4375).uv(0.8125, 0.375).uv(0.75, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.9199999570846558, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.9375, 0.4375).uv(1.0, 0.4375).uv(1.0, 0.375).uv(0.9375, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.0, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.125, 0.5).uv(0.1875, 0.5).uv(0.1875, 0.4375).uv(0.125, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                    }
                                                }
                                                Color.rgba(0.05999999865889549, 0.09000000357627869, 0.07999999821186066, 1.0) {
                                                    name = "__text_color"
                                                }
                                                Transform.position(1.087999939918518, -0.03999999538064003, -0.02500000037252903).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(2.247999906539917, 0.1679999828338623, 1.0) {
                                                    name = "__bg"
                                                    Color.rgba(0.9200000166893005, 0.9700000286102295, 0.9200000166893005, 1.0) {
                                                        Renderable.square() {
                                                            Raycastable.click_only() {
                                                                name = "__bg_raycastable"
                                                            }
                                                            RaycastableShape.quad_2d() {
                                                                name = "__bg_raycastable_shape"
                                                            }
                                                            Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                        }
                                                    }
                                                }
                                            }
                                            Transform.position(0.0, -4.159999370574951, 0.05000000074505806).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                                                name = "item_20"
                                                Style {
                                                    name = "item_20_style"
                                                }
                                            }
                                            Transform.position(0.05199999734759331, -4.287999153137207, 0.05000000074505806).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                                                name = "item_21"
                                                Style {
                                                    name = "item_21_style"
                                                }
                                                Transform.position(0.0, 0.0, 0.019999999552965164).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                                                    name = "item_21_text_root"
                                                    Text {
                                                        "Editor { id=36v1 }"
                                                        name = "item_21_text"
                                                        Color.rgba(0.0, 0.0, 0.0, 1.0) {
                                                            name = "item_21_text_color"
                                                        }
                                                        Transform.position(0.03999999910593033, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.3125, 0.3125).uv(0.375, 0.3125).uv(0.375, 0.25).uv(0.3125, 0.25)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.11999999731779099, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.25, 0.4375).uv(0.3125, 0.4375).uv(0.3125, 0.375).uv(0.25, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.19999998807907104, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.5625, 0.4375).uv(0.625, 0.4375).uv(0.625, 0.375).uv(0.5625, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.2800000011920929, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.25, 0.5).uv(0.3125, 0.5).uv(0.3125, 0.4375).uv(0.25, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.35999998450279236, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.9375, 0.4375).uv(1.0, 0.4375).uv(1.0, 0.375).uv(0.9375, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.4399999976158142, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.125, 0.5).uv(0.1875, 0.5).uv(0.1875, 0.4375).uv(0.125, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.5999999642372131, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.6875, 0.5).uv(0.75, 0.5).uv(0.75, 0.4375).uv(0.6875, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.7599999904632568, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.5625, 0.4375).uv(0.625, 0.4375).uv(0.625, 0.375).uv(0.5625, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.8399999737739563, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.25, 0.4375).uv(0.3125, 0.4375).uv(0.3125, 0.375).uv(0.25, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.9199999570846558, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.8125, 0.25).uv(0.875, 0.25).uv(0.875, 0.1875).uv(0.8125, 0.1875)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.0, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.1875, 0.25).uv(0.25, 0.25).uv(0.25, 0.1875).uv(0.1875, 0.1875)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.0799999237060547, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.375, 0.25).uv(0.4375, 0.25).uv(0.4375, 0.1875).uv(0.375, 0.1875)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.159999966621399, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.375, 0.5).uv(0.4375, 0.5).uv(0.4375, 0.4375).uv(0.375, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.2400000095367432, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.0625, 0.25).uv(0.125, 0.25).uv(0.125, 0.1875).uv(0.0625, 0.1875)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.399999976158142, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.8125, 0.5).uv(0.875, 0.5).uv(0.875, 0.4375).uv(0.8125, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                    }
                                                }
                                                Color.rgba(0.0, 0.0, 0.0, 1.0) {
                                                    name = "__text_color"
                                                }
                                                Transform.position(1.087999939918518, -0.03999999538064003, -0.02500000037252903).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(2.247999906539917, 0.1679999828338623, 1.0) {
                                                    name = "__bg"
                                                    Color.rgba(0.18000000715255737, 0.7799999713897705, 0.2199999988079071, 0.949999988079071) {
                                                        Renderable.square() {
                                                            Opacity.opacity(0.949999988079071)
                                                            Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                        }
                                                    }
                                                }
                                            }
                                            Transform.position(0.05199999734759331, -4.495998859405518, 0.05000000074505806).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                                                name = "item_22"
                                                Raycastable.click_only() {
                                                    name = "item_22_raycastable"
                                                }
                                                Style {
                                                    name = "item_22_style"
                                                }
                                                Transform.position(0.0, 0.0, 0.019999999552965164).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                                                    name = "item_22_text_root"
                                                    Text {
                                                        "input_xr"
                                                        name = "item_22_text"
                                                        Color.rgba(0.05999999865889549, 0.09000000357627869, 0.07999999821186066, 1.0) {
                                                            name = "item_22_text_color"
                                                        }
                                                        Transform.position(0.03999999910593033, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.5625, 0.4375).uv(0.625, 0.4375).uv(0.625, 0.375).uv(0.5625, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.11999999731779099, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.875, 0.4375).uv(0.9375, 0.4375).uv(0.9375, 0.375).uv(0.875, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.19999998807907104, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.0, 0.5).uv(0.0625, 0.5).uv(0.0625, 0.4375).uv(0.0, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.2800000011920929, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.3125, 0.5).uv(0.375, 0.5).uv(0.375, 0.4375).uv(0.3125, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.35999998450279236, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.25, 0.5).uv(0.3125, 0.5).uv(0.3125, 0.4375).uv(0.25, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.4399999976158142, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.9375, 0.375).uv(1.0, 0.375).uv(1.0, 0.3125).uv(0.9375, 0.3125)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.5199999809265137, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.5, 0.5).uv(0.5625, 0.5).uv(0.5625, 0.4375).uv(0.5, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.5999999642372131, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.125, 0.5).uv(0.1875, 0.5).uv(0.1875, 0.4375).uv(0.125, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                    }
                                                }
                                                Color.rgba(0.05999999865889549, 0.09000000357627869, 0.07999999821186066, 1.0) {
                                                    name = "__text_color"
                                                }
                                                Transform.position(1.087999939918518, -0.03999999538064003, -0.02500000037252903).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(2.247999906539917, 0.1679999828338623, 1.0) {
                                                    name = "__bg"
                                                    Color.rgba(0.9200000166893005, 0.9700000286102295, 0.9200000166893005, 1.0) {
                                                        Renderable.square() {
                                                            Raycastable.click_only() {
                                                                name = "__bg_raycastable"
                                                            }
                                                            RaycastableShape.quad_2d() {
                                                                name = "__bg_raycastable_shape"
                                                            }
                                                            Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                        }
                                                    }
                                                }
                                            }
                                            Transform.position(0.05199999734759331, -4.703999042510986, 0.05000000074505806).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                                                name = "item_23"
                                                Raycastable.click_only() {
                                                    name = "item_23_raycastable"
                                                }
                                                Style {
                                                    name = "item_23_style"
                                                }
                                                Transform.position(0.0, 0.0, 0.019999999552965164).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                                                    name = "item_23_text_root"
                                                    Text {
                                                        "  transform"
                                                        name = "item_23_text"
                                                        Color.rgba(0.05999999865889549, 0.09000000357627869, 0.07999999821186066, 1.0) {
                                                            name = "item_23_text_color"
                                                        }
                                                        Transform.position(0.19999998807907104, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.25, 0.5).uv(0.3125, 0.5).uv(0.3125, 0.4375).uv(0.25, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.2800000011920929, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.125, 0.5).uv(0.1875, 0.5).uv(0.1875, 0.4375).uv(0.125, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.35999998450279236, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.0625, 0.4375).uv(0.125, 0.4375).uv(0.125, 0.375).uv(0.0625, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.4399999976158142, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.875, 0.4375).uv(0.9375, 0.4375).uv(0.9375, 0.375).uv(0.875, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.5199999809265137, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.1875, 0.5).uv(0.25, 0.5).uv(0.25, 0.4375).uv(0.1875, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.5999999642372131, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.375, 0.4375).uv(0.4375, 0.4375).uv(0.4375, 0.375).uv(0.375, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.6800000071525574, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.9375, 0.4375).uv(1.0, 0.4375).uv(1.0, 0.375).uv(0.9375, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.7599999904632568, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.125, 0.5).uv(0.1875, 0.5).uv(0.1875, 0.4375).uv(0.125, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.8399999737739563, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.8125, 0.4375).uv(0.875, 0.4375).uv(0.875, 0.375).uv(0.8125, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                    }
                                                }
                                                Color.rgba(0.05999999865889549, 0.09000000357627869, 0.07999999821186066, 1.0) {
                                                    name = "__text_color"
                                                }
                                                Transform.position(1.087999939918518, -0.03999999538064003, -0.02500000037252903).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(2.247999906539917, 0.1679999828338623, 1.0) {
                                                    name = "__bg"
                                                    Color.rgba(0.9200000166893005, 0.9700000286102295, 0.9200000166893005, 1.0) {
                                                        Renderable.square() {
                                                            Raycastable.click_only() {
                                                                name = "__bg_raycastable"
                                                            }
                                                            RaycastableShape.quad_2d() {
                                                                name = "__bg_raycastable_shape"
                                                            }
                                                            Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                        }
                                                    }
                                                }
                                            }
                                            Transform.position(0.05199999734759331, -4.911998748779297, 0.05000000074505806).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                                                name = "item_24"
                                                Raycastable.click_only() {
                                                    name = "item_24_raycastable"
                                                }
                                                Style {
                                                    name = "item_24_style"
                                                }
                                                Transform.position(0.0, 0.0, 0.019999999552965164).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                                                    name = "item_24_text_root"
                                                    Text {
                                                        "    avatar_control"
                                                        name = "item_24_text"
                                                        Color.rgba(0.05999999865889549, 0.09000000357627869, 0.07999999821186066, 1.0) {
                                                            name = "item_24_text_color"
                                                        }
                                                        Transform.position(0.35999998450279236, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.0625, 0.4375).uv(0.125, 0.4375).uv(0.125, 0.375).uv(0.0625, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.4399999976158142, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.375, 0.5).uv(0.4375, 0.5).uv(0.4375, 0.4375).uv(0.375, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.5199999809265137, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.0625, 0.4375).uv(0.125, 0.4375).uv(0.125, 0.375).uv(0.0625, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.5999999642372131, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.25, 0.5).uv(0.3125, 0.5).uv(0.3125, 0.4375).uv(0.25, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.6800000071525574, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.0625, 0.4375).uv(0.125, 0.4375).uv(0.125, 0.375).uv(0.0625, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.7599999904632568, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.125, 0.5).uv(0.1875, 0.5).uv(0.1875, 0.4375).uv(0.125, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.8399999737739563, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.9375, 0.375).uv(1.0, 0.375).uv(1.0, 0.3125).uv(0.9375, 0.3125)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.9199999570846558, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.1875, 0.4375).uv(0.25, 0.4375).uv(0.25, 0.375).uv(0.1875, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.0, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.9375, 0.4375).uv(1.0, 0.4375).uv(1.0, 0.375).uv(0.9375, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.0799999237060547, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.875, 0.4375).uv(0.9375, 0.4375).uv(0.9375, 0.375).uv(0.875, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.159999966621399, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.25, 0.5).uv(0.3125, 0.5).uv(0.3125, 0.4375).uv(0.25, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.2400000095367432, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.125, 0.5).uv(0.1875, 0.5).uv(0.1875, 0.4375).uv(0.125, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.3199999332427979, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.9375, 0.4375).uv(1.0, 0.4375).uv(1.0, 0.375).uv(0.9375, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.399999976158142, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.75, 0.4375).uv(0.8125, 0.4375).uv(0.8125, 0.375).uv(0.75, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                    }
                                                }
                                                Color.rgba(0.05999999865889549, 0.09000000357627869, 0.07999999821186066, 1.0) {
                                                    name = "__text_color"
                                                }
                                                Transform.position(1.087999939918518, -0.03999999538064003, -0.02500000037252903).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(2.247999906539917, 0.1679999828338623, 1.0) {
                                                    name = "__bg"
                                                    Color.rgba(0.9200000166893005, 0.9700000286102295, 0.9200000166893005, 1.0) {
                                                        Renderable.square() {
                                                            Raycastable.click_only() {
                                                                name = "__bg_raycastable"
                                                            }
                                                            RaycastableShape.quad_2d() {
                                                                name = "__bg_raycastable_shape"
                                                            }
                                                            Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                        }
                                                    }
                                                }
                                            }
                                            Transform.position(0.05199999734759331, -5.119998455047607, 0.05000000074505806).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                                                name = "item_25"
                                                Raycastable.click_only() {
                                                    name = "item_25_raycastable"
                                                }
                                                Style {
                                                    name = "item_25_style"
                                                }
                                                Transform.position(0.0, 0.0, 0.019999999552965164).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                                                    name = "item_25_text_root"
                                                    Text {
                                                        "      transform"
                                                        name = "item_25_text"
                                                        Color.rgba(0.05999999865889549, 0.09000000357627869, 0.07999999821186066, 1.0) {
                                                            name = "item_25_text_color"
                                                        }
                                                        Transform.position(0.5199999809265137, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.25, 0.5).uv(0.3125, 0.5).uv(0.3125, 0.4375).uv(0.25, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.5999999642372131, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.125, 0.5).uv(0.1875, 0.5).uv(0.1875, 0.4375).uv(0.125, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.6800000071525574, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.0625, 0.4375).uv(0.125, 0.4375).uv(0.125, 0.375).uv(0.0625, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.7599999904632568, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.875, 0.4375).uv(0.9375, 0.4375).uv(0.9375, 0.375).uv(0.875, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.8399999737739563, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.1875, 0.5).uv(0.25, 0.5).uv(0.25, 0.4375).uv(0.1875, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.9199999570846558, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.375, 0.4375).uv(0.4375, 0.4375).uv(0.4375, 0.375).uv(0.375, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.0, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.9375, 0.4375).uv(1.0, 0.4375).uv(1.0, 0.375).uv(0.9375, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.0799999237060547, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.125, 0.5).uv(0.1875, 0.5).uv(0.1875, 0.4375).uv(0.125, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.159999966621399, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.8125, 0.4375).uv(0.875, 0.4375).uv(0.875, 0.375).uv(0.8125, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                    }
                                                }
                                                Color.rgba(0.05999999865889549, 0.09000000357627869, 0.07999999821186066, 1.0) {
                                                    name = "__text_color"
                                                }
                                                Transform.position(1.087999939918518, -0.03999999538064003, -0.02500000037252903).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(2.247999906539917, 0.1679999828338623, 1.0) {
                                                    name = "__bg"
                                                    Color.rgba(0.9200000166893005, 0.9700000286102295, 0.9200000166893005, 1.0) {
                                                        Renderable.square() {
                                                            Raycastable.click_only() {
                                                                name = "__bg_raycastable"
                                                            }
                                                            RaycastableShape.quad_2d() {
                                                                name = "__bg_raycastable_shape"
                                                            }
                                                            Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                        }
                                                    }
                                                }
                                            }
                                            Transform.position(0.05199999734759331, -5.327999114990234, 0.05000000074505806).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                                                name = "item_26"
                                                Raycastable.click_only() {
                                                    name = "item_26_raycastable"
                                                }
                                                Style {
                                                    name = "item_26_style"
                                                }
                                                Transform.position(0.0, 0.0, 0.019999999552965164).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                                                    name = "item_26_text_root"
                                                    Text {
                                                        "        gltf"
                                                        name = "item_26_text"
                                                        Color.rgba(0.05999999865889549, 0.09000000357627869, 0.07999999821186066, 1.0) {
                                                            name = "item_26_text_color"
                                                        }
                                                        Transform.position(0.6800000071525574, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.4375, 0.4375).uv(0.5, 0.4375).uv(0.5, 0.375).uv(0.4375, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.7599999904632568, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.75, 0.4375).uv(0.8125, 0.4375).uv(0.8125, 0.375).uv(0.75, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.8399999737739563, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.25, 0.5).uv(0.3125, 0.5).uv(0.3125, 0.4375).uv(0.25, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.9199999570846558, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.375, 0.4375).uv(0.4375, 0.4375).uv(0.4375, 0.375).uv(0.375, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                    }
                                                }
                                                Color.rgba(0.05999999865889549, 0.09000000357627869, 0.07999999821186066, 1.0) {
                                                    name = "__text_color"
                                                }
                                                Transform.position(1.087999939918518, -0.03999999538064003, -0.02500000037252903).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(2.247999906539917, 0.1679999828338623, 1.0) {
                                                    name = "__bg"
                                                    Color.rgba(0.9200000166893005, 0.9700000286102295, 0.9200000166893005, 1.0) {
                                                        Renderable.square() {
                                                            Raycastable.click_only() {
                                                                name = "__bg_raycastable"
                                                            }
                                                            RaycastableShape.quad_2d() {
                                                                name = "__bg_raycastable_shape"
                                                            }
                                                            Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                        }
                                                    }
                                                }
                                            }
                                            Transform.position(0.05199999734759331, -5.535998821258545, 0.05000000074505806).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                                                name = "item_27"
                                                Raycastable.click_only() {
                                                    name = "item_27_raycastable"
                                                }
                                                Style {
                                                    name = "item_27_style"
                                                }
                                                Transform.position(0.0, 0.0, 0.019999999552965164).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                                                    name = "item_27_text_root"
                                                    Text {
                                                        "          emissive"
                                                        name = "item_27_text"
                                                        Color.rgba(0.05999999865889549, 0.09000000357627869, 0.07999999821186066, 1.0) {
                                                            name = "item_27_text_color"
                                                        }
                                                        Transform.position(0.8399999737739563, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.3125, 0.4375).uv(0.375, 0.4375).uv(0.375, 0.375).uv(0.3125, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.9199999570846558, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.8125, 0.4375).uv(0.875, 0.4375).uv(0.875, 0.375).uv(0.8125, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.0, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.5625, 0.4375).uv(0.625, 0.4375).uv(0.625, 0.375).uv(0.5625, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.0799999237060547, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.1875, 0.5).uv(0.25, 0.5).uv(0.25, 0.4375).uv(0.1875, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.159999966621399, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.1875, 0.5).uv(0.25, 0.5).uv(0.25, 0.4375).uv(0.1875, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.2400000095367432, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.5625, 0.4375).uv(0.625, 0.4375).uv(0.625, 0.375).uv(0.5625, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.3199999332427979, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.375, 0.5).uv(0.4375, 0.5).uv(0.4375, 0.4375).uv(0.375, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.399999976158142, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.3125, 0.4375).uv(0.375, 0.4375).uv(0.375, 0.375).uv(0.3125, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                    }
                                                }
                                                Color.rgba(0.05999999865889549, 0.09000000357627869, 0.07999999821186066, 1.0) {
                                                    name = "__text_color"
                                                }
                                                Transform.position(1.087999939918518, -0.03999999538064003, -0.02500000037252903).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(2.247999906539917, 0.1679999828338623, 1.0) {
                                                    name = "__bg"
                                                    Color.rgba(0.9200000166893005, 0.9700000286102295, 0.9200000166893005, 1.0) {
                                                        Renderable.square() {
                                                            Raycastable.click_only() {
                                                                name = "__bg_raycastable"
                                                            }
                                                            RaycastableShape.quad_2d() {
                                                                name = "__bg_raycastable_shape"
                                                            }
                                                            Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                        }
                                                    }
                                                }
                                            }
                                            Transform.position(0.05199999734759331, -5.743999004364014, 0.05000000074505806).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                                                name = "item_28"
                                                Raycastable.click_only() {
                                                    name = "item_28_raycastable"
                                                }
                                                Style {
                                                    name = "item_28_style"
                                                }
                                                Transform.position(0.0, 0.0, 0.019999999552965164).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                                                    name = "item_28_text_root"
                                                    Text {
                                                        "      transform"
                                                        name = "item_28_text"
                                                        Color.rgba(0.05999999865889549, 0.09000000357627869, 0.07999999821186066, 1.0) {
                                                            name = "item_28_text_color"
                                                        }
                                                        Transform.position(0.5199999809265137, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.25, 0.5).uv(0.3125, 0.5).uv(0.3125, 0.4375).uv(0.25, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.5999999642372131, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.125, 0.5).uv(0.1875, 0.5).uv(0.1875, 0.4375).uv(0.125, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.6800000071525574, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.0625, 0.4375).uv(0.125, 0.4375).uv(0.125, 0.375).uv(0.0625, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.7599999904632568, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.875, 0.4375).uv(0.9375, 0.4375).uv(0.9375, 0.375).uv(0.875, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.8399999737739563, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.1875, 0.5).uv(0.25, 0.5).uv(0.25, 0.4375).uv(0.1875, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.9199999570846558, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.375, 0.4375).uv(0.4375, 0.4375).uv(0.4375, 0.375).uv(0.375, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.0, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.9375, 0.4375).uv(1.0, 0.4375).uv(1.0, 0.375).uv(0.9375, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.0799999237060547, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.125, 0.5).uv(0.1875, 0.5).uv(0.1875, 0.4375).uv(0.125, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.159999966621399, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.8125, 0.4375).uv(0.875, 0.4375).uv(0.875, 0.375).uv(0.8125, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                    }
                                                }
                                                Color.rgba(0.05999999865889549, 0.09000000357627869, 0.07999999821186066, 1.0) {
                                                    name = "__text_color"
                                                }
                                                Transform.position(1.087999939918518, -0.03999999538064003, -0.02500000037252903).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(2.247999906539917, 0.1679999828338623, 1.0) {
                                                    name = "__bg"
                                                    Color.rgba(0.9200000166893005, 0.9700000286102295, 0.9200000166893005, 1.0) {
                                                        Renderable.square() {
                                                            Raycastable.click_only() {
                                                                name = "__bg_raycastable"
                                                            }
                                                            RaycastableShape.quad_2d() {
                                                                name = "__bg_raycastable_shape"
                                                            }
                                                            Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                        }
                                                    }
                                                }
                                            }
                                            Transform.position(0.05199999734759331, -5.951998710632324, 0.05000000074505806).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                                                name = "item_29"
                                                Raycastable.click_only() {
                                                    name = "item_29_raycastable"
                                                }
                                                Style {
                                                    name = "item_29_style"
                                                }
                                                Transform.position(0.0, 0.0, 0.019999999552965164).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                                                    name = "item_29_text_root"
                                                    Text {
                                                        "        camera_xr"
                                                        name = "item_29_text"
                                                        Color.rgba(0.05999999865889549, 0.09000000357627869, 0.07999999821186066, 1.0) {
                                                            name = "item_29_text_color"
                                                        }
                                                        Transform.position(0.6800000071525574, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.1875, 0.4375).uv(0.25, 0.4375).uv(0.25, 0.375).uv(0.1875, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.7599999904632568, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.0625, 0.4375).uv(0.125, 0.4375).uv(0.125, 0.375).uv(0.0625, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.8399999737739563, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.8125, 0.4375).uv(0.875, 0.4375).uv(0.875, 0.375).uv(0.8125, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.9199999570846558, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.3125, 0.4375).uv(0.375, 0.4375).uv(0.375, 0.375).uv(0.3125, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.0, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.125, 0.5).uv(0.1875, 0.5).uv(0.1875, 0.4375).uv(0.125, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.0799999237060547, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.0625, 0.4375).uv(0.125, 0.4375).uv(0.125, 0.375).uv(0.0625, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.159999966621399, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.9375, 0.375).uv(1.0, 0.375).uv(1.0, 0.3125).uv(0.9375, 0.3125)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.2400000095367432, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.5, 0.5).uv(0.5625, 0.5).uv(0.5625, 0.4375).uv(0.5, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.3199999332427979, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.125, 0.5).uv(0.1875, 0.5).uv(0.1875, 0.4375).uv(0.125, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                    }
                                                }
                                                Color.rgba(0.05999999865889549, 0.09000000357627869, 0.07999999821186066, 1.0) {
                                                    name = "__text_color"
                                                }
                                                Transform.position(1.087999939918518, -0.03999999538064003, -0.02500000037252903).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(2.247999906539917, 0.1679999828338623, 1.0) {
                                                    name = "__bg"
                                                    Color.rgba(0.9200000166893005, 0.9700000286102295, 0.9200000166893005, 1.0) {
                                                        Renderable.square() {
                                                            Raycastable.click_only() {
                                                                name = "__bg_raycastable"
                                                            }
                                                            RaycastableShape.quad_2d() {
                                                                name = "__bg_raycastable_shape"
                                                            }
                                                            Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                        }
                                                    }
                                                }
                                            }
                                            Transform.position(0.05199999734759331, -6.159998416900635, 0.05000000074505806).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                                                name = "item_30"
                                                Raycastable.click_only() {
                                                    name = "item_30_raycastable"
                                                }
                                                Style {
                                                    name = "item_30_style"
                                                }
                                                Transform.position(0.0, 0.0, 0.019999999552965164).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                                                    name = "item_30_text_root"
                                                    Text {
                                                        "          pointer"
                                                        name = "item_30_text"
                                                        Color.rgba(0.05999999865889549, 0.09000000357627869, 0.07999999821186066, 1.0) {
                                                            name = "item_30_text_color"
                                                        }
                                                        Transform.position(0.8399999737739563, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.0, 0.5).uv(0.0625, 0.5).uv(0.0625, 0.4375).uv(0.0, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.9199999570846558, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.9375, 0.4375).uv(1.0, 0.4375).uv(1.0, 0.375).uv(0.9375, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.0, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.5625, 0.4375).uv(0.625, 0.4375).uv(0.625, 0.375).uv(0.5625, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.0799999237060547, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.875, 0.4375).uv(0.9375, 0.4375).uv(0.9375, 0.375).uv(0.875, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.159999966621399, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.25, 0.5).uv(0.3125, 0.5).uv(0.3125, 0.4375).uv(0.25, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.2400000095367432, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.3125, 0.4375).uv(0.375, 0.4375).uv(0.375, 0.375).uv(0.3125, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.3199999332427979, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.125, 0.5).uv(0.1875, 0.5).uv(0.1875, 0.4375).uv(0.125, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                    }
                                                }
                                                Color.rgba(0.05999999865889549, 0.09000000357627869, 0.07999999821186066, 1.0) {
                                                    name = "__text_color"
                                                }
                                                Transform.position(1.087999939918518, -0.03999999538064003, -0.02500000037252903).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(2.247999906539917, 0.1679999828338623, 1.0) {
                                                    name = "__bg"
                                                    Color.rgba(0.9200000166893005, 0.9700000286102295, 0.9200000166893005, 1.0) {
                                                        Renderable.square() {
                                                            Raycastable.click_only() {
                                                                name = "__bg_raycastable"
                                                            }
                                                            RaycastableShape.quad_2d() {
                                                                name = "__bg_raycastable_shape"
                                                            }
                                                            Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                        }
                                                    }
                                                }
                                            }
                                            Transform.position(0.05199999734759331, -6.3679986000061035, 0.05000000074505806).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                                                name = "item_31"
                                                Raycastable.click_only() {
                                                    name = "item_31_raycastable"
                                                }
                                                Style {
                                                    name = "item_31_style"
                                                }
                                                Transform.position(0.0, 0.0, 0.019999999552965164).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                                                    name = "item_31_text_root"
                                                    Text {
                                                        "      controller_xr"
                                                        name = "item_31_text"
                                                        Color.rgba(0.05999999865889549, 0.09000000357627869, 0.07999999821186066, 1.0) {
                                                            name = "item_31_text_color"
                                                        }
                                                        Transform.position(0.5199999809265137, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.1875, 0.4375).uv(0.25, 0.4375).uv(0.25, 0.375).uv(0.1875, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.5999999642372131, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.9375, 0.4375).uv(1.0, 0.4375).uv(1.0, 0.375).uv(0.9375, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.6800000071525574, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.875, 0.4375).uv(0.9375, 0.4375).uv(0.9375, 0.375).uv(0.875, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.7599999904632568, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.25, 0.5).uv(0.3125, 0.5).uv(0.3125, 0.4375).uv(0.25, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.8399999737739563, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.125, 0.5).uv(0.1875, 0.5).uv(0.1875, 0.4375).uv(0.125, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.9199999570846558, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.9375, 0.4375).uv(1.0, 0.4375).uv(1.0, 0.375).uv(0.9375, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.0, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.75, 0.4375).uv(0.8125, 0.4375).uv(0.8125, 0.375).uv(0.75, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.0799999237060547, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.75, 0.4375).uv(0.8125, 0.4375).uv(0.8125, 0.375).uv(0.75, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.159999966621399, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.3125, 0.4375).uv(0.375, 0.4375).uv(0.375, 0.375).uv(0.3125, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.2400000095367432, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.125, 0.5).uv(0.1875, 0.5).uv(0.1875, 0.4375).uv(0.125, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.3199999332427979, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.9375, 0.375).uv(1.0, 0.375).uv(1.0, 0.3125).uv(0.9375, 0.3125)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.399999976158142, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.5, 0.5).uv(0.5625, 0.5).uv(0.5625, 0.4375).uv(0.5, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.4800000190734863, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.125, 0.5).uv(0.1875, 0.5).uv(0.1875, 0.4375).uv(0.125, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                    }
                                                }
                                                Color.rgba(0.05999999865889549, 0.09000000357627869, 0.07999999821186066, 1.0) {
                                                    name = "__text_color"
                                                }
                                                Transform.position(1.087999939918518, -0.03999999538064003, -0.02500000037252903).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(2.247999906539917, 0.1679999828338623, 1.0) {
                                                    name = "__bg"
                                                    Color.rgba(0.9200000166893005, 0.9700000286102295, 0.9200000166893005, 1.0) {
                                                        Renderable.square() {
                                                            Raycastable.click_only() {
                                                                name = "__bg_raycastable"
                                                            }
                                                            RaycastableShape.quad_2d() {
                                                                name = "__bg_raycastable_shape"
                                                            }
                                                            Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                        }
                                                    }
                                                }
                                            }
                                            Transform.position(0.05199999734759331, -6.575998306274414, 0.05000000074505806).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                                                name = "item_32"
                                                Raycastable.click_only() {
                                                    name = "item_32_raycastable"
                                                }
                                                Style {
                                                    name = "item_32_style"
                                                }
                                                Transform.position(0.0, 0.0, 0.019999999552965164).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                                                    name = "item_32_text_root"
                                                    Text {
                                                        "        transform"
                                                        name = "item_32_text"
                                                        Color.rgba(0.05999999865889549, 0.09000000357627869, 0.07999999821186066, 1.0) {
                                                            name = "item_32_text_color"
                                                        }
                                                        Transform.position(0.6800000071525574, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.25, 0.5).uv(0.3125, 0.5).uv(0.3125, 0.4375).uv(0.25, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.7599999904632568, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.125, 0.5).uv(0.1875, 0.5).uv(0.1875, 0.4375).uv(0.125, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.8399999737739563, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.0625, 0.4375).uv(0.125, 0.4375).uv(0.125, 0.375).uv(0.0625, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.9199999570846558, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.875, 0.4375).uv(0.9375, 0.4375).uv(0.9375, 0.375).uv(0.875, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.0, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.1875, 0.5).uv(0.25, 0.5).uv(0.25, 0.4375).uv(0.1875, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.0799999237060547, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.375, 0.4375).uv(0.4375, 0.4375).uv(0.4375, 0.375).uv(0.375, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.159999966621399, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.9375, 0.4375).uv(1.0, 0.4375).uv(1.0, 0.375).uv(0.9375, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.2400000095367432, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.125, 0.5).uv(0.1875, 0.5).uv(0.1875, 0.4375).uv(0.125, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.3199999332427979, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.8125, 0.4375).uv(0.875, 0.4375).uv(0.875, 0.375).uv(0.8125, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                    }
                                                }
                                                Color.rgba(0.05999999865889549, 0.09000000357627869, 0.07999999821186066, 1.0) {
                                                    name = "__text_color"
                                                }
                                                Transform.position(1.087999939918518, -0.03999999538064003, -0.02500000037252903).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(2.247999906539917, 0.1679999828338623, 1.0) {
                                                    name = "__bg"
                                                    Color.rgba(0.9200000166893005, 0.9700000286102295, 0.9200000166893005, 1.0) {
                                                        Renderable.square() {
                                                            Raycastable.click_only() {
                                                                name = "__bg_raycastable"
                                                            }
                                                            RaycastableShape.quad_2d() {
                                                                name = "__bg_raycastable_shape"
                                                            }
                                                            Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                        }
                                                    }
                                                }
                                            }
                                            Transform.position(0.05199999734759331, -6.783998489379883, 0.05000000074505806).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                                                name = "item_33"
                                                Raycastable.click_only() {
                                                    name = "item_33_raycastable"
                                                }
                                                Style {
                                                    name = "item_33_style"
                                                }
                                                Transform.position(0.0, 0.0, 0.019999999552965164).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                                                    name = "item_33_text_root"
                                                    Text {
                                                        "          pointer"
                                                        name = "item_33_text"
                                                        Color.rgba(0.05999999865889549, 0.09000000357627869, 0.07999999821186066, 1.0) {
                                                            name = "item_33_text_color"
                                                        }
                                                        Transform.position(0.8399999737739563, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.0, 0.5).uv(0.0625, 0.5).uv(0.0625, 0.4375).uv(0.0, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.9199999570846558, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.9375, 0.4375).uv(1.0, 0.4375).uv(1.0, 0.375).uv(0.9375, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.0, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.5625, 0.4375).uv(0.625, 0.4375).uv(0.625, 0.375).uv(0.5625, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.0799999237060547, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.875, 0.4375).uv(0.9375, 0.4375).uv(0.9375, 0.375).uv(0.875, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.159999966621399, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.25, 0.5).uv(0.3125, 0.5).uv(0.3125, 0.4375).uv(0.25, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.2400000095367432, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.3125, 0.4375).uv(0.375, 0.4375).uv(0.375, 0.375).uv(0.3125, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.3199999332427979, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.125, 0.5).uv(0.1875, 0.5).uv(0.1875, 0.4375).uv(0.125, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                    }
                                                }
                                                Color.rgba(0.05999999865889549, 0.09000000357627869, 0.07999999821186066, 1.0) {
                                                    name = "__text_color"
                                                }
                                                Transform.position(1.087999939918518, -0.03999999538064003, -0.02500000037252903).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(2.247999906539917, 0.1679999828338623, 1.0) {
                                                    name = "__bg"
                                                    Color.rgba(0.9200000166893005, 0.9700000286102295, 0.9200000166893005, 1.0) {
                                                        Renderable.square() {
                                                            Raycastable.click_only() {
                                                                name = "__bg_raycastable"
                                                            }
                                                            RaycastableShape.quad_2d() {
                                                                name = "__bg_raycastable_shape"
                                                            }
                                                            Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                        }
                                                    }
                                                }
                                            }
                                            Transform.position(0.05199999734759331, -6.991998195648193, 0.05000000074505806).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                                                name = "item_34"
                                                Raycastable.click_only() {
                                                    name = "item_34_raycastable"
                                                }
                                                Style {
                                                    name = "item_34_style"
                                                }
                                                Transform.position(0.0, 0.0, 0.019999999552965164).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                                                    name = "item_34_text_root"
                                                    Text {
                                                        "      controller_xr"
                                                        name = "item_34_text"
                                                        Color.rgba(0.05999999865889549, 0.09000000357627869, 0.07999999821186066, 1.0) {
                                                            name = "item_34_text_color"
                                                        }
                                                        Transform.position(0.5199999809265137, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.1875, 0.4375).uv(0.25, 0.4375).uv(0.25, 0.375).uv(0.1875, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.5999999642372131, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.9375, 0.4375).uv(1.0, 0.4375).uv(1.0, 0.375).uv(0.9375, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.6800000071525574, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.875, 0.4375).uv(0.9375, 0.4375).uv(0.9375, 0.375).uv(0.875, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.7599999904632568, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.25, 0.5).uv(0.3125, 0.5).uv(0.3125, 0.4375).uv(0.25, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.8399999737739563, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.125, 0.5).uv(0.1875, 0.5).uv(0.1875, 0.4375).uv(0.125, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.9199999570846558, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.9375, 0.4375).uv(1.0, 0.4375).uv(1.0, 0.375).uv(0.9375, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.0, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.75, 0.4375).uv(0.8125, 0.4375).uv(0.8125, 0.375).uv(0.75, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.0799999237060547, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.75, 0.4375).uv(0.8125, 0.4375).uv(0.8125, 0.375).uv(0.75, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.159999966621399, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.3125, 0.4375).uv(0.375, 0.4375).uv(0.375, 0.375).uv(0.3125, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.2400000095367432, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.125, 0.5).uv(0.1875, 0.5).uv(0.1875, 0.4375).uv(0.125, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.3199999332427979, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.9375, 0.375).uv(1.0, 0.375).uv(1.0, 0.3125).uv(0.9375, 0.3125)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.399999976158142, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.5, 0.5).uv(0.5625, 0.5).uv(0.5625, 0.4375).uv(0.5, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.4800000190734863, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.125, 0.5).uv(0.1875, 0.5).uv(0.1875, 0.4375).uv(0.125, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                    }
                                                }
                                                Color.rgba(0.05999999865889549, 0.09000000357627869, 0.07999999821186066, 1.0) {
                                                    name = "__text_color"
                                                }
                                                Transform.position(1.087999939918518, -0.03999999538064003, -0.02500000037252903).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(2.247999906539917, 0.1679999828338623, 1.0) {
                                                    name = "__bg"
                                                    Color.rgba(0.9200000166893005, 0.9700000286102295, 0.9200000166893005, 1.0) {
                                                        Renderable.square() {
                                                            Raycastable.click_only() {
                                                                name = "__bg_raycastable"
                                                            }
                                                            RaycastableShape.quad_2d() {
                                                                name = "__bg_raycastable_shape"
                                                            }
                                                            Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                        }
                                                    }
                                                }
                                            }
                                            Transform.position(0.05199999734759331, -7.199997901916504, 0.05000000074505806).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                                                name = "item_35"
                                                Raycastable.click_only() {
                                                    name = "item_35_raycastable"
                                                }
                                                Style {
                                                    name = "item_35_style"
                                                }
                                                Transform.position(0.0, 0.0, 0.019999999552965164).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                                                    name = "item_35_text_root"
                                                    Text {
                                                        "        transform"
                                                        name = "item_35_text"
                                                        Color.rgba(0.05999999865889549, 0.09000000357627869, 0.07999999821186066, 1.0) {
                                                            name = "item_35_text_color"
                                                        }
                                                        Transform.position(0.6800000071525574, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.25, 0.5).uv(0.3125, 0.5).uv(0.3125, 0.4375).uv(0.25, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.7599999904632568, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.125, 0.5).uv(0.1875, 0.5).uv(0.1875, 0.4375).uv(0.125, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.8399999737739563, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.0625, 0.4375).uv(0.125, 0.4375).uv(0.125, 0.375).uv(0.0625, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.9199999570846558, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.875, 0.4375).uv(0.9375, 0.4375).uv(0.9375, 0.375).uv(0.875, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.0, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.1875, 0.5).uv(0.25, 0.5).uv(0.25, 0.4375).uv(0.1875, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.0799999237060547, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.375, 0.4375).uv(0.4375, 0.4375).uv(0.4375, 0.375).uv(0.375, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.159999966621399, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.9375, 0.4375).uv(1.0, 0.4375).uv(1.0, 0.375).uv(0.9375, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.2400000095367432, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.125, 0.5).uv(0.1875, 0.5).uv(0.1875, 0.4375).uv(0.125, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.3199999332427979, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.8125, 0.4375).uv(0.875, 0.4375).uv(0.875, 0.375).uv(0.8125, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                    }
                                                }
                                                Color.rgba(0.05999999865889549, 0.09000000357627869, 0.07999999821186066, 1.0) {
                                                    name = "__text_color"
                                                }
                                                Transform.position(1.087999939918518, -0.03999999538064003, -0.02500000037252903).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(2.247999906539917, 0.1679999828338623, 1.0) {
                                                    name = "__bg"
                                                    Color.rgba(0.9200000166893005, 0.9700000286102295, 0.9200000166893005, 1.0) {
                                                        Renderable.square() {
                                                            Raycastable.click_only() {
                                                                name = "__bg_raycastable"
                                                            }
                                                            RaycastableShape.quad_2d() {
                                                                name = "__bg_raycastable_shape"
                                                            }
                                                            Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                        }
                                                    }
                                                }
                                            }
                                            Transform.position(0.05199999734759331, -7.407998085021973, 0.05000000074505806).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                                                name = "item_36"
                                                Raycastable.click_only() {
                                                    name = "item_36_raycastable"
                                                }
                                                Style {
                                                    name = "item_36_style"
                                                }
                                                Transform.position(0.0, 0.0, 0.019999999552965164).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                                                    name = "item_36_text_root"
                                                    Text {
                                                        "          pointer"
                                                        name = "item_36_text"
                                                        Color.rgba(0.05999999865889549, 0.09000000357627869, 0.07999999821186066, 1.0) {
                                                            name = "item_36_text_color"
                                                        }
                                                        Transform.position(0.8399999737739563, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.0, 0.5).uv(0.0625, 0.5).uv(0.0625, 0.4375).uv(0.0, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.9199999570846558, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.9375, 0.4375).uv(1.0, 0.4375).uv(1.0, 0.375).uv(0.9375, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.0, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.5625, 0.4375).uv(0.625, 0.4375).uv(0.625, 0.375).uv(0.5625, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.0799999237060547, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.875, 0.4375).uv(0.9375, 0.4375).uv(0.9375, 0.375).uv(0.875, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.159999966621399, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.25, 0.5).uv(0.3125, 0.5).uv(0.3125, 0.4375).uv(0.25, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.2400000095367432, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.3125, 0.4375).uv(0.375, 0.4375).uv(0.375, 0.375).uv(0.3125, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.3199999332427979, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.125, 0.5).uv(0.1875, 0.5).uv(0.1875, 0.4375).uv(0.125, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                    }
                                                }
                                                Color.rgba(0.05999999865889549, 0.09000000357627869, 0.07999999821186066, 1.0) {
                                                    name = "__text_color"
                                                }
                                                Transform.position(1.087999939918518, -0.03999999538064003, -0.02500000037252903).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(2.247999906539917, 0.1679999828338623, 1.0) {
                                                    name = "__bg"
                                                    Color.rgba(0.9200000166893005, 0.9700000286102295, 0.9200000166893005, 1.0) {
                                                        Renderable.square() {
                                                            Raycastable.click_only() {
                                                                name = "__bg_raycastable"
                                                            }
                                                            RaycastableShape.quad_2d() {
                                                                name = "__bg_raycastable_shape"
                                                            }
                                                            Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                        }
                                                    }
                                                }
                                            }
                                            Transform.position(0.05199999734759331, -7.615997791290283, 0.05000000074505806).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                                                name = "item_37"
                                                Raycastable.click_only() {
                                                    name = "item_37_raycastable"
                                                }
                                                Style {
                                                    name = "item_37_style"
                                                }
                                                Transform.position(0.0, 0.0, 0.019999999552965164).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                                                    name = "item_37_text_root"
                                                    Text {
                                                        "    overlay"
                                                        name = "item_37_text"
                                                        Color.rgba(0.05999999865889549, 0.09000000357627869, 0.07999999821186066, 1.0) {
                                                            name = "item_37_text_color"
                                                        }
                                                        Transform.position(0.35999998450279236, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.9375, 0.4375).uv(1.0, 0.4375).uv(1.0, 0.375).uv(0.9375, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.4399999976158142, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.375, 0.5).uv(0.4375, 0.5).uv(0.4375, 0.4375).uv(0.375, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.5199999809265137, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.3125, 0.4375).uv(0.375, 0.4375).uv(0.375, 0.375).uv(0.3125, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.5999999642372131, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.125, 0.5).uv(0.1875, 0.5).uv(0.1875, 0.4375).uv(0.125, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.6800000071525574, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.75, 0.4375).uv(0.8125, 0.4375).uv(0.8125, 0.375).uv(0.75, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.7599999904632568, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.0625, 0.4375).uv(0.125, 0.4375).uv(0.125, 0.375).uv(0.0625, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.8399999737739563, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.5625, 0.5).uv(0.625, 0.5).uv(0.625, 0.4375).uv(0.5625, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                    }
                                                }
                                                Color.rgba(0.05999999865889549, 0.09000000357627869, 0.07999999821186066, 1.0) {
                                                    name = "__text_color"
                                                }
                                                Transform.position(1.087999939918518, -0.03999999538064003, -0.02500000037252903).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(2.247999906539917, 0.1679999828338623, 1.0) {
                                                    name = "__bg"
                                                    Color.rgba(0.9200000166893005, 0.9700000286102295, 0.9200000166893005, 1.0) {
                                                        Renderable.square() {
                                                            Raycastable.click_only() {
                                                                name = "__bg_raycastable"
                                                            }
                                                            RaycastableShape.quad_2d() {
                                                                name = "__bg_raycastable_shape"
                                                            }
                                                            Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                        }
                                                    }
                                                }
                                            }
                                            Transform.position(0.05199999734759331, -7.823997497558594, 0.05000000074505806).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                                                name = "item_38"
                                                Raycastable.click_only() {
                                                    name = "item_38_raycastable"
                                                }
                                                Style {
                                                    name = "item_38_style"
                                                }
                                                Transform.position(0.0, 0.0, 0.019999999552965164).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                                                    name = "item_38_text_root"
                                                    Text {
                                                        "      transform"
                                                        name = "item_38_text"
                                                        Color.rgba(0.05999999865889549, 0.09000000357627869, 0.07999999821186066, 1.0) {
                                                            name = "item_38_text_color"
                                                        }
                                                        Transform.position(0.5199999809265137, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.25, 0.5).uv(0.3125, 0.5).uv(0.3125, 0.4375).uv(0.25, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.5999999642372131, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.125, 0.5).uv(0.1875, 0.5).uv(0.1875, 0.4375).uv(0.125, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.6800000071525574, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.0625, 0.4375).uv(0.125, 0.4375).uv(0.125, 0.375).uv(0.0625, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.7599999904632568, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.875, 0.4375).uv(0.9375, 0.4375).uv(0.9375, 0.375).uv(0.875, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.8399999737739563, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.1875, 0.5).uv(0.25, 0.5).uv(0.25, 0.4375).uv(0.1875, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.9199999570846558, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.375, 0.4375).uv(0.4375, 0.4375).uv(0.4375, 0.375).uv(0.375, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.0, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.9375, 0.4375).uv(1.0, 0.4375).uv(1.0, 0.375).uv(0.9375, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.0799999237060547, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.125, 0.5).uv(0.1875, 0.5).uv(0.1875, 0.4375).uv(0.125, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.159999966621399, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.8125, 0.4375).uv(0.875, 0.4375).uv(0.875, 0.375).uv(0.8125, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                    }
                                                }
                                                Color.rgba(0.05999999865889549, 0.09000000357627869, 0.07999999821186066, 1.0) {
                                                    name = "__text_color"
                                                }
                                                Transform.position(1.087999939918518, -0.03999999538064003, -0.02500000037252903).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(2.247999906539917, 0.1679999828338623, 1.0) {
                                                    name = "__bg"
                                                    Color.rgba(0.9200000166893005, 0.9700000286102295, 0.9200000166893005, 1.0) {
                                                        Renderable.square() {
                                                            Raycastable.click_only() {
                                                                name = "__bg_raycastable"
                                                            }
                                                            RaycastableShape.quad_2d() {
                                                                name = "__bg_raycastable_shape"
                                                            }
                                                            Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                        }
                                                    }
                                                }
                                            }
                                            Transform.position(0.05199999734759331, -8.031997680664063, 0.05000000074505806).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                                                name = "item_39"
                                                Raycastable.click_only() {
                                                    name = "item_39_raycastable"
                                                }
                                                Style {
                                                    name = "item_39_style"
                                                }
                                                Transform.position(0.0, 0.0, 0.019999999552965164).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                                                    name = "item_39_text_root"
                                                    Text {
                                                        "        renderable"
                                                        name = "item_39_text"
                                                        Color.rgba(0.05999999865889549, 0.09000000357627869, 0.07999999821186066, 1.0) {
                                                            name = "item_39_text_color"
                                                        }
                                                        Transform.position(0.6800000071525574, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.125, 0.5).uv(0.1875, 0.5).uv(0.1875, 0.4375).uv(0.125, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.7599999904632568, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.3125, 0.4375).uv(0.375, 0.4375).uv(0.375, 0.375).uv(0.3125, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.8399999737739563, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.875, 0.4375).uv(0.9375, 0.4375).uv(0.9375, 0.375).uv(0.875, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.9199999570846558, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.25, 0.4375).uv(0.3125, 0.4375).uv(0.3125, 0.375).uv(0.25, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.0, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.3125, 0.4375).uv(0.375, 0.4375).uv(0.375, 0.375).uv(0.3125, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.0799999237060547, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.125, 0.5).uv(0.1875, 0.5).uv(0.1875, 0.4375).uv(0.125, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.159999966621399, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.0625, 0.4375).uv(0.125, 0.4375).uv(0.125, 0.375).uv(0.0625, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.2400000095367432, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.125, 0.4375).uv(0.1875, 0.4375).uv(0.1875, 0.375).uv(0.125, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.3199999332427979, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.75, 0.4375).uv(0.8125, 0.4375).uv(0.8125, 0.375).uv(0.75, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.399999976158142, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.3125, 0.4375).uv(0.375, 0.4375).uv(0.375, 0.375).uv(0.3125, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                    }
                                                }
                                                Color.rgba(0.05999999865889549, 0.09000000357627869, 0.07999999821186066, 1.0) {
                                                    name = "__text_color"
                                                }
                                                Transform.position(1.087999939918518, -0.03999999538064003, -0.02500000037252903).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(2.247999906539917, 0.1679999828338623, 1.0) {
                                                    name = "__bg"
                                                    Color.rgba(0.9200000166893005, 0.9700000286102295, 0.9200000166893005, 1.0) {
                                                        Renderable.square() {
                                                            Raycastable.click_only() {
                                                                name = "__bg_raycastable"
                                                            }
                                                            RaycastableShape.quad_2d() {
                                                                name = "__bg_raycastable_shape"
                                                            }
                                                            Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                        }
                                                    }
                                                }
                                            }
                                            Transform.position(0.05199999734759331, -8.239996910095215, 0.05000000074505806).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                                                name = "item_40"
                                                Raycastable.click_only() {
                                                    name = "item_40_raycastable"
                                                }
                                                Style {
                                                    name = "item_40_style"
                                                }
                                                Transform.position(0.0, 0.0, 0.019999999552965164).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                                                    name = "item_40_text_root"
                                                    Text {
                                                        "          color"
                                                        name = "item_40_text"
                                                        Color.rgba(0.05999999865889549, 0.09000000357627869, 0.07999999821186066, 1.0) {
                                                            name = "item_40_text_color"
                                                        }
                                                        Transform.position(0.8399999737739563, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.1875, 0.4375).uv(0.25, 0.4375).uv(0.25, 0.375).uv(0.1875, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.9199999570846558, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.9375, 0.4375).uv(1.0, 0.4375).uv(1.0, 0.375).uv(0.9375, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.0, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.75, 0.4375).uv(0.8125, 0.4375).uv(0.8125, 0.375).uv(0.75, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.0799999237060547, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.9375, 0.4375).uv(1.0, 0.4375).uv(1.0, 0.375).uv(0.9375, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.159999966621399, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.125, 0.5).uv(0.1875, 0.5).uv(0.1875, 0.4375).uv(0.125, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                    }
                                                }
                                                Color.rgba(0.05999999865889549, 0.09000000357627869, 0.07999999821186066, 1.0) {
                                                    name = "__text_color"
                                                }
                                                Transform.position(1.087999939918518, -0.03999999538064003, -0.02500000037252903).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(2.247999906539917, 0.1679999828338623, 1.0) {
                                                    name = "__bg"
                                                    Color.rgba(0.9200000166893005, 0.9700000286102295, 0.9200000166893005, 1.0) {
                                                        Renderable.square() {
                                                            Raycastable.click_only() {
                                                                name = "__bg_raycastable"
                                                            }
                                                            RaycastableShape.quad_2d() {
                                                                name = "__bg_raycastable_shape"
                                                            }
                                                            Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                        }
                                                    }
                                                }
                                            }
                                            Transform.position(0.05199999734759331, -8.447997093200684, 0.05000000074505806).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                                                name = "item_41"
                                                Raycastable.click_only() {
                                                    name = "item_41_raycastable"
                                                }
                                                Style {
                                                    name = "item_41_style"
                                                }
                                                Transform.position(0.0, 0.0, 0.019999999552965164).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                                                    name = "item_41_text_root"
                                                    Text {
                                                        "          emissive"
                                                        name = "item_41_text"
                                                        Color.rgba(0.05999999865889549, 0.09000000357627869, 0.07999999821186066, 1.0) {
                                                            name = "item_41_text_color"
                                                        }
                                                        Transform.position(0.8399999737739563, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.3125, 0.4375).uv(0.375, 0.4375).uv(0.375, 0.375).uv(0.3125, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(0.9199999570846558, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.8125, 0.4375).uv(0.875, 0.4375).uv(0.875, 0.375).uv(0.8125, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.0, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.5625, 0.4375).uv(0.625, 0.4375).uv(0.625, 0.375).uv(0.5625, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.0799999237060547, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.1875, 0.5).uv(0.25, 0.5).uv(0.25, 0.4375).uv(0.1875, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.159999966621399, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.1875, 0.5).uv(0.25, 0.5).uv(0.25, 0.4375).uv(0.1875, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.2400000095367432, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.5625, 0.4375).uv(0.625, 0.4375).uv(0.625, 0.375).uv(0.5625, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.3199999332427979, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.375, 0.5).uv(0.4375, 0.5).uv(0.4375, 0.4375).uv(0.375, 0.4375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                        Transform.position(1.399999976158142, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                                            Renderable.square() {
                                                                UV.uv(0.3125, 0.4375).uv(0.375, 0.4375).uv(0.375, 0.375).uv(0.3125, 0.375)
                                                                Texture.from_dds("assets/textures/font_system.dds")
                                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                            }
                                                        }
                                                    }
                                                }
                                                Color.rgba(0.05999999865889549, 0.09000000357627869, 0.07999999821186066, 1.0) {
                                                    name = "__text_color"
                                                }
                                                Transform.position(1.087999939918518, -0.03999999538064003, -0.02500000037252903).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(2.247999906539917, 0.1679999828338623, 1.0) {
                                                    name = "__bg"
                                                    Color.rgba(0.9200000166893005, 0.9700000286102295, 0.9200000166893005, 1.0) {
                                                        Renderable.square() {
                                                            Raycastable.click_only() {
                                                                name = "__bg_raycastable"
                                                            }
                                                            RaycastableShape.quad_2d() {
                                                                name = "__bg_raycastable_shape"
                                                            }
                                                            Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                                Router.target("__scroll_track") {
                                    name = "__scroll_router"
                                }
                            }
                            Router.target("__scroll") {
                                name = "__layout_overflow_router"
                            }
                        }
                    }
                }
                Transform.position(2.359999895095825, -0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                    name = "editor_inspector_panel_shell"
                    Style {}
                    Transform.position(0.03999999910593033, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                        name = "inspector_panel_root"
                        Style {}
                        Transform.position(0.0, -0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                            name = "title_bar"
                            Style {}
                            Transform.position(0.019999999552965164, -0.0, 0.05000000074505806).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                                name = "title_label_wrap"
                                Style {}
                                Transform.position(0.0, 0.0, 0.014999999664723873).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                                    Text {
                                        "Inspector"
                                        name = "title_label"
                                        Transform.position(0.03999999910593033, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                            Renderable.square() {
                                                UV.uv(0.5625, 0.3125).uv(0.625, 0.3125).uv(0.625, 0.25).uv(0.5625, 0.25)
                                                Texture.from_dds("assets/textures/font_system.dds")
                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                            }
                                        }
                                        Transform.position(0.11999999731779099, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                            Renderable.square() {
                                                UV.uv(0.875, 0.4375).uv(0.9375, 0.4375).uv(0.9375, 0.375).uv(0.875, 0.375)
                                                Texture.from_dds("assets/textures/font_system.dds")
                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                            }
                                        }
                                        Transform.position(0.19999998807907104, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                            Renderable.square() {
                                                UV.uv(0.1875, 0.5).uv(0.25, 0.5).uv(0.25, 0.4375).uv(0.1875, 0.4375)
                                                Texture.from_dds("assets/textures/font_system.dds")
                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                            }
                                        }
                                        Transform.position(0.2800000011920929, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                            Renderable.square() {
                                                UV.uv(0.0, 0.5).uv(0.0625, 0.5).uv(0.0625, 0.4375).uv(0.0, 0.4375)
                                                Texture.from_dds("assets/textures/font_system.dds")
                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                            }
                                        }
                                        Transform.position(0.35999998450279236, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                            Renderable.square() {
                                                UV.uv(0.3125, 0.4375).uv(0.375, 0.4375).uv(0.375, 0.375).uv(0.3125, 0.375)
                                                Texture.from_dds("assets/textures/font_system.dds")
                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                            }
                                        }
                                        Transform.position(0.4399999976158142, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                            Renderable.square() {
                                                UV.uv(0.1875, 0.4375).uv(0.25, 0.4375).uv(0.25, 0.375).uv(0.1875, 0.375)
                                                Texture.from_dds("assets/textures/font_system.dds")
                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                            }
                                        }
                                        Transform.position(0.5199999809265137, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                            Renderable.square() {
                                                UV.uv(0.25, 0.5).uv(0.3125, 0.5).uv(0.3125, 0.4375).uv(0.25, 0.4375)
                                                Texture.from_dds("assets/textures/font_system.dds")
                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                            }
                                        }
                                        Transform.position(0.5999999642372131, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                            Renderable.square() {
                                                UV.uv(0.9375, 0.4375).uv(1.0, 0.4375).uv(1.0, 0.375).uv(0.9375, 0.375)
                                                Texture.from_dds("assets/textures/font_system.dds")
                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                            }
                                        }
                                        Transform.position(0.6800000071525574, -0.03999999910593033, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.07999999821186066, 0.07999999821186066, 1.0) {
                                            Renderable.square() {
                                                UV.uv(0.125, 0.5).uv(0.1875, 0.5).uv(0.1875, 0.4375).uv(0.125, 0.4375)
                                                Texture.from_dds("assets/textures/font_system.dds")
                                                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                            }
                                        }
                                    }
                                }
                                Color.rgba(0.8999999761581421, 1.0, 0.9200000166893005, 1.0) {
                                    name = "__text_color"
                                }
                            }
                            Transform.position(0.8399999737739563, -0.11999999731779099, -0.02500000037252903).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.6799999475479126, 0.23999999463558197, 1.0) {
                                name = "__bg"
                                Color.rgba(0.18000000715255737, 0.7799999713897705, 0.2199999988079071, 0.949999988079071) {
                                    Renderable.square() {
                                        Opacity.opacity(0.949999988079071)
                                        Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                    }
                                }
                            }
                        }
                        Transform.position(0.0, -0.23999999463558197, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                            name = "content_slot"
                            Style {}
                            Transform.position(0.8399999737739563, -2.1599998474121094, -0.02500000037252903).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.6799999475479126, 4.319999694824219, 1.0) {
                                name = "__bg"
                                Color.rgba(0.9599999785423279, 0.9200000166893005, 0.18000000715255737, 0.800000011920929) {
                                    Renderable.square() {
                                        Opacity.opacity(0.800000011920929)
                                        Raycastable.drag_only() {
                                            name = "__scroll_drag_raycastable"
                                        }
                                        RaycastableShape.quad_2d() {
                                            name = "__scroll_drag_shape"
                                        }
                                        Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
                                    }
                                }
                            }
                            StencilClip {
                                name = "__layout_stencil_clip"
                            }
                            Scrolling.new(54.0, 0.0) {
                                name = "__scroll"
                                Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                                    name = "__scroll_track"
                                    Transform.position(0.0, -0.0, 0.05000000074505806).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                                        name = "inspector_panel_content_root"
                                        Style {}
                                        Transform.position(0.0, -0.0, 0.05000000074505806).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                                            name = "rows_mount"
                                            Style {}
                                        }
                                    }
                                }
                                Router.target("__scroll_track") {
                                    name = "__scroll_router"
                                }
                            }
                            Router.target("__scroll") {
                                name = "__layout_overflow_router"
                            }
                        }
                    }
                }
            }
        }
    }
}

