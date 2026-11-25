// Deterministic implicit-cloud sky. All clusters use the one color supplied by
// the caller and are kept on a noisy ring outside the scene's centre, leaving
// the playable/interior area clear.
import { cloud } from "cloud.mms"

fn hash01(seed) {
    let x = Math.sin(seed * 12.9898 + 78.233) * 43758.5453
    return x - Math.floor(x)
}

fn cloud_cluster(index, puff_count, max_puff_size, puff_clustering, color) {
    let evenly_spaced_angle = index * 6.283185307179586 / puff_count
    // `puff_clustering` is radians of peak-to-peak angular perturbation. Zero
    // is an even ring; larger values form visibly denser groups and gaps.
    let angular_noise = (hash01(index + 11.0) - 0.5) * puff_clustering
    let theta = evenly_spaced_angle + angular_noise
    let radius = 25.0 + hash01(index + 23.0) * 17.0
    let height = 4.0 + hash01(index + 37.0) * 12.0
    let width = max_puff_size * (0.58 + hash01(index + 47.0) * 0.42)
    let puffs_in_cluster = 3.0
    if hash01(index + 59.0) > 0.42 {
        puffs_in_cluster = 5.0
    }

    return T.position(Math.cos(theta) * radius, height, Math.sin(theta) * radius) {
        cloud(puffs_in_cluster, width, C.rgba(color[0], color[1], color[2], color[3]) {})
    }
}

// `puff_count` controls the number of cloud clusters, while the lower-level
// `cloud` prefab chooses three or five intersecting implicit-surface puffs for
// each cluster. Keep it modest because every cluster bakes one mesh.
export fn clouds(color, puff_count, max_puff_size, puff_clustering) {
    return T {
        for index in range(puff_count) {
            cloud_cluster(index, puff_count, max_puff_size, puff_clustering, color)
        }
    }
}

// Each bank is [x, y, z, uniform scale, yaw, puff count]. Scale the voxel size
// with the prefab so each cloud bakes at the same resolution.
export fn sky_cloud_banks(banks, color) {
    let tints = {}
    let root = T {
        name = "implicit_sky_clouds"
        for index in range(len(banks)) {
            let bank = banks[index]
            let tint = C.rgba(color[0], color[1], color[2], 1.0)
            tints["" + index] = tint
            T.position(bank[0], bank[1], bank[2]).rotation(0.0, bank[4], 0.0)
                .scale(bank[3], bank[3], bank[3]) {
                cloud(bank[5], 3.8, tint, bank[3] * 0.18)
            }
        }
    }
    fn set_tint(rgb) {
        for index in range(len(banks)) {
            tints["" + index].set_color([rgb[0], rgb[1], rgb[2], 1.0])
        }
    }
    return { root = root set_tint = set_tint }
}
