import { f32 } from "types"

let curl_type = { trigger: f32, grip: f32 }

fn make_hand(side, driver) {
    return {
        side = side driver = driver outgoing = null
        trigger = 0.0 grip = 0.0 sent_trigger = 0.0 sent_grip = 0.0 elapsed = 0.0 sent = false
        remote_trigger = 0.0 remote_grip = 0.0 index = [] fingers = []
    }
}

fn apply_finger(joints, curl) {
    // VRoid fingers extend along local +Y and flex toward local -Z.
    let angles = [1.05, 1.25, 0.75]
    for i in range(3) {
        let bone = joints[i]
        if bone { bone.rest_relative_rotation([-angles[i] * curl, 0.0, 0.0]) }
    }
}

fn apply_curls(hand) {
    for joints in hand.index { apply_finger(joints, hand.remote_trigger) }
    for joints in hand.fingers { apply_finger(joints, hand.remote_grip) }
}

fn finger_joints(model, side, finger) {
    let prefix = "J_Bip_R_"
    if side == "Left" { prefix = "J_Bip_L_" }
    let name = prefix + finger
    return [
        model.query("[name='" + name + "1']"),
        model.query("[name='" + name + "2']"),
        model.query("[name='" + name + "3']"),
    ]
}

fn send(hand, elapsed) {
    if !hand.outgoing || !hand.outgoing.ready() { return }
    let trigger = 0.0
    let grip = 0.0
    if hand.driver.pose_valid() {
        trigger = hand.trigger
        grip = hand.grip
    }
    // An initially open hand can stay silent. Once used, refresh the current
    // state so a dropped release packet cannot leave it curled indefinitely.
    if !hand.sent && trigger == 0.0 && grip == 0.0 { return }
    hand.elapsed = hand.elapsed + elapsed
    if trigger == hand.sent_trigger && grip == hand.sent_grip && hand.elapsed < 1.0 { return }
    hand.outgoing.write({ trigger = trigger grip = grip })
    hand.sent_trigger = trigger
    hand.sent_grip = grip
    hand.elapsed = 0.0
    hand.sent = true
}

export fn controller_hand_curls(options) {
    let left = make_hand("Left", options.left_driver)
    let right = make_hand("Right", options.right_driver)
    let hands = [left, right]
    let binding = { model = null }
    if options.xr {
        on(options.input, "XrAxisChanged", fn(event) {
            let hand = left
            if event.hand == "Right" { hand = right }
            let value = Math.round(Math.clamp(event.value[0], 0.0, 1.0) * 32.0) / 32.0
            if event.control == hand.side + "Trigger" { hand.trigger = value }
            if event.control == hand.side + "Grip" { hand.grip = value }
        })
    }

    fn connect_hand(hand, group, address) {
        let name = "controller-curls." + hand.side
        if options.xr {
            hand.outgoing = options.socket.publish(name).to(address).type(curl_type)
            group.attach(hand.outgoing)
        }
        let incoming = options.socket.subscribe(name).from(address).type(curl_type)
        group.attach(incoming)
        on(incoming, "UdpSample", fn(sample) {
            let trigger = Math.clamp(sample.value.trigger, 0.0, 1.0)
            let grip = Math.clamp(sample.value.grip, 0.0, 1.0)
            if trigger == hand.remote_trigger && grip == hand.remote_grip { return }
            hand.remote_trigger = trigger
            hand.remote_grip = grip
            apply_curls(hand)
        })
    }

    fn reset(hand) {
        hand.outgoing = null
        hand.sent = false
        hand.sent_trigger = 0.0
        hand.sent_grip = 0.0
        hand.elapsed = 0.0
        if hand.remote_trigger != 0.0 || hand.remote_grip != 0.0 {
            hand.remote_trigger = 0.0
            hand.remote_grip = 0.0
            apply_curls(hand)
        }
    }

    fn disconnect() { for hand in hands { reset(hand) } }
    fn connect(group, address) {
        disconnect()
        for hand in hands { connect_hand(hand, group, address) }
    }

    fn bind_model(model, choice) {
        if binding.model { binding.model.remove() binding.model = null }
        for hand in hands { hand.index = [] hand.fingers = [] }
        if choice == "miku" { return }
        fn bind_joints() {
            for hand in hands {
                hand.index = [finger_joints(model, hand.side, "Index")]
                hand.fingers = [
                    finger_joints(model, hand.side, "Middle"),
                    finger_joints(model, hand.side, "Ring"),
                    finger_joints(model, hand.side, "Little"),
                ]
                if hand.remote_trigger != 0.0 || hand.remote_grip != 0.0 { apply_curls(hand) }
            }
        }
        // A replacement avatar can be attached before its GLTF joints exist.
        binding.model = on(model, "GLTFInitialized", fn(event) { bind_joints() })
        bind_joints()
    }

    fn tick(elapsed) { for hand in hands { send(hand, elapsed) } }
    return { connect = connect disconnect = disconnect bind_model = bind_model tick = tick }
}
