import { bisket_anime_shading } from "../materials/bisket_anime_shading.mms"
import { mitten_model } from "mitten.mms"
import { vroid_arm_ik_bend_plane_prototype } from "../arm_ik/vroid.mms"
import { ambient_eye_saccades } from "../animations/ambient_eye_saccades.mms"
import { looping_blink } from "../animations/looping_blink.mms"
import { vroid_humanoid_bone_map } from "../humanoid_bone_maps/vroid.mms"
import { lumae_colliders } from "../characters/lumae/colliders.mms"
import { lumae_secondary_motion } from "../characters/lumae/secondary_motion.mms"
import { pose as lumae_relaxed_pose } from "../characters/lumae/poses/000-relaxed.pose.mms"
import { pose as bisket_relaxed_pose } from "../poses/bisket/000-relaxed.pose.mms"
import { pose as mitten_seated_pose } from "../poses/mitten/floor-seated.pose.mms"
import { miku_twintails, miku_hand_colliders, prepare_miku_arms, install_miku_equipment } from "./tda_miku_rig.mms"
import { detach_avatar_equipment, install_vroid_equipment } from "./avatar_equipment.mms"
import { miku_humanoid_bone_map } from "../humanoid_bone_maps/tda_miku.mms"
import { health_bar } from "../health_bar.mms"
import { hud } from "../hud.mms"
import { player_hitbox } from "./player_hitbox.mms"

// The TDA export is in MMD units (~8 cm), about 20 m tall as meters. Its
// GLTF bakes a uniform scale to this height on import, so the rig is meter
// scale like the VRoid avatars; GLB-unit constants multiply by import_scale().
let MIKU_HEIGHT = 1.7
// Eye height in GLB units, measured under the former 0.09 wrapper (1.59 m).
let MIKU_EYE_GLB = 1.59 / 0.09
// Before the GLB loads (a player starting as Miku) assume that old scale.
let MIKU_ASSUMED_SCALE = 0.09
// Below this height the body stays this tall and the whole rig scales down
// instead, shrinking eye spacing, head motion and hands with it. The scale is
// 1 at the boundary, so heights move smoothly into tiny mode.
let TINY_BODY_METERS = 0.5

fn vroid_face() {
    return MorphTargetMap.new()
        .slot("left_eye_blink", "Fcl_EYE_Close_L")
        .slot("right_eye_blink", "Fcl_EYE_Close_R")
        .slot("viseme_aa", "Fcl_MTH_A")
}

let model_uris = {
    lumae = "assets/components/characters/lumae/lumae.glb"
    miku = "assets/models/local/third_party/TDA_Hatsune_Miku/tda_miku_with_bones.glb"
    succubus = "assets/models/local/succubus.glb"
}

// pose (Mitten only): "relaxed" starts untracked arms in the relaxed rest
// pose; "seated" starts the whole body in the floor-seated pose, a stand-in
// for sitting in a vehicle seat until a chair pose is captured.
fn make_mitten(model_name, track_hands, bucket, target_meters, pose) {
    let children = null
    if pose == "relaxed" { children = [bisket_relaxed_pose()] }
    if pose == "seated" { children = [mitten_seated_pose()] }
    return mitten_model(model_name, { xr = track_hands tail_wag = "canine" material = bisket_anime_shading(bucket)
        target_height = target_meters children = children })
}

fn make_lumae(model_name, track_hands, bucket, target_meters, relaxed) {
    let model = GLTF.new(model_uris.lumae) {
        if target_meters != null { target_height(target_meters) }
        name = model_name
        if track_hands { HandTracking.new().source("openxr") }
        bisket_anime_shading(bucket)
        vroid_humanoid_bone_map()
        vroid_face()
        lumae_relaxed_pose()
        lumae_colliders()
        lumae_secondary_motion()
    }
    install_vroid_equipment(model)
    return { model = model eye_height = 1.644 sole_offset = [0.0, 0.0687271, 0.1129329] }
}

