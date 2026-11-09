import { square_gohei } from "./gohei_squares_common.mms"

// Euclidean automatic weights with a gentle repeating pose demonstration.
export fn gohei_naive_weights(label) {
    if label == null { label = "gohei_naive_weights" }
    return square_gohei(label, true)
}
