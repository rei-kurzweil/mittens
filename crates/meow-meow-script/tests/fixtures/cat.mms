// cat.mms — a cat made of cubes.
//
// Emits a single component tree at positional index 0.
// Import with: import { 0 as cat } from "cat.mms"
//
// Coordinate system: Y up, +Z forward (toward viewer from behind).
// Cat sits with feet near y=0, head top near y=1.3.

T {
    // body
    T.position(0.0, 0.42, 0.0) {
        scale(0.85, 0.65, 1.3)
        R.cube() { C.rgba(0.85, 0.75, 0.65, 1.0) }
    }

    // head
    T.position(0.0, 0.95, 0.52) {
        scale(0.75, 0.7, 0.75)
        R.cube() { C.rgba(0.85, 0.75, 0.65, 1.0) }
    }

    // ear left
    T.position(-0.22, 1.38, 0.52) {
        scale(0.18, 0.26, 0.13)
        R.cube() { C.rgba(0.85, 0.75, 0.65, 1.0) }
    }

    // ear right
    T.position(0.22, 1.38, 0.52) {
        scale(0.18, 0.26, 0.13)
        R.cube() { C.rgba(0.85, 0.75, 0.65, 1.0) }
    }

    // inner ear left (pink)
    T.position(-0.22, 1.37, 0.56) {
        scale(0.09, 0.14, 0.05)
        R.cube() { C.rgba(0.95, 0.65, 0.72, 1.0) }
    }

    // inner ear right
    T.position(0.22, 1.37, 0.56) {
        scale(0.09, 0.14, 0.05)
        R.cube() { C.rgba(0.95, 0.65, 0.72, 1.0) }
    }

    // eye left
    T.position(-0.17, 0.98, 0.91) {
        scale(0.13, 0.12, 0.04)
        R.cube() { C.rgba(0.08, 0.04, 0.04, 1.0) }
    }

    // eye right
    T.position(0.17, 0.98, 0.91) {
        scale(0.13, 0.12, 0.04)
        R.cube() { C.rgba(0.08, 0.04, 0.04, 1.0) }
    }

    // nose
    T.position(0.0, 0.84, 0.92) {
        scale(0.09, 0.07, 0.04)
        R.cube() { C.rgba(0.92, 0.48, 0.52, 1.0) }
    }

    // tail — angled up and back
    T.position(0.0, 0.58, -0.82) {
        rotation(-38.0, 0.0, 0.0)
        scale(0.11, 0.11, 0.88)
        R.cube() { C.rgba(0.72, 0.62, 0.52, 1.0) }
    }

    // front left leg
    T.position(-0.27, 0.09, 0.36) {
        scale(0.19, 0.38, 0.19)
        R.cube() { C.rgba(0.80, 0.71, 0.61, 1.0) }
    }

    // front right leg
    T.position(0.27, 0.09, 0.36) {
        scale(0.19, 0.38, 0.19)
        R.cube() { C.rgba(0.80, 0.71, 0.61, 1.0) }
    }

    // back left leg
    T.position(-0.27, 0.09, -0.36) {
        scale(0.19, 0.38, 0.19)
        R.cube() { C.rgba(0.80, 0.71, 0.61, 1.0) }
    }

    // back right leg
    T.position(0.27, 0.09, -0.36) {
        scale(0.19, 0.38, 0.19)
        R.cube() { C.rgba(0.80, 0.71, 0.61, 1.0) }
    }
}