fn make_miku(model_name, track_hands, bucket, target_meters, relaxed) {
    let model = GLTF.new(model_uris.miku) {
        if target_meters != null { target_height(target_meters) } else { default_height_if_outside(1.0, 3.0, MIKU_HEIGHT) }
        name = model_name
        // The finger chains are mapped in tda_miku.mms; the curl axes are not
        // yet checked against this rig (see miku_rig_report.rs).
        if track_hands { HandTracking.new().source("openxr") }
        bisket_anime_shading(bucket)
        miku_humanoid_bone_map()
        miku_twintails()
        miku_hand_colliders()
    }
    install_miku_equipment(model)
    return { model = model eye_height = MIKU_EYE_GLB * MIKU_ASSUMED_SCALE import_scale = MIKU_ASSUMED_SCALE }
}

fn make_succubus(model_name, track_hands, bucket, target_meters, relaxed) {
    let model = GLTF.new(model_uris.succubus) {
        if target_meters != null { target_height(target_meters) }
        name = model_name
        if track_hands { HandTracking.new().source("openxr") }
        bisket_anime_shading(bucket)
        vroid_humanoid_bone_map()
        vroid_face()
    }
    install_vroid_equipment(model)
    return { model = model eye_height = 1.659 sole_offset = [0.0, 0.055684, 0.091826] }
}

let factories = { mitten = make_mitten lumae = make_lumae miku = make_miku succubus = make_succubus }

