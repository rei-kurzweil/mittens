import { gohei_shide } from "./gohei_shide.mms"
import { lambert_toon_definition } from "../materials/lambert_toon.mms"

// Overlapping panels for the reference, animated, and spring specimens.
// The prop root is the grip frame, not the middle of the staff or paper.
export fn square_gohei(label, weighted, spring_paper) {
    if spring_paper == null { spring_paper = false }
    let staff_length = 0.80
    let radius = 0.012
    let staff_color = [0.54, 0.30, 0.18]
    let hand_offset = [0.0, 0.0, 0.0]
    let grip_pitch = 0.0
    if spring_paper {
        staff_length = 1.28
        radius = 0.006
        staff_color = [0.012, 0.009, 0.018]
        // Retain the 5 cm pullback, shift 1.5 cm left, and lean toward -Z.
        hand_offset = [-0.015, 0.0, -0.055]
        grip_pitch = -Math.pi / 6.0
    }
    let paper_material = Material(lambert_toon_definition("near")).inputs({
        shade_color = [0.76, 0.73, 0.84], shade_strength = 0.32,
        shade_threshold = 0.0, lit_threshold = 1.2,
        rim_color = [1.0, 0.95, 1.0], rim_strength = 0.04, rim_power = 3.5
    })
    let staff_material = Material(MaterialDefinition.anime())
    if spring_paper {
        paper_material = Material(MaterialDefinition.anime()).inputs({
            shade_color = [0.76, 0.73, 0.84], shade_strength = 0.32,
            rim_color = [0.85, 0.60, 1.0], rim_strength = 2.5, rim_power = 2.0
        })
        staff_material = Material(MaterialDefinition.anime()).inputs({
            rim_color = [0.85, 0.60, 1.0], rim_strength = 2.5, rim_power = 2.0
        })
    }
    return T {
        name = label
        Grabbable.to_hand(true).hand_offset(hand_offset) {}
        // Keep the adjustment below the snapped grip root so hand alignment
        // preserves it. Staff and paper rotate together about the grip origin.
        T.rotation(grip_pitch, 0.0, 0.0) {
            name = label + "_grip_adjustment"
            // Grip at zero leaves 18 cm of staff below the hand.
            T.position(0.0, -0.18, 0.0) {
                name = label + "_staff"
                R.cylinder(radius, staff_length, 16) {
                    staff_material
                    C.rgba(staff_color[0], staff_color[1], staff_color[2], 1.0)
                    Raycastable.enabled()
                }
            }
            gohei_shide(label, paper_material, weighted, spring_paper, staff_length, 1.0)
            if spring_paper {
                gohei_shide(label+"_second_shide", paper_material, weighted, spring_paper, staff_length, -1.0)
            }
        }
    }
}
