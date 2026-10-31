// Untracked Lumae: shared AVC bindings/capsule/legs, with its authored skeleton.
import { lumae_colliders } from "colliders.mms"
import { lumae_secondary_motion } from "secondary_motion.mms"
import { pose as relaxed_pose } from "poses/000-relaxed.pose.mms"

export fn lumae_avatar(label, material) {
    return AVC {
        name = label + "_avc"
        // Rest ankle projected vertically onto this GLB's shoe-floor plane (Y=0).
        // Inverse foot-rest transforms give these local sole landmarks; they
        // are Lumae calibration, not the mitten/Bisket shoe offsets.
        crouch_sole_offsets([0.0, 0.0687271, 0.1129329], [0.0, 0.0687271, 0.1129329])
        T {
            GLTF("assets/components/characters/lumae/lumae.glb") {
                name = label
                material
                relaxed_pose()
                lumae_colliders()
                lumae_secondary_motion()
            }
        }
    }
}