export fn make_rooftop_player(options) {
    let prefix = options.prefix
    let local_player = options.local_player
    let use_xr = options.use_xr
    let use_desktop = options.use_desktop
    let camera_far = options["camera_far"]
    let voice_level = options["voice_level"]
    let mouth_tuning = options["mouth_tuning"]
    let track_hands = local_player && use_xr
    // A non-local player that walks under its own gravity and slides against
    // the world (an NPC), instead of following a peer's replicated root.
    let physical = !local_player && options["physics"] == true
    // Mitten bodies start in the relaxed rest pose (arms down), or seated
    // (legs then keep the pose: no crouch or walking IK), when set.
    let seated = options["seated_pose"] == true
    let relaxed_pose = null
    if options["relaxed_pose"] == true { relaxed_pose = "relaxed" }
    if seated { relaxed_pose = "seated" }
    let view = { third_person = local_player && options["third_person"] == true }
    let initial_factory = factories[options.avatar]
    // Optional opaque depth bucket for every body this player wears.
    let bucket = options["opaque_bucket"]
    let initial = initial_factory(prefix + "_model", track_hands, bucket, null, relaxed_pose)
    let left = T.position(-0.35, -0.35, -0.25) {
        name = prefix + "_left_target"
        if track_hands {
            Pointer { name = "local_left_pointer" click_max_ray_angle_deg(12.0) click_max_origin_distance(0.16) }
        }
    }
    let right = T.position(0.35, -0.35, -0.25) {
        name = prefix + "_right_target"
        if track_hands {
            Pointer { name = "local_right_pointer" click_max_ray_angle_deg(12.0) click_max_origin_distance(0.16) }
        }
    }
    let external_hands = !local_player || !use_xr
    let gait = options["walking_ik"]
    let left_driver = XRHand.new(true, "Left", "GripAim").external(external_hands) {
        if track_hands { laser() }
        left
    }
    let right_driver = XRHand.new(true, "Right", "GripAim").external(external_hands) {
        if track_hands { laser() }
        right
    }
    let root_name = prefix + "_root"
    fn build_body(spec, choice) {
        let avatar = AVC.movement_target("[name='" + root_name + "']") {
            name = prefix + "_avatar"
            // Desktop owns an authored clip; XR closure comes only from tracking.
            idle_blink(false)
            // Per-model sole landmarks with paper-craft's local gait.
            // Match camera-bearing and camera-free rigs' eye-to-head calibration.
            eye_offset([0.0, 0.08, 0.12])
            crouch_enabled(!seated)
            walking_enabled(!seated)
            // Get low enough and crouching gives way to hands and knees.
            posture("auto")
            third_person(view.third_person)
            // Torso spheres keep the pitched body colliding on hands and knees.
            collision_detail("detailed")
            if choice == "miku" {
                // Measured at the boot soles in the weighted ankle frames, in GLB
                // units, so scaled by the baked import scale.
                crouch_sole_offsets(
                    [-0.0065 * spec.import_scale, -1.599 * spec.import_scale, 0.311 * spec.import_scale],
                    [0.0065 * spec.import_scale, -1.599 * spec.import_scale, 0.311 * spec.import_scale])
                crouch_support_margin(0.25)
                arm_ik({ left = { pole_direction = [1.0, -1.5, 0.6] } right = { pole_direction = [-1.0, -1.5, 0.6] } })
                collision_disabled()
                avatar_height(spec.eye_height)
            } else {
                crouch_sole_offsets(spec.sole_offset, spec.sole_offset)
                arm_ik(vroid_arm_ik_bend_plane_prototype())
            }
            WalkingIK {
                if gait { stride(gait.stride) step_scale(gait.step_scale) step_height(gait.step_height) }
                else { step_scale(1.3) }
            }
            if !local_player && !physical { collision_response("read_only") }
            if local_player && choice != "miku" {
                mouth_open_from_amplitude(voice_level)
                mouth_open_rms_center_range(mouth_tuning.center_rms, mouth_tuning.range_rms)
                mouth_open_amount(mouth_tuning.amount)
                mouth_open_smoothing(16.0)
            }
            initial_yaw(3.14159)
            hand_rotation_smoothing(220.0)
        }
        return avatar
    }

    let camera = T.position(0.0, 0.08, 0.12) {
        if local_player && use_desktop {
            name = "local_desktop_camera_anchor"
            C3D {
                fov(65.0) near(0.01)
                if camera_far != null { far(camera_far) }
                Pointer {
                    name = "local_desktop_pointer"
                    Raycast.event_driven().min_distance(0.75) {}
                }
            }
        }
        if local_player && use_xr {
            name = "local_xr_camera_anchor"
            CXR { if camera_far != null { far(camera_far) } }
        }
    }
    let eye_tracking = XREyeTracking.on().priority(["htc", "vrchat_osc"]).enable_pupil_direction_tracking(false)
    fn mount_body(avatar, spec, choice) {
        let model_frame = T {
            // The wearer can aim through their own hands and body.
            if track_hands { Raycastable.disabled() Selectable.off() }
        }
        let model_mount = spec.model
        if spec["staging"] {
            model_mount = spec.staging
            model_mount.update_transform([0, 0, 0])
        }
        // The imported scene already includes its axis conversion, as in the cafeteria.
        model_frame.attach(model_mount)
        avatar.attach(model_frame)
        avatar.attach(left_driver)
        avatar.attach(right_driver)
        if local_player {
            // The desktop camera faces back along Miku's converted axes.
            let yaw = 0.0
            if choice == "miku" && use_desktop && !use_xr { yaw = Math.pi }
            camera.update_transform([0.0, 0.08, 0.12], [0.0, yaw, 0.0])
            avatar.attach(camera)
            if use_xr { avatar.attach(eye_tracking) }
        }
    }

    fn animate_face(model, avatar) {
        let left_eye = model.query("[name='J_Adj_L_FaceEye']")
        let right_eye = model.query("[name='J_Adj_R_FaceEye']")
        if left_eye && right_eye { avatar.attach(ambient_eye_saccades(left_eye, right_eye, 2.0)) }
        if local_player && !use_xr { avatar.attach(looping_blink(model, 2.0)) }
    }

    let avatar = build_body(initial, options.avatar)
    mount_body(avatar, initial, options.avatar)
    on(initial.model, "GLTFInitialized", fn(event) { animate_face(event.gltf, avatar) })
    let head = T.position(0.0, initial.eye_height, 0.0) { name = prefix + "_head_target" avatar }
    let shield_mount = null
    let shield_frame = null
    if options["shield_mount"] == true {
        shield_mount = T.position(0.0, -0.62, 0.0) { name = prefix + "_shield_mount" }
        // Upright, but at the rig's scale so a tiny player's shield fits them.
        shield_frame = TOP.fork_trs().drop("rotation") { shield_mount }
        head.attach(shield_frame)
    }
    let label = Text { prefix C.rgba(0.12, 0.14, 0.20, 1.0) }
    let health = null
    let health_view = null
    let head_display = null
    let hit_zone = null
    let hitbox = null
    if options["show_health"] == true {
        health = health_bar(Text { "HEALTH" font_size(0.65) C.rgba(1.0, 1.0, 1.0, 1.0) Unlit {} })
        if local_player {
            health_view = hud(prefix + "_health_hud", [0.0, 0.34, -1.10], T.scale(0.043, 0.043, 0.043) { health })
            // XR renders the tracked target pose. Desktop uses the mapped camera.
            if use_xr { head_display = health_view } else { camera.attach(health_view) }
        } else {
            let viewer = options["health_view"]
            let yaw = Math.pi
            if viewer { yaw = 0.0 }
            // Take position from the peer's head and yaw from the local view.
            let overhead = TOP.fork_trs().sample_ancestor("translation", 0).extract_yaw().drop("scale") {
                T.position(0.0, 0.37, 0.0).rotation(0.0, yaw, 0.0).scale(0.055, 0.055, 0.055) {
                    name = prefix + "_health_overhead"
                    health
                    T.position(-5.0, 3.0, 0.12).scale(1.5, 1.5, 1.5) { label }
                }
            }
            head_display = overhead
            if viewer { head_display = TOP.parent(viewer) { overhead } }
            hitbox = player_hitbox(prefix + "_player_hit_zone")
            hit_zone = hitbox.zone
            on(initial.model, "GLTFInitialized", fn(event) { hitbox.tag_model(event.gltf) })
        }
    }
    if head_display { head.attach(head_display) }
    if hitbox { head.attach(hitbox.root) }
    let root = T {
        name = root_name
        // Voice capture belongs to the player across body replacements.
        if local_player { voice_level }
        if local_player && use_xr {
            Rider.anchor("#local_xr_camera_anchor").movement_root("#local_root").input("#local_xr_locomotion") {
                name = "local_xr_rider"
            }
        }
        if local_player && use_desktop {
            Rider.anchor("#local_desktop_camera_anchor").movement_root("#local_root").input("/#local_desktop_locomotion") {
                name = "local_desktop_rider"
            }
        }
        if local_player || !health {
            T.position(-0.25, 2.2, 0.0).scale(0.1, 0.1, 0.1) { label }
        }
    }
    let driver = PoseInput {}
    let gamepad = null
    if local_player && use_xr {
        gamepad = InputXRGamepad {
            name = "local_xr_locomotion" locomotion() speed(1.5) enable_flying(true)
            stick_mode(InputSettings.xr.stick_mode())
        }
        driver = InputXR.on() { gamepad }
        on_global("DataEvent", fn(event) {
            if event == "InputSettingsChanged" { gamepad.set_stick_mode(InputSettings.xr.stick_mode()) }
        })
    }
    driver.attach(head)
    root.attach(driver)
    let locomotion = root
    let gravity = null
    let velocity = null
    if track_hands || physical {
        velocity = Velocity { name = prefix + "_velocity" root }
        gravity = Gravity.enabled(false) { name = prefix + "_gravity" velocity }
        locomotion = gravity
    }
    fn enable_gravity_when_ready(body) {
        if !gravity { return }
        on(body, "DataEvent", fn(event) {
            if event == "CapsuleReady" { gravity.set_enabled(true) }
        })
        if body.capsule_ready() { gravity.set_enabled(true) }
    }
    enable_gravity_when_ready(avatar)
    let state = { head = head avatar = avatar model = initial.model choice = options.avatar height = null pending = null avatar_timeout = null changed = null capsule = null remote_height = null }
    let held = { left = false right = false desktop = false mounted = false }
    fn holding_item() { return held.left || held.right || held.desktop }
    fn can_swap(report) {
        if held.mounted { report("Dismount to switch avatars") return false }
        if holding_item() { report("Release held items first") return false }
        return true
    }

    // The cameras, hands, capsule and stick locomotion follow the root's scale.
    fn apply_rig_scale(scale) {
        let pose = root.trs()
        root.trs({ translation = pose.translation rotation_quat_xyzw = pose.rotation_quat_xyzw scale = [scale, scale, scale] })
    }
    fn finish_avatar_load() {
        state.pending = null
        state.avatar_timeout.bypass(true)
    }
    fn cancel_avatar_load() {
        if state.pending {
            state.pending.staging.remove_subtree()
            finish_avatar_load()
        }
    }
    state.avatar_timeout = on(world, "FrameTick", fn(event) {
        let pending = state.pending
        if !pending { return }
        pending.elapsed = pending.elapsed + event.dt_sec
        if pending.elapsed < 8.0 { return }
        cancel_avatar_load()
        pending.report(pending.choice + " load timed out. Current avatar kept.", false)
    })
    state.avatar_timeout.bypass(true)
    fn select_avatar(choice, report) {
        // A remote body is built at the peer's height, so tiny peers get the
        // same body and rig scale they see themselves.
        let target_meters = state.remote_height
        if local_player { target_meters = AvatarSettings.height.meters() }
        if state.pending {
            if state.pending.choice == choice && state.pending.height == target_meters {
                report("Loading " + choice + "…")
                return
            }
            cancel_avatar_load()
        }
        if choice == state.choice && target_meters == state.height { report(choice + " is active", true) return }
        if !factories[choice] { report("Avatar unavailable", false) return }
        if !can_swap(report) { return }
        // Load and validate away from the play area while the live body and camera stay mounted.
        let model_meters = target_meters
        let rig_scale = 1.0
        if target_meters != null && target_meters < TINY_BODY_METERS {
            model_meters = TINY_BODY_METERS
            rig_scale = target_meters / TINY_BODY_METERS
        }
        let factory = factories[choice]
        let spec = factory(prefix + "_model", track_hands, bucket, model_meters, relaxed_pose)
        let probe = spec.model
        let staging = T.position(0.0, -10000.0, 0.0) { probe }
        spec.staging = staging
        state.pending = { staging = staging elapsed = 0.0 choice = choice height = target_meters report = report }
        state.avatar_timeout.bypass(false)
        on(probe, "GLTFInitialized", fn(event) {
            if !state.pending || state.pending.staging != staging { return }
            if !can_swap(report) {
                staging.remove_subtree()
                finish_avatar_load()
                return
            }
            let scale = event.gltf.import_scale()
            if choice == "miku" {
                spec.import_scale = scale
                spec.eye_height = MIKU_EYE_GLB * spec.import_scale
            } else {
                let sole = spec.sole_offset
                spec.sole_offset = [sole[0] * scale, sole[1] * scale, sole[2] * scale]
                spec.eye_height = spec.eye_height * scale
            }
            if choice == "miku" && !prepare_miku_arms(event.gltf, spec.import_scale) {
                staging.remove_subtree()
                finish_avatar_load()
                report("Miku rig unavailable", false)
                return
            }
            let body = build_body(spec, choice)
            if gravity { gravity.set_enabled(false) }
            if choice != "miku" { enable_gravity_when_ready(body) }
            if choice != "miku" { animate_face(spec.model, body) }
            if hitbox { hitbox.tag_model(spec.model) }
            // Move the camera and hands before disposing of their old rig.
            mount_body(body, spec, choice)
            // AVC splices its head bone under the pose target, so that target
            // must be retired with the old AVC to remove the entire old rig.
            let next_head = T { name = prefix + "_head_target" body }
            if state.capsule { state.capsule.remove_subtree() state.capsule = null }
            if choice == "miku" {
                // Keep the authored capsule proportional to the imported avatar.
                let capsule_scale = 1.0
                if model_meters != null { capsule_scale = model_meters / MIKU_HEIGHT }
                let capsule = T.position(0, 0.91 * capsule_scale, 0) {
                    name = prefix + "_miku_capsule"
                    Zone.capsule_y(0.28 * capsule_scale, 0.63 * capsule_scale) {
                        Collidable.slide().movement_target(root) {
                            if !local_player { response("read_only") }
                        }
                    }
                }
                root.attach(capsule)
                state.capsule = capsule
                // No AVC capsule reports ready for Miku; this one is in place.
                if gravity { gravity.set_enabled(true) }
            }
            next_head.trs(state.head.trs())
            if local_player && !use_xr { next_head.update_transform([0.0, spec.eye_height, 0.0]) }
            driver.attach(next_head)
            if shield_frame { next_head.attach(shield_frame) }
            if head_display { next_head.attach(head_display) }
            if hitbox { next_head.attach(hitbox.root) }
            detach_avatar_equipment(state.head)
            state.head.remove_subtree()
            state.head = next_head
            state.avatar = body
            state.model = spec.model
            state.choice = choice
            state.height = target_meters
            finish_avatar_load()
            apply_rig_scale(rig_scale)
            if state.changed { state.changed(next_head, spec.model, choice) }
            report(choice + " is active", true)
        })
        root.attach(staging)
        report("Loading " + choice + "…")
    }
    fn apply_desired_height(report) {
        let choice = state.choice
        if state.pending { choice = state.pending.choice }
        select_avatar(choice, report)
    }
    fn desired_height() { return AvatarSettings.height.meters() }
    // Remote players only: the peer's desired height, or null for the model default.
    fn set_remote_height(height, report) {
        if local_player || height == state.remote_height { return }
        state.remote_height = height
        apply_desired_height(report)
    }
    // Local players: false lets go of any seat or deck (the attachment ends
    // the connection on its next tick) and keeps the player from mounting or
    // boarding until set back to true.
    fn set_riding_enabled(value) {
        for name in ["#local_xr_rider", "#local_desktop_rider"] {
            let rider = query(name)
            if rider { rider.set_enabled(value) }
        }
    }
    if local_player {
        on_global("MountStarted", fn(event) { held.mounted = true })
        on_global("MountEnded", fn(event) { held.mounted = false })
    }
    fn watch_pointer(pointer, key) {
        on(pointer, "PointerGrabbed", fn(event) { held[key] = true })
        on(pointer, "PointerReleased", fn(event) { held[key] = false })
    }
    fn blink(slot) {
        if state.choice == "miku" { return 0.0 }
        return state.model.morph_weight(slot)
    }
    fn head_pose() { return state.head.trs() }
    fn third_person_enabled() { return view.third_person }
    fn set_third_person(enabled) {
        if view.third_person == enabled { return }
        view.third_person = enabled
        state.avatar.set_third_person(enabled)
        emit_data(root, "AvatarCameraViewChanged")
    }
    fn toggle_camera_view() { set_third_person(!view.third_person) }
    fn avatar_id() { return state.choice }
    fn flying() {
        if gamepad { return gamepad.flying() }
        return false
    }
    fn set_remote_flying(enabled) { if !local_player { driver.set_flying(enabled) } }
    fn on_avatar_changed(callback) { state.changed = callback }
    fn bind_avatar() { if state.changed { state.changed(state.head, state.model, state.choice) } }
    return {
        dispose = fn() { cancel_avatar_load() state.avatar_timeout.remove() }
        root = root locomotion = locomotion velocity = velocity head = head left = left right = right model = initial.model
        left_driver = left_driver right_driver = right_driver label = label health = health health_view = health_view hit_zone = hit_zone
        shield_mount = shield_mount
        select_avatar = select_avatar watch_pointer = watch_pointer blink = blink head_pose = head_pose
        apply_desired_height = apply_desired_height toggle_camera_view = toggle_camera_view
        desired_height = desired_height set_remote_height = set_remote_height
        set_riding_enabled = set_riding_enabled
        // false holds the legs in their authored pose (seated); true walks.
        set_legs_enabled = fn(enabled) { state.avatar.set_legs_enabled(enabled) }
        flying = flying set_remote_flying = set_remote_flying
        camera_view = { enabled = third_person_enabled set = set_third_person }
        avatar_id = avatar_id on_avatar_changed = on_avatar_changed bind_avatar = bind_avatar cancel_avatar_load = cancel_avatar_load
    }
}
