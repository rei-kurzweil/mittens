// A one-time pistol grip for a VRoid/Bisket hand: middle, ring and little
// fingers wrapped round the grip, the index finger resting on the trigger.
// Same joint convention as controller_hand_curls: fingers extend along local
// +Y and flex toward local -Z. Apply once the GLTF has spawned, e.g.
//   on(model, "GLTFInitialized", fn(event) { pistol_grip(event.gltf, "Right") })
let PHALANX_ANGLES = [1.05, 1.25, 0.75]

// Mitten's authored wrist uses +Y toward the middle finger. Convert an
// upright -Z aiming frame to the bind basis consumed by the external driver.
// The GLB's middle finger defines forward and little-to-index defines up.
export fn mitten_right_wrist_basis() {
    return [-0.504981, 0.455556, -0.504982, 0.531467]
}

fn curl_finger(model, prefix, finger, curl) {
    let segments = ["1", "2", "3"]
    for i in range(3) {
        let bone = model.query("[name='" + prefix + finger + segments[i] + "']")
        if bone { bone.rest_relative_rotation([-PHALANX_ANGLES[i] * curl, 0.0, 0.0]) }
    }
}

export fn pistol_grip(model, side) {
    let prefix = "J_Bip_R_"
    if side == "Left" { prefix = "J_Bip_L_" }
    curl_finger(model, prefix, "Index", 0.45)
    for finger in ["Middle", "Ring", "Little"] { curl_finger(model, prefix, finger, 1.0) }
}
