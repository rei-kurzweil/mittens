// One independently weighted paper strand. Joint zero stays fixed at the staff.
export fn gohei_shide(label, paper_material, weighted, spring_paper, staff_length, side) {
    let panels=5
    let step_x=0.11
    let step_y=-0.08
    let half_width=0.055
    let half_height=0.055
    let depth=0.0
    let paper_color=[0.98,0.97,1.0]
    if spring_paper {
        panels=7 step_x=0.08 step_y=-0.11 half_width=0.04 half_height=0.065
        if side<0 {depth=-0.012}
    }
    step_x=step_x*side
    let paper = T {
        name = label + "_paper"
        for panel in range(panels) {
            // Adjacent edges meet over a finite vertical interval; panels do
            // not overlap horizontally. Depth steps keep faces distinct.
            T.position(panel * step_x, panel * step_y, panel * 0.002) {
                name = label + "_panel_" + panel
                for face_side in [-1.0, 1.0] {
                    let yaw = 0.0
                    if face_side < 0.0 { yaw = Math.pi }
                    T.position(0.0, 0.0, face_side * 0.001).rotation(0.0, yaw, 0.0)
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
    let mount = T.position(0.03 * side, staff_length - 0.24, depth) {
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
                        .stiffness(0.75).drag_force(0.50)
                        .gravity(2.7, [0, -1, 0])
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
    return mount
}
