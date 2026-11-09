import { lambert_toon_definition } from "../materials/lambert_toon.mms"

// First deliberately naive construction: five overlapping square panels.
// The prop root is the grip frame, not the middle of the staff or paper.
export fn square_gohei(label, weighted) {
    let paper_material = Material(lambert_toon_definition("near")).inputs({
        shade_color = [0.76, 0.73, 0.84], shade_strength = 0.32,
        shade_threshold = 0.0, lit_threshold = 1.2,
        rim_color = [1.0, 0.95, 1.0], rim_strength = 0.04, rim_power = 3.5
    })
    let paper = T {
        name = label + "_paper"
        for panel in range(5) {
            // A small overlap gives this naive version finite visible joins.
            // Each panel has front/back faces, without scaling its mesh frame.
            T.position(panel * 0.08, -panel * 0.08, 0.0) {
                name = label + "_panel_" + panel
                for side in [-1.0, 1.0] {
                    let yaw = 0.0
                    if side < 0.0 { yaw = Math.pi }
                    T.position(0.0, 0.0, side * 0.001).rotation(0.0, yaw, 0.0) {
                        R.polygon("paper-craft/naive-square/v1", [[-0.055,-0.055],[0.055,-0.055],[0.055,0.055],[-0.055,0.055]]) {
                            paper_material
                            C.rgba(0.98, 0.97, 1.0, 1.0)
                            Raycastable.enabled()
                        }
                    }
                }
            }
        }
    }
    paper
    let mount = T.position(0.03, 0.56, 0.0) {
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
        for joint in range(1,5) {
            let next = T.position(0.08, -0.08, 0.0) {
                name = label + "_joint_" + joint
                Joint {}
            }
            next
            parent.attach(next)
            parent = next
            if joint == 1 { bend = next }
            if joint == 2 { bend_next = next }
        }
        let armature = Armature { name = label + "_armature" first }
        armature
        mount.attach(armature)
        let binding = Rigging.new(armature, paper).give_automatic_weights() {
            name = label + "_rigging"
        }
        binding
        mount.attach(binding)
        let bend_transition = Transition { duration_beats(0.8) ease_in_out_sine() replace_same_target() }
        bend_transition
        bend.attach(bend_transition)
        let next_transition = Transition { duration_beats(0.8) ease_in_out_sine() replace_same_target() }
        next_transition
        bend_next.attach(next_transition)
        let demo = Animation.looping().length(8.0) {
            name = label + "_pose_demo"
            Keyframe.at(0.0) {
                bend.update_transform([0.08,-0.08,0.0], [0.0,0.0,0.0], [1.0,1.0,1.0])
                bend_next.update_transform([0.08,-0.08,0.0], [0.0,0.0,0.0], [1.0,1.0,1.0])
            }
            Keyframe.at(2.0) {
                bend.update_transform([0.08,-0.08,0.0], [0.0,0.0,0.40], [1.0,1.0,1.0])
                bend_next.update_transform([0.08,-0.08,0.0], [0.0,0.0,-0.15], [1.0,1.0,1.0])
            }
            Keyframe.at(4.0) {
                bend.update_transform([0.08,-0.08,0.0], [0.0,0.0,-0.30], [1.0,1.0,1.0])
                bend_next.update_transform([0.08,-0.08,0.0], [0.0,0.0,0.20], [1.0,1.0,1.0])
            }
            Keyframe.at(6.0) {
                bend.update_transform([0.08,-0.08,0.0], [0.0,0.0,0.0], [1.0,1.0,1.0])
                bend_next.update_transform([0.08,-0.08,0.0], [0.0,0.0,0.0], [1.0,1.0,1.0])
            }
        }
        demo
        mount.attach(demo)
    }
    return T {
        name = label
        Grabbable {}
        // Local +Y points along the staff. Grip at zero leaves 18 cm below hand.
        T.position(0.0, -0.18, 0.0) {
            name = label + "_staff"
            R.cylinder(0.012, 0.80, 16) {
                Material(MaterialDefinition.anime())
                C.rgba(0.54, 0.30, 0.18, 1.0)
                Raycastable.enabled()
            }
        }
        mount
    }
}
