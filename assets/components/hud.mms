// Mount to a view-aligned transform (-Z forward). Position is in metres.
// A stretch spring lets the content lag in translation while keeping it upright
// in the view's own frame. The contents keep their authored size.
export fn hud(name, position, content) {
    let display = T.position(position[0], position[1], position[2]) {
        name = name + "_display"
        Joint {}
        content
    }
    let anchor = T { name = name + "_anchor" Joint {} display }
    let rig = Armature.joints(anchor).capture_rest_pose() {
        SecondaryMotion {
            SpringBone.new(name + "_spring")
                .allow_stretch(true).world_collision(false).avatar_collision(false) {
                name = name + "_spring"
                SpringJoint.new(anchor).stiffness(10.0).drag_force(0.65).gravity(0.0, [0.0, -1.0, 0.0])
                SpringJoint.new(display).stiffness(10.0).drag_force(0.65).gravity(0.0, [0.0, -1.0, 0.0])
            }
        }
    }
    return T {
        name = name
        Raycastable.disabled()
        Selectable.off()
        TOP.fork_trs().drop("scale") {
            T { anchor rig }
        }
    }
}
