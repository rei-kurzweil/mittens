// Lying down when a player or NPC is downed, and getting back up.
//
// AvatarControl hangs the body from the head target and splits the head off
// onto it, so the whole avatar goes wherever that target goes. Lying down:
//   1. pauses AVC's solvers (AVC.set_paused), so nothing re-poses the body;
//   2. puts the head target where the head lands if the body toppled about
//      its feet: backwards ("back"), or onto its right side ("side");
//   3. rotates the body, about that head point in the body's own frame, by the
//      same fall, so the feet stay where they stood and the neck still meets
//      the head;
//   4. bends the limbs. Only legs and arms: the head follows the head target,
//      and bending the spine would pull the neck away from it.
// The local XR player's view is held at the head (InputXR.lock_head), with a
// quarter of their head's turn let through. Stick and desktop movement stop.
// Getting up puts every touched bone back as it was, unpauses AVC and
// returns the view.
//
// Pose angles are rest-relative Euler rotations (radians) for VRoid J_Bip
// bones. Signs assume the model faces +Z. Zero offsets on the feet clear the
// ankle rotations left by the paused leg solver, including the crawl pose.
let POSES = {
    back = [
        ["J_Bip_L_UpperArm", [0.0, 0.0, -1.15]],
        ["J_Bip_R_UpperArm", [0.0, 0.0, 1.15]],
        ["J_Bip_L_LowerLeg", [0.15, 0.0, 0.0]],
        ["J_Bip_R_LowerLeg", [0.15, 0.0, 0.0]],
        ["J_Bip_L_Foot", [0.0, 0.0, 0.0]],
        ["J_Bip_R_Foot", [0.0, 0.0, 0.0]],
    ]
    side = [
        ["J_Bip_L_UpperLeg", [-1.3, 0.0, 0.0]],
        ["J_Bip_R_UpperLeg", [-1.1, 0.0, 0.0]],
        ["J_Bip_L_LowerLeg", [1.6, 0.0, 0.0]],
        ["J_Bip_R_LowerLeg", [1.5, 0.0, 0.0]],
        ["J_Bip_L_Foot", [0.0, 0.0, 0.0]],
        ["J_Bip_R_Foot", [0.0, 0.0, 0.0]],
        ["J_Bip_L_UpperArm", [-0.7, 0.0, -1.0]],
        ["J_Bip_R_UpperArm", [-0.7, 0.0, 1.0]],
        ["J_Bip_L_LowerArm", [0.0, -1.1, 0.0]],
        ["J_Bip_R_LowerArm", [0.0, 1.1, 0.0]],
    ]
}
// Raises the fallen head so the head and torso rest on the ground.
let LIFT = 0.12
// How much of the headset's motion a downed XR player's view follows.
let VIEW_TRANSLATION_FOLLOW = 0.0
let VIEW_ROTATION_FOLLOW = 0.25

fn qmul(a, b) {
    return [a[3] * b[0] + a[0] * b[3] + a[1] * b[2] - a[2] * b[1],
        a[3] * b[1] - a[0] * b[2] + a[1] * b[3] + a[2] * b[0],
        a[3] * b[2] + a[0] * b[1] - a[1] * b[0] + a[2] * b[3],
        a[3] * b[3] - a[0] * b[0] - a[1] * b[1] - a[2] * b[2]]
}
fn qconj(q) { return [-q[0], -q[1], -q[2], q[3]] }
fn qrot(q, v) {
    let p = qmul(qmul(q, [v[0], v[1], v[2], 0.0]), qconj(q))
    return [p[0], p[1], p[2]]
}
fn axis_angle(axis, angle) {
    let s = Math.sin(angle / 2.0)
    return [axis[0] * s, axis[1] * s, axis[2] * s, Math.cos(angle / 2.0)]
}
fn sub(a, b) { return [a[0] - b[0], a[1] - b[1], a[2] - b[2]] }
fn add(a, b) { return [a[0] + b[0], a[1] + b[1], a[2] + b[2]] }

// The head's level forward direction (heads look along -Z).
fn level_forward(q) {
    let f = qrot(q, [0.0, 0.0, -1.0])
    let length = Math.sqrt(f[0] * f[0] + f[2] * f[2])
    if length < 0.0001 { return [0.0, 0.0, -1.0] }
    return [f[0] / length, 0.0, f[2] / length]
}

