use super::*;
use crate::engine::user_input::{HidProvider, TextInputFrameEvent as E};
use std::{
    net::UdpSocket,
    time::{Duration, Instant},
};

#[test]
fn duplicate_and_gap_packets_only_deliver_one_ordered_edit_stream() {
    let mut receiver = UdpTextInputProvider::bind("127.0.0.1:0".parse().unwrap()).unwrap();
    let address = receiver.local_addr().unwrap();
    let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
    let session = uuid::Uuid::new_v4();
    Packet::new(session, 0, vec![])
        .send(&socket, address)
        .unwrap();
    receiver.poll();
    Packet::new(session, 2, vec![Edit::Insert("late".into())])
        .send(&socket, address)
        .unwrap();
    assert!(receiver.poll().text_input_events().is_empty());
    let first = Packet::new(
        session,
        1,
        vec![Edit::Insert("猫".into()), Edit::Left, Edit::Backspace],
    );
    first.send(&socket, address).unwrap();
    first.send(&socket, address).unwrap();
    assert_eq!(
        receiver.poll().text_input_events(),
        &[E::InsertText("猫".into()), E::MoveCaretLeft, E::Backspace]
    );
    assert!(receiver.poll().text_input_events().is_empty());
    assert!(Packet::decode(b"not JSON").is_none());
    assert!(Packet::decode(&vec![b' '; MAX_PACKET + 1]).is_none());
}
#[test]
fn sender_handshakes_and_preserves_utf8_paste_in_bounded_packets() {
    let mut receiver = UdpTextInputProvider::bind("127.0.0.1:0".parse().unwrap()).unwrap();
    let mut sender = UdpTextInputSender::new(receiver.local_addr().unwrap()).unwrap();
    sender.set_enabled(true);
    let expected = "猫とミトン".repeat(30);
    sender.enqueue(&[E::InsertText(expected.clone())]).unwrap();
    let mut received = String::new();
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(2) {
        sender.poll().unwrap();
        for event in receiver.poll().text_input_events() {
            if let E::InsertText(text) = event {
                received.push_str(text);
            }
        }
        if received == expected {
            break;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(received, expected);
    assert!(sender.is_ready());
    sender.set_enabled(false);
    assert!(!sender.is_ready());
}
#[test]
fn udp_edits_use_normal_text_input_after_browser_focus_loss() {
    use crate::engine::{
        Universe,
        ecs::{World, component::TextInputComponent},
    };
    let mut scene = Universe::new(World::default());
    scene
        .load_mms_source_at_path(
            "T { TextInput { name = \"terminal\" \"\" } }",
            "examples/udp-terminal-test.mms",
        )
        .unwrap();
    for _ in 0..2 {
        scene.update(0.01, &Default::default());
    }
    // The provider and host helper are independent of Android and the MMS shell.
    let mut provider = UdpTextInputProvider::bind("127.0.0.1:0".parse().unwrap()).unwrap();
    let target = scene
        .world
        .all_components()
        .find(|&id| scene.world.component_label(id) == Some("terminal"))
        .unwrap();
    let host = super::host::UdpTerminalHost::for_target(target);
    let mut sender = UdpTextInputSender::new(provider.local_addr().unwrap()).unwrap();
    sender.set_enabled(true);
    sender
        .enqueue(&[
            E::InsertText("abc".into()),
            E::MoveCaretLeft,
            E::Backspace,
            E::InsertText("Z".into()),
        ])
        .unwrap();
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(2) {
        sender.poll().unwrap();
        let input = provider.poll();
        host.prepare(&mut scene, &input);
        scene.update(0.01, &input);
        if scene
            .world
            .get_component_by_id_as::<TextInputComponent>(target)
            .unwrap()
            .text
            == "aZc"
        {
            return;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    panic!("UDP edits did not reach TextInput");
}
