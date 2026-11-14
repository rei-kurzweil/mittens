//! All scene protocol families share one transport and release their leases.
use crate::engine::ecs::component::{HTCEyeTrackingComponent, VRChatOSCEyeTrackingComponent};
use crate::engine::user_input::{
    TextInputFrameEvent,
    udp_text::{UdpTerminalHost, UdpTextInputSender},
};
use crate::engine::{Universe, ecs::World};

#[test]
fn shared_udp_scene_protocols_and_terminal_use_one_owner() {
    let mut scene = Universe::new(World::default());
    scene
        .load_mms_source_at_path(
            r#"
XR.off()
MultiplayerSession.host("127.0.0.1:0") {}
UdpSocket.bind("127.0.0.1:0") {}
TextInput { name = "terminal" }
"#,
            "examples/_shared-udp-test.mms",
        )
        .unwrap();
    let osc = scene
        .world
        .add_component(VRChatOSCEyeTrackingComponent::listen("127.0.0.1", 0));
    let htc = scene
        .world
        .add_component(HTCEyeTrackingComponent::listen("127.0.0.1", 0));
    scene
        .systems
        .xr_eye_tracking
        .register_component(&scene.world, osc);
    scene
        .systems
        .xr_eye_tracking
        .register_component(&scene.world, htc);
    scene.update(1. / 60., &Default::default());
    let udp = scene.systems.udp.clone();
    let endpoints = udp.diagnostics();
    let address = |label| {
        endpoints
            .iter()
            .find(|e| e.label == label)
            .unwrap()
            .local_addr
    };
    let external = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
    let mut osc_packet = Vec::new();
    for text in ["/tracking/eye/EyesClosedAmount", ",f"] {
        osc_packet.extend_from_slice(text.as_bytes());
        osc_packet.push(0);
        while osc_packet.len() % 4 != 0 {
            osc_packet.push(0);
        }
    }
    osc_packet.extend_from_slice(&0.6f32.to_be_bytes());
    external.send_to(&osc_packet, address("eye-osc")).unwrap();
    let mut htc_packet = [0u8; 84];
    htc_packet[0] = b'M';
    htc_packet[1] = 1;
    htc_packet[2] = 0b1100;
    htc_packet[36..40].copy_from_slice(&0.8f32.to_le_bytes());
    htc_packet[48..52].copy_from_slice(&0.25f32.to_le_bytes());
    external.send_to(&htc_packet, address("eye-htc")).unwrap();
    scene.update(1. / 60., &Default::default());
    let osc_value = scene
        .world
        .get_component_by_id_as::<VRChatOSCEyeTrackingComponent>(osc)
        .unwrap()
        .closure_sample;
    let htc_value = scene
        .world
        .get_component_by_id_as::<HTCEyeTrackingComponent>(htc)
        .unwrap()
        .closure_sample;
    assert!((osc_value.left.unwrap() - 0.6).abs() < 1e-6);
    assert!((htc_value.left.unwrap() - 0.2).abs() < 1e-6);
    assert!((htc_value.right.unwrap() - 0.75).abs() < 1e-6);

    let terminal =
        UdpTerminalHost::install(&mut scene, "127.0.0.1:0".parse().unwrap(), "terminal").unwrap();
    let terminal_address = udp
        .diagnostics()
        .into_iter()
        .find(|e| e.label == "terminal-input")
        .unwrap()
        .local_addr;
    let mut sender = UdpTextInputSender::new_on(&udp, terminal_address).unwrap();
    sender.set_enabled(true);
    sender
        .enqueue(&[TextInputFrameEvent::InsertText("shared transport".into())])
        .unwrap();
    let mut inserts = 0;
    for _ in 0..20 {
        sender.poll().unwrap();
        let input = scene.input_providers.poll();
        inserts+=input.text_input_events().iter().filter(|event|matches!(event,TextInputFrameEvent::InsertText(text) if text=="shared transport")).count();
        terminal.prepare(&mut scene, &input);
        scene.update(1. / 60., &input);
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert_eq!(inserts, 1);
    let endpoints = udp.diagnostics();
    for label in [
        "multiplayer",
        "typed-channels",
        "eye-osc",
        "eye-htc",
        "terminal-input",
        "terminal-sender",
    ] {
        let endpoint = endpoints
            .iter()
            .find(|e| e.label == label)
            .unwrap_or_else(|| panic!("missing {label}: {endpoints:?}"));
        assert!(
            endpoint.stats.polls > 0,
            "{label} did not use shared polling"
        );
    }
    assert_eq!(endpoints.len(), 6);
    for id in [osc, htc] {
        scene
            .systems
            .remove_subtree_immediate(&mut scene.world, &mut scene.visuals, id);
    }
    assert!(
        udp.diagnostics()
            .iter()
            .all(|e| !e.label.starts_with("eye-"))
    );
    drop(scene);
    assert_eq!(
        udp.diagnostics().len(),
        1,
        "only standalone sender lease should remain"
    );
    drop(sender);
    assert!(udp.diagnostics().is_empty());
}