// The world rotation of the fall. Backwards turns up into -forward (the face
// ends up looking at the sky); sideways turns up into the right.
fn fall_rotation(kind, forward) {
    let right = [-forward[2], 0.0, forward[0]]
    if kind == "side" { return axis_angle(forward, Math.pi / 2.0) }
    return axis_angle(right, Math.pi / 2.0)
}

// kind: "back" or "side". controls (optional): { desktop } for a local
// desktop player's movement (an XR player's stick is player.gamepad).
// Returns false when the player is already down or has no body yet.
export fn lie_down(player, kind, controls) {
    if player["downed"] { return false }
    let avatar = player.avatar()
    let head = player.head_target()
    let lay = player.lay
    if !avatar || !head || !lay.frame { return false }
    if kind != "side" { kind = "back" }
    avatar.set_paused(true)

    let pose = head.world_trs()
    let h = pose.translation
    let forward = level_forward(pose.rotation_quat_xyzw)
    let yaw = axis_angle([0.0, 1.0, 0.0], Math.atan2(-forward[0], -forward[2]))
    let fall = fall_rotation(kind, forward)
    // Topple about the point on the ground under the head.
    let ground = player.root.world_trs().translation[1]
    let feet = [h[0], ground, h[2]]
    let landed = add(add(feet, qrot(fall, sub(h, feet))), [0.0, LIFT, 0.0])
    let landed_rotation = qmul(fall, yaw)

    // The body frame follows the head's translation and holds its yaw while
    // paused, so the same fall expressed in that frame lays it down about
    // the head point.
    let body = lay.model_frame.world_trs()
    let inverse = qconj(body.rotation_quat_xyzw)
    let pivot = qrot(inverse, sub(h, body.translation))
    let turn = qmul(qmul(inverse, fall), body.rotation_quat_xyzw)
    lay.frame.trs({ translation = sub(pivot, qrot(turn, pivot)) rotation_quat_xyzw = turn scale = [1.0, 1.0, 1.0] })

    let state = { standing = pose marker = null bones = [] controls = null }
    let xr = player.xr_input()
    if xr {
        let marker = T { name = "downed_view" }
        marker
        marker.world_trs({ translation = landed rotation_quat_xyzw = landed_rotation scale = [1.0, 1.0, 1.0] })
        xr.lock_head(marker, VIEW_TRANSLATION_FOLLOW, VIEW_ROTATION_FOLLOW)
        state.marker = marker
    } else {
        head.world_trs({ translation = landed rotation_quat_xyzw = landed_rotation scale = [1.0, 1.0, 1.0] })
    }
    if player.gamepad { player.gamepad.disable() }
    if controls && controls["desktop"] { controls.desktop.disable() }
    state.controls = controls
    if player.velocity { player.velocity.reset() }

    let model = player.current_model()
    for entry in POSES[kind] {
        let bone = model.query("[name='" + entry[0] + "']")
        if bone {
            state.bones.push([bone, bone.trs()])
            bone.rest_relative_rotation(entry[1])
        }
    }
    player.downed = state
    return true
}

export fn get_up(player) {
    let state = player["downed"]
    if !state { return false }
    player.downed = null
    for saved in state.bones { saved[0].trs(saved[1]) }
    player.lay.frame.trs({ translation = [0.0, 0.0, 0.0] rotation_quat_xyzw = [0.0, 0.0, 0.0, 1.0] scale = [1.0, 1.0, 1.0] })
    let xr = player.xr_input()
    if xr { xr.unlock_head() } else { player.head_target().world_trs(state.standing) }
    if state.marker { state.marker.remove_subtree() }
    if player.gamepad { player.gamepad.enable() }
    if state.controls && state.controls["desktop"] { state.controls.desktop.enable() }
    player.avatar().set_paused(false)
    return true
}

export fn is_down(player) { return player["downed"] != null }

// on_death / on_respawn for a local player: leave any seat, lie down
// (alternating on the back and on the side), and get back up. controls() is
// called at death for lie_down's controls, e.g. the desktop locomotion.
export fn player_down_hooks(player, controls) {
    let fall = { count = 0 }
    fn down() {
        player.set_riding_enabled(false)
        let kind = "back"
        if fall.count % 2 == 1 { kind = "side" }
        fall.count = fall.count + 1
        let held = null
        if controls { held = controls() }
        lie_down(player, kind, held)
    }
    fn up() {
        get_up(player)
        player.set_riding_enabled(true)
    }
    return { down = down up = up }
}
