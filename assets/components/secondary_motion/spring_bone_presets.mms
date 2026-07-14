// Reusable heuristic spring-chain presets. Passing false uses the generic
// selectors; when a config table is supplied, all selector fields for that
// preset are required. Physical settings deliberately remain in this library.

export fn soft_hair_chain(config) {
    let root = "[name='Hair1']"
    let middle = "[name='Hair2']"
    let tip = "[name='Hair3']"
    if config {
        root = config.root
        middle = config.middle
        tip = config.tip
    }
    return SpringBone.new(root).virtual_end_length_ratio(1.0) {
        SpringJoint.new(root).stiffness(1.0).drag_force(0.35).gravity(3.0, [0, -1, 0])
        SpringJoint.new(middle).stiffness(1.0).drag_force(0.35).gravity(3.0, [0, -1, 0])
        SpringJoint.new(tip).stiffness(1.0).drag_force(0.35).gravity(3.0, [0, -1, 0])
    }
}

export fn soft_hair_chain_4(config) {
    let root = "[name='Hair1']"
    let middle = "[name='Hair2']"
    let tip = "[name='Hair3']"
    let end = "[name='Hair4']"
    if config {
        root = config.root
        middle = config.middle
        tip = config.tip
        end = config.end
    }
    return SpringBone.new(root).virtual_end_length_ratio(1.0) {
        SpringJoint.new(root).stiffness(1.0).drag_force(0.35).gravity(3.0, [0, -1, 0])
        SpringJoint.new(middle).stiffness(1.0).drag_force(0.35).gravity(3.0, [0, -1, 0])
        SpringJoint.new(tip).stiffness(1.0).drag_force(0.35).gravity(3.0, [0, -1, 0])
        SpringJoint.new(end).stiffness(1.0).drag_force(0.35).gravity(3.0, [0, -1, 0])
    }
}

export fn soft_bust_chain(config) {
    let root = "[name='Bust1']"
    let tip = "[name='Bust2']"
    if config {
        root = config.root
        tip = config.tip
    }
    return SpringBone.new(root).virtual_end_length_ratio(1.0) {
        SpringJoint.new(root).stiffness(2.0).drag_force(0.60).gravity(1.0, [0, -1, 0])
        SpringJoint.new(tip).stiffness(2.0).drag_force(0.60).gravity(1.0, [0, -1, 0])
    }
}
