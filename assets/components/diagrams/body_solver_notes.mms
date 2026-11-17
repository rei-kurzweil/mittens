// Notes for examples/body-solver-flowchart.mms. Desktop, 2026-11-16.
// Card text only: this module registers no components.
export fn body_solver_notes() {
    return {
        avc_settings = {
            title = "AvatarControl settings"
            status = "AUTHORED / per avatar"
            summary = "posture(...) and collision_detail(...) opt an avatar in."
            detail = "posture(\"vertical\") is the default and never leaves vertical. \"auto\" lets the body solver switch on head height. \"horizontal\" forces hands and knees. collision_detail(\"detailed\") adds torso spheres on the hips, chest, and upper chest."
            contract = "Settings are read every frame; nothing is cached at bind time. Defaults keep existing scenes unchanged."
            example = "AVC.movement_target(\"[name='root']\") {\n    posture(\"auto\")\n    collision_detail(\"detailed\")\n}"
            source = "src/engine/ecs/component/avatar_control.rs: BodyPosture / CollisionDetail"
        }
        body_solver = {
            title = "Body solver"
            status = "DECIDES / ik/body"
            summary = "Mode, blend weight, torso pitch, last known floor."
            detail = "Runs first each frame, after AVC has placed the head mount. Picks vertical or horizontal for each avatar, eases a 0..1 weight over 0.35 s, solves the torso pitch, and keeps the last floor height the lower body reported."
            contract = "The only place posture is decided. Other solvers read its output; none of them guess a mode. It writes no bones itself."
            example = "mode(id) -> Vertical | Horizontal\nweight(id) -> 0.0 ..= 1.0\ntorso_pitch(id) -> radians\nfloor(id) -> Option<f32>"
            source = "src/engine/ecs/system/ik/body/mod.rs: BodySolverSystem::tick"
        }
        upper_body = {
            title = "Upper body solver"
            status = "WRITES TORSO / ik/upper_body"
            summary = "Applies the pitch to hips, spine, chest, upper chest."
            detail = "Binds the spine rig from the humanoid map (hips, spine, chest, upper chest, neck, head) and writes the body solver's pitch: half at the pelvis, the rest split over three spine bones, all about the avatar's left axis."
            contract = "The only torso writer. No modes of its own: pitch zero restores the rest pose once. Arms are still ik_system chains."
            example = "pitch 1.6 rad -> hips 0.8, spine 0.27,\nchest 0.27, upper chest 0.27"
            source = "src/engine/ecs/system/ik/upper_body/mod.rs\nsrc/engine/ecs/system/ik/upper_body/rig.rs: SpineRig::write"
        }
        head_neck = {
            title = "Head / neck placement"
            status = "UNCHANGED / reads live pose"
            summary = "Places the body root under the authoritative head."
            detail = "Closes head -> neck -> root using the neck parent's live world pose. Because the upper body wrote the pitched torso before it runs, it places the root correctly under the head with no posture logic of its own."
            contract = "The head (XR or desktop) stays authoritative. Root rotation stays yaw only; the pitch lives in the torso bones."
            example = "root = head - FK(root -> neck -> head)\n// FK already includes the pitched spine"
            source = "src/engine/ecs/system/ik/head_neck_body_constraint.rs"
        }
        feedback = {
            title = "Previous-frame feedback"
            status = "READ BY BODY SOLVER"
            summary = "Floor heights, Limited status, spine geometry."
            detail = "The body solver reads the lower body's previous-frame foot support heights and Limited status, and the upper body's spine rig lengths. That is a one-frame-delayed loop, not strict top-down."
            contract = "No events: the systems share this state directly. Losing support keeps the last known floor."
            example = "lower.feet(id) -> support_height\nlower.status(id) -> Limited\nupper.spine(id) -> segments, thigh_length"
            source = "src/engine/ecs/system/ik/body/mod.rs: tick inputs"
        }
        lower_body = {
            title = "Lower body solver"
            status = "WRITES LEGS / ik/lower_body"
            summary = "Kneels at weight >= 0.5, otherwise stands."
            detail = "sync_modes copies a kneeling flag (blend weight at least 0.5) and the body solver's floor into each avatar. system.rs then runs kneel.rs, or the existing crouch, walking, and airborne policies."
            contract = "The only leg writer. The behavior modules below it never see posture; system.rs chooses between them."
            example = "lower.sync_modes(&body)\n// kneeling -> kneel::solve\n// else -> crouch / walking / airborne"
            source = "src/engine/ecs/system/ik/lower_body/system.rs"
        }
        policies = {
            title = "Leg behavior modules"
            status = "POLICIES / goals only"
            summary = "crouch, walking, airborne, kneel produce leg goals."
            detail = "Each module takes plain inputs (hip frames, floor, time) and returns goals. None reads the body mode. See panel 02 for each one."
            contract = "Goals in, goals out; the lower body system validates and emits both legs together."
            example = "crouch::evaluate(...)\nwalking.advance(...)\nkneel::solve(...)"
            source = "src/engine/ecs/system/ik/lower_body/"
        }
        posture_event = {
            title = "PostureChanged"
            status = "EVENT / outward only"
            summary = "mode, previous, pose, reason; scoped to the AVC."
            detail = "Emitted when the body solver decides a switch. pose is \"standing\" or \"hands_and_knees\"; reason is \"head_height\", \"crouch_exhausted\", or \"forced\". The visible blend follows over 0.35 s."
            contract = "For MMS and other observers. The solvers do not depend on it."
            example = "on(avatar, \"PostureChanged\", fn(e) {\n    print(e.pose + \" / \" + e.reason)\n})"
            source = "src/engine/ecs/signals/signal.rs: EventSignal::PostureChanged"
        }
        lower_system = {
            title = "lower_body/system.rs"
            status = "SELECTS / per avatar"
            summary = "Binds legs, queries floor, picks one policy."
            detail = "Per avatar: binds the leg rig, finds support, then runs exactly one policy. Kneeling avatars go to kneel.rs; others get crouch rest goals, walking gait when enabled, and airborne goals without support."
            contract = "Validates both legs before emitting either; restores rest poses when an avatar loses its rig or binding."
            example = "kneeling -> kneel\nwalking enabled -> walking\nunsupported -> airborne\nelse -> crouch"
            source = "src/engine/ecs/system/ik/lower_body/system.rs: solve_frame"
        }
        support = {
            title = "support.rs"
            status = "SHARED / floor queries"
            summary = "Flat support from the retained collidable BVH."
            detail = "Finds the floor under a point near the capsule bottom (or the previous floor for rigs without collision). Every policy uses its results."
            contract = "Read-only. While kneeling, the floor carries over from standing and a query may only replace it within 0.3 m."
            example = "ground.at(point) -> Support { zone, height }"
            source = "src/engine/ecs/system/ik/lower_body/support.rs"
        }
        leg = {
            title = "leg.rs"
            status = "SHARED / two-bone solve"
            summary = "Hip -> knee -> ankle with a pole and a knee limit."
            detail = "Turns an ankle goal and a bend direction into thigh, shin, and foot rotations, and reports the solved knee and sole."
            contract = "Clamps to reachable lengths and the knee flexion limit; reports residual instead of stretching."
            example = "solve_leg(leg, hip_frame, sole_offset, goal, pole, knee_limit)"
            source = "src/engine/ecs/system/ik/lower_body/leg.rs"
        }
        next_steps = {
            title = "Planted hands, lying down"
            status = "PROPOSED"
            planned = true
            summary = "Next postures and contacts, not built yet."
            detail = "Untracked hands planted under the shoulders; lying on the stomach or back as more horizontal poses; upright kneeling between crouch and hands and knees."
            contract = "Each would be a new body solver pose plus policies in the upper and lower body solvers."
            example = "pose: \"stomach\" | \"back\" | \"kneeling\""
            source = "docs/task/body-solver-horizontal-mode.md"
        }
        crouch = {
            title = "crouch.rs"
            status = "POLICY / vertical"
            summary = "Rest foot goals raised out of the floor."
            detail = "Starts from each foot's rest pose under the current hips and lifts it by its floor penetration, which bends the knees as the head lowers."
            contract = "No gait state and no joint writes; returns goals only."
            example = "evaluate(base, support, enabled) -> RestGoal"
            source = "src/engine/ecs/system/ik/lower_body/crouch.rs"
        }
        walking = {
            title = "walking.rs"
            status = "POLICY / vertical"
            summary = "Stance and swing feet while the body moves."
            detail = "Plants feet and swings one at a time as the body moves away from them, using the WalkingIK stride settings."
            contract = "Reset whenever kneeling starts or stops."
            example = "WalkingIK { step_scale(1.3) }"
            source = "src/engine/ecs/system/ik/lower_body/walking.rs"
        }
        airborne = {
            title = "airborne.rs"
            status = "POLICY / vertical"
            summary = "Legs relax while there is no support."
            detail = "Without support under the feet (jumping, flying, falling), feet ease toward a relaxed airborne goal instead of reaching for a floor."
            contract = "Only with walking enabled; flying skips kneeling too."
            example = "airborne.goal(rest_foot, scale)"
            source = "src/engine/ecs/system/ik/lower_body/airborne.rs"
        }
        kneel = {
            title = "kneel.rs"
            status = "POLICY / horizontal"
            summary = "Planted knees, flat shins, crawling gait."
            detail = "Each knee's spot is where the thigh reaches the floor: under the hip, or ahead of it when the hips are low. A knee steps once its hip moves 0.12 m since it landed, one leg at a time, alternating."
            contract = "Planted spots set direction; thigh length sets distance, so knees stay on the floor as the hips move."
            example = "STEP_TRIGGER 0.12 m, SWING 0.28 s,\nLIFT 0.06 m, OVERSHOOT 35%"
            source = "src/engine/ecs/system/ik/lower_body/kneel.rs: KneelGait"
        }
        mode_selection = {
            title = "Mode selection"
            status = "BODY SOLVER / hysteresis"
            summary = "Enter < 42% head height (55% if Limited); exit > 60%."
            detail = "Head height is measured above the last known floor, against the rig's standing head height. \"vertical\" and \"horizontal\" posture settings skip the thresholds."
            contract = "At least 0.4 s in a mode before auto switches again. Emits PostureChanged on each switch."
            example = "enter: head < 0.42 * standing\n   or Limited && head < 0.55 * standing\nexit:  head > 0.60 * standing"
            source = "src/engine/ecs/system/ik/body/mod.rs: tick"
        }
        blend = {
            title = "Blend weight"
            status = "BODY SOLVER / 0.35 s"
            summary = "Eases in and out; legs kneel at weight >= 0.5."
            detail = "The weight moves toward 1 in horizontal and 0 in vertical. While blending out, the last solved pitch fades with the weight instead of snapping upright under kneeling legs."
            contract = "Torso pitch = weight * held pitch."
            example = "weight += clamp(goal - weight, -dt/0.35, dt/0.35)"
            source = "src/engine/ecs/system/ik/body/mod.rs: held_pitch"
        }
        pitch_solve = {
            title = "Torso pitch solve"
            status = "BODY SOLVER / bisection"
            summary = "Hips one thigh length above the floor."
            detail = "Bisects the pitch in [0, 100 deg] so the hip joints sit one thigh length plus knee clearance above the floor below a fixed head. Clamped when the head is too low; then the knees move ahead of the hips."
            contract = "Assumes the neck follows the upper chest; the head/neck closure absorbs the rest."
            example = "hip_joint_height(pitch) == floor + thigh + 0.06"
            source = "src/engine/ecs/system/ik/body/mod.rs: solve_pitch"
        }
        yaw = {
            title = "Body yaw deadband"
            status = "BODY SOLVER / writes operator"
            summary = "45 deg standing, blends to 10 deg on all fours."
            detail = "Tightens the body operator's yaw-follow threshold as horizontal blends in, so the torso tracks the head. Yaw extraction also uses the head's up axis, so looking straight down stays stable."
            contract = "The body solver's one direct write to another component. Standing restores the authored threshold."
            example = "threshold = authored + (10 deg - authored) * weight"
            source = "src/engine/ecs/system/ik/body/mod.rs: apply_body_yaw\nsrc/engine/ecs/system/transform_operator.rs: extract_yaw_from_quat"
        }
    }
}
