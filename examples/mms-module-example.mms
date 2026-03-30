// mms-module-example.mms — Phase 6 module import demo.
//
// Imports cat.mms and places it in a lit scene.
// The cat CE is embedded inside a parent transform, exercising the
// Positional(ComponentExpr) → Child promotion path in the evaluator.

import { 0 as cat } from "cat.mms"

// Place the cat at the scene origin.
T.position(0.0, 0.0, 0.0) {
    cat
}

// Warm key light from upper-right-front.
T.position(2.5, 4.0, 2.5) {
    DL {
        intensity(1.0)
        C.rgba(1.0, 0.94, 0.85, 1.0)
    }
}

// Cool fill light from left.
T.position(-3.5, 1.5, 0.5) {
    DL {
        intensity(0.4)
        C.rgba(0.4, 0.55, 1.0, 1.0)
    }
}

// Soft back rim.
T.position(0.0, 3.0, -3.0) {
    DL {
        intensity(0.25)
        C.rgba(0.9, 0.85, 1.0, 1.0)
    }
}

// Ambient — low warm fill so shadows aren't pure black.
AL { C.rgba(0.10, 0.09, 0.07, 1.0) }
