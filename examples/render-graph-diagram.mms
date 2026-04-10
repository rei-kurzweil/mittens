// render-graph-diagram.mms
//
// A scene that exercises every cat engine render phase, with labels matching
// the phase colours from docs/spec/render-graph-pipeline.svg.
//
// Label colour palette (matching SVG left-bar accents):
//   Phase 1  Background          indigo  #5c6bc0  text #9fa8da
//   Phase 3  Opaque              green   #2e7d32  text #81c784
//   Phase 3  Emissive/Bloom src  blue    #29b6f6  text #90caf9
//
// Phases not yet in the MMS component registry (commented out below):
//   Phase 2  Background Occluded+Lit   (no MMS ctor; spawned from BG in Rust only)
//   Phase 4  Cutout / Alpha-tested     (TC not in registry)
//   Phase 5  Transparent single-layer  (Opacity not in registry)
//   Phase 6  Transparent multi-layer   (Opacity not in registry)
//   Phase 7  Overlay                   (OV not in registry)
//
// Future RenderGraph syntax:
//
//   RenderGraph {
//       Bloom.radius(0.2).intensity(0.8) {
//           quality = 0.5
//           EmissiveSource
//       }
//       Bokeh {
//           focus_distance = 3.0
//           aperture = 0.08
//           max_blur_radius = 10.0
//       }
//   }
//
// See: docs/spec/render-graph-post-processing.md
//      docs/spec/render-graph-pipeline.svg
//      docs/spec/render-graph-pipeline-post-processing.svg

// ── Helpers ───────────────────────────────────────────────────────────────────
// Label topology (matches text-example.rs):
//
//   T.position(x, y, z) {
//       T.scale(s, s, 1.0) {
//           C.rgba(text_r, text_g, text_b, 1.0) {   ← ancestor of glyph quads
//               TXT {
//                   "text content"
//                   TextBackground {
//                       padding(p)
//                       C.rgba(bg_r, bg_g, bg_b, a)  ← bg quad colour
//                   }
//                   TextureFiltering.nearest_magnification()
//               }
//           }
//       }
//   }

// ── Lighting ─────────────────────────────────────────────────────────────────

AL { C.rgba(0.04, 0.04, 0.06, 1.0) }

T.position(4.0, 6.0, 3.0) {
    DL { intensity(1.1)  C.rgba(1.0, 0.92, 0.78, 1.0) }
}

T.position(-3.0, 3.0, -4.0) {
    DL { intensity(0.3)  C.rgba(0.5, 0.65, 1.0, 1.0) }
}

// ── Phase 1: Background ───────────────────────────────────────────────────────
// Objects under BG render before foreground with no depth write.

BG {
    T.position(0.0, -0.52, 0.0).scale(30.0, 0.04, 30.0) {
        R.cube() { C.rgba(0.04, 0.04, 0.06, 1.0) }
    }
    T.position(0.0, 1.0, -18.0).scale(60.0, 6.0, 0.2) {
        R.cube() { C.rgba(0.05, 0.05, 0.08, 1.0) }
    }
}

// Phase 1 label — floats above the background ground, left side.
// Indigo text #9fa8da on dark indigo bg #0d1433 (matching SVG Phase-1 bar).
T.position(-5.2, 0.6, -3.5) {
    T.scale(0.19, 0.19, 1.0) {
        C.rgba(0.62, 0.66, 0.86, 1.0) {
            TXT {
                "Ph.1  Background"
                TextureFiltering.nearest_magnification()
            }
        }
    }
}

// ── Phase 3: Opaque ───────────────────────────────────────────────────────────
// Standard toon-shaded geometry with full depth test + write.

T.position(-2.4, 0.0, 0.0).scale(0.7, 1.0, 0.7) {
    R.cube() { C.rgba(0.18, 0.26, 0.55, 1.0) }
}

T.position(0.0, 0.0, 0.0).scale(0.7, 1.0, 0.7) {
    R.cube() { C.rgba(0.55, 0.18, 0.18, 1.0) }
}

T.position(2.4, 0.0, 0.0).scale(0.7, 1.0, 0.7) {
    R.cube() { C.rgba(0.18, 0.52, 0.22, 1.0) }
}

T.position(4.6, 0.0, -0.5).scale(0.6, 0.6, 0.6) {
    R.tetrahedron() { C.rgba(0.55, 0.38, 0.08, 1.0) }
}

// Phase 3 label — in front of the pedestal row, low.
// Green text #81c784 on dark green bg #0a1f0a (matching SVG Phase-3 bar).
T.position(0.0, -0.32, 1.1) {
    T.scale(0.19, 0.19, 1.0) {
        C.rgba(0.51, 0.78, 0.52, 1.0) {
            TXT {
                "Ph.3  Opaque"
                TextureFiltering.nearest_magnification()
            }
        }
    }
}

// ── Phase 3 (emissive) — future Bloom source ─────────────────────────────────
// Emissive objects are in the Opaque phase today.  Once RenderGraphComponent
// is implemented they feed the dedicated Emissive Prepass ([2] in the PP graph).

T.position(-2.4, 1.5, 0.0).scale(0.32, 0.32, 0.32) {
    R.sphere() { C.rgba(1.0, 0.45, 0.1, 1.0)   Emissive {} }
}

T.position(0.0, 1.5, 0.0).scale(0.32, 0.32, 0.32) {
    R.sphere() { C.rgba(0.45, 0.75, 1.0, 1.0)  Emissive {} }
}

T.position(2.4, 1.5, 0.0).scale(0.32, 0.32, 0.32) {
    R.sphere() { C.rgba(0.6, 1.0, 0.25, 1.0)   Emissive {} }
}

// Emissive label — floats above the orb row.
// Bloom-blue text #90caf9 on dark blue bg #001435 (matching SVG Bloom block).
T.position(0.0, 2.55, 1.1) {
    T.scale(0.19, 0.19, 1.0) {
        C.rgba(0.56, 0.79, 0.98, 1.0) {
            TXT {
                "Ph.3  Emissive  (future Bloom source)"
                TextureFiltering.nearest_magnification()
            }
        }
    }
}

// Small accent emissive dots — scattered in background space.
T.position(-3.8, 0.0, -2.0).scale(0.1, 0.1, 0.1) {
    R.sphere() { C.rgba(1.0, 0.7, 0.2, 1.0)  Emissive {} }
}
T.position(1.2, 0.0, -3.5).scale(0.1, 0.1, 0.1) {
    R.sphere() { C.rgba(0.3, 0.8, 1.0, 1.0)  Emissive {} }
}
T.position(3.5, 0.0, -1.5).scale(0.1, 0.1, 0.1) {
    R.sphere() { C.rgba(0.9, 1.0, 0.3, 1.0)  Emissive {} }
}
T.position(-1.5, 0.0, -4.0).scale(0.08, 0.08, 0.08) {
    R.sphere() { C.rgba(1.0, 0.3, 0.6, 1.0)  Emissive {} }
}
