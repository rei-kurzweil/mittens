import { lambert_toon_definition } from "../materials/lambert_toon.mms"

// Overlapping panels for the reference, animated, and spring specimens.
// The prop root is the grip frame, not the middle of the staff or paper.
export fn square_gohei(label, weighted, spring_paper) {
    if spring_paper == null { spring_paper = false }
    let panels = 5
    let step_x = 0.11
    let step_y = -0.08
    let half_width = 0.055
    let half_height = 0.055
    let staff_length = 0.80
    let radius = 0.012
    let paper_color = [0.98, 0.97, 1.0]
    let staff_color = [0.54, 0.30, 0.18]
    let hand_offset = [0.0, 0.0, 0.0]
    let grip_pitch = 0.0
    if spring_paper {
        panels = 7
        step_x = 0.08
        step_y = -0.11
        half_width = 0.04
        half_height = 0.065
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
    let paper = T {
        name = label + "_paper"
        for panel in range(panels) {
            // Adjacent edges meet over a finite vertical interval; panels do
            // not overlap horizontally. Depth steps keep faces distinct.
            T.position(panel * step_x, panel * step_y, panel * 0.002) {
                name = label + "_panel_" + panel
                for side in [-1.0, 1.0] {
                    let yaw = 0.0
                    if side < 0.0 { yaw = Math.pi }
                    T.position(0.0, 0.0, side * 0.001).rotation(0.0, yaw, 0.0)
                        .scale(half_width * 2.0, half_height * 2.0, half_width * 2.0) {
                        R.plane() {
                            paper_material
                            C.rgba(paper_color[0], paper_color[1], paper_color[2], 1.0)
                            Raycastable.enabled()
                        }
                    }
                }
            }
        }
    }
    paper
    let mount = T.position(0.03, staff_length - 0.24, 0.0) {
        name = label + "_paper_mount"
        paper
    }
    mount
    if weighted {
        let first = T { name = label + "_joint_0" Joint {} }
        first
        let parent = first
        let bend = null
        let bend_next = null
        for joint in range(1,panels) {
            let next = T.position(step_x, step_y, 0.002) {
                name = label + "_joint_" + joint
                Joint {}
            }
            next
            parent.attach(next)
            parent = next
            if joint == 1 { bend = next }
            if joint == 2 { bend_next = next }
        }
        // The referenced joint tree is a sibling, not a required Armature child.
        mount.attach(first)
        let armature = Armature.joints(first).give_automatic_weights() {
            name = label + "_armature"
            paper
            if spring_paper {
                SecondaryMotion {
                    name = label + "_secondary_motion"
                    // Joint zero fixes the mount; simulate the six lower joints.
                    SpringBone.from_root("[name='" + label + "_joint_1']")
                        .virtual_end_length_ratio(1.0)
                        .stiffness(0.8).drag_force(0.55)
                        .gravity(0.25, [0, -1, 0])
                        .world_collision(false).avatar_collision(false) {
                            name = label + "_spring"
                    }
                }
            }
        }
        armature
        mount.attach(armature)
        if !spring_paper {
            let bend_transition = Transition { duration_beats(0.8) ease_in_out_sine() replace_same_target() }
            bend_transition
            bend.attach(bend_transition)
            let next_transition = Transition { duration_beats(0.8) ease_in_out_sine() replace_same_target() }
            next_transition
            bend_next.attach(next_transition)
            let demo = Animation.looping().length(8.0) {
                name = label + "_pose_demo"
                Keyframe.at(0.0) {
                    bend.update_transform([step_x,step_y,0.002], [0.0,0.0,0.0], [1.0,1.0,1.0])
                    bend_next.update_transform([step_x,step_y,0.002], [0.0,0.0,0.0], [1.0,1.0,1.0])
                }
                Keyframe.at(2.0) {
                    bend.update_transform([step_x,step_y,0.002], [0.0,0.0,0.40], [1.0,1.0,1.0])
                    bend_next.update_transform([step_x,step_y,0.002], [0.0,0.0,-0.15], [1.0,1.0,1.0])
                }
                Keyframe.at(4.0) {
                    bend.update_transform([step_x,step_y,0.002], [0.0,0.0,-0.30], [1.0,1.0,1.0])
                    bend_next.update_transform([step_x,step_y,0.002], [0.0,0.0,0.20], [1.0,1.0,1.0])
                }
                Keyframe.at(6.0) {
                    bend.update_transform([step_x,step_y,0.002], [0.0,0.0,0.0], [1.0,1.0,1.0])
                    bend_next.update_transform([step_x,step_y,0.002], [0.0,0.0,0.0], [1.0,1.0,1.0])
                }
            }
            demo
            mount.attach(demo)
        }
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
            mount
        }
    }
}
