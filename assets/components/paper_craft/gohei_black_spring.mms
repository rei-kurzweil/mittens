import { square_gohei } from "./gohei_squares_common.mms"

// Longer, thinner black staff and seven white paper panels with light gravity.
export fn gohei_black_spring(label) {
    if label == null { label = "gohei_black_spring" }
    return square_gohei(label, true, true)
}
