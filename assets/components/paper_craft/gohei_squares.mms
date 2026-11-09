import { square_gohei } from "./gohei_squares_common.mms"

// Unrigged geometry reference; later versions remain separate spawnable prefabs.
export fn gohei_squares(label) {
    if label == null { label = "gohei_squares" }
    return square_gohei(label, false)
}
