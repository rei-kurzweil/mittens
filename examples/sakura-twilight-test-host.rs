//! A renderer-free Sakura Twilight peer for measuring XR controls in either role.
use meow_meow_script::{TransportValue, TypeTag};
use mittens_engine::{
    engine::{
        ecs::{
            ComponentId, EventSignal, RxWorld, World,
            component::MultiplayerSessionComponent,
            system::{MultiplayerSystem, udp_channel_system::transform_schema},
        },
        transform::TransformTrs,
        udp::UdpSystem,
        udp_channels::{ChannelSocket, PublicationId, Schema, SubscriptionId},
    },
    utils::math::quat_from_axis_angle,
};
use std::{
    collections::VecDeque,
    error::Error,
    net::{Ipv4Addr, SocketAddr},
    thread,
    time::{Duration, Instant},
};

type Result<T> = std::result::Result<T, Box<dyn Error>>;
const SAMPLE_PERIOD: Duration = Duration::from_millis(50);
const POLL_PERIOD: Duration = Duration::from_millis(4);
const POSES: [&str; 4] = ["root", "head", "left-hand", "right-hand"];
const SCALARS: [&str; 5] = [
    "blink.left",
    "blink.right",
    "left-hand.valid",
    "right-hand.valid",
    "avatar.flight",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Session,
    Still,
    Motion,
}

#[derive(Clone, Copy)]
struct PoseSample {
    ready_at: Instant,
    poses: [TransformTrs; 4],
    blink: f32,
}

impl PoseSample {
    fn new(now: Instant, delay: Duration, t: f32) -> Self {
        let wave = (1.9 * t).sin();
        let orbit = 0.65 * t;
        let wrist = |side: f32| {
            TransformTrs::new(
                [
                    side * (0.36 + 0.06 * wave),
                    -0.32 + side * 0.14 * wave,
                    -0.38,
                ],
                quat_from_axis_angle([0.0, 0.0, 1.0], side * (0.2 + 0.4 * wave)),
                [1.0; 3],
            )
        };
        Self {
            ready_at: now + delay,
            poses: [
                TransformTrs::new(
                    [
                        -22.5 + 2.1 * orbit.sin(),
                        12.6 + (0.95 * t).sin(),
                        -1.5 + 2.5 * orbit.cos(),
                    ],
                    quat_from_axis_angle([0.0, 1.0, 0.0], 0.45 * orbit.sin()),
                    [1.0; 3],
                ),
                TransformTrs::new(
                    [0.07 * wave, 1.64 + 0.08 * wave, 0.06 * (1.1 * t).sin()],
                    quat_from_axis_angle([1.0, 0.0, 0.0], 0.25 * wave),
                    [1.0; 3],
                ),
                wrist(-1.0),
                wrist(1.0),
            ],
            blink: ((t % 3.2 - 2.8).abs().mul_add(-12.0, 1.0)).max(0.0),
        }
    }
}

struct Peer {
    id: u32,
    pose_channels: Vec<PublicationId>,
    scalar_channels: Vec<PublicationId>,
    avatar_channel: Option<PublicationId>,
    subscriptions: Vec<SubscriptionId>,
    started: Instant,
    next_sample: Instant,
    announced_avatar: bool,
    last_flight: Option<Instant>,
    pending: VecDeque<PoseSample>,
}

impl Peer {
    fn connect(
        socket: &mut ChannelSocket,
        id: u32,
        address: SocketAddr,
        mode: Mode,
        now: Instant,
    ) -> Result<Self> {
        let mut peer = Self {
            id,
            pose_channels: Vec::new(),
            scalar_channels: Vec::new(),
            avatar_channel: None,
            subscriptions: Vec::new(),
            started: now,
            next_sample: now,
            announced_avatar: false,
            last_flight: None,
            pending: VecDeque::new(),
        };
        // Acknowledge the headset's streams in all three modes, so it still
        // does its usual outgoing pose work in the control run.
        let channels = POSES
            .map(|name| (name, transform_schema()))
            .into_iter()
            .chain(SCALARS.map(|name| (name, Schema::primitive(TypeTag::F32))))
            .chain([("avatar", Schema::primitive(TypeTag::Str))]);
        for (i, (name, schema)) in channels.enumerate() {
            peer.subscriptions
                .push(socket.subscribe(address, name, schema.clone())?);
            if mode == Mode::Session {
                continue;
            }
            let publication = socket.publish(address, name, schema)?;
            if i < POSES.len() {
                peer.pose_channels.push(publication);
            } else if name == "avatar" {
                peer.avatar_channel = Some(publication);
            } else {
                peer.scalar_channels.push(publication);
            }
        }
        Ok(peer)
    }

    fn disconnect(self, socket: &mut ChannelSocket) -> Result<()> {
        for id in self
            .pose_channels
            .into_iter()
            .chain(self.scalar_channels)
            .chain(self.avatar_channel)
        {
            socket.remove_publication(id)?;
        }
        for id in self.subscriptions {
            socket.remove_subscription(id)?;
        }
        Ok(())
    }

    fn tick(
        &mut self,
        socket: &mut ChannelSocket,
        mode: Mode,
        now: Instant,
        delay: Duration,
    ) -> Result<()> {
        if mode == Mode::Session {
            return Ok(());
        }
        if now >= self.next_sample {
            let t = if mode == Mode::Motion {
                now.duration_since(self.started).as_secs_f32()
            } else {
                0.0
            };
            self.pending.push_back(PoseSample::new(now, delay, t));
            self.next_sample += SAMPLE_PERIOD;
            if self.next_sample <= now {
                self.next_sample = now + SAMPLE_PERIOD;
            }
        }
        while self
            .pending
            .front()
            .is_some_and(|sample| sample.ready_at <= now)
        {
            let sample = self.pending.pop_front().unwrap();
            for (&id, pose) in self.pose_channels.iter().zip(sample.poses) {
                let [x, y, z, w] = pose.rotation_quat_xyzw;
                let [tx, ty, tz] = pose.translation;
                write_numeric(socket, id, &[x, y, z, w, 1.0, 1.0, 1.0, tx, ty, tz])?;
            }
            let [left_blink, right_blink, left_valid, right_valid, flight] =
                self.scalar_channels[..]
            else {
                return Err("unexpected avatar channel layout".into());
            };
            for (id, value) in [
                (left_blink, sample.blink),
                (right_blink, sample.blink),
                (left_valid, 1.0),
                (right_valid, 1.0),
            ] {
                write_numeric(socket, id, &[value])?;
            }
            if self
                .last_flight
                .is_none_or(|last| now.duration_since(last) >= Duration::from_secs(1))
                && socket.is_publication_ready(flight)
                && socket.write_numeric(flight, &[1.0])?
            {
                self.last_flight = Some(now);
            }
            if let Some(id) = self.avatar_channel
                && !self.announced_avatar
                && socket.is_publication_ready(id)
            {
                self.announced_avatar =
                    socket.write(id, &TransportValue::String("mitten".into()))?;
            }
        }
        Ok(())
    }
}

fn write_numeric(socket: &mut ChannelSocket, id: PublicationId, values: &[f32]) -> Result<()> {
    if socket.is_publication_ready(id) {
        socket.write_numeric(id, values)?;
    }
    Ok(())
}

struct TestPeer {
    world: World,
    session: ComponentId,
    multiplayer: MultiplayerSystem,
    inbox: RxWorld,
    socket: ChannelSocket,
    peer: Option<Peer>,
    mode: Mode,
    delay: Duration,
    remote_data_port: u16,
    last_poll: Instant,
}

impl TestPeer {
    fn bind(
        control: MultiplayerSessionComponent,
        data: SocketAddr,
        remote_data_port: u16,
        mode: Mode,
        delay: Duration,
    ) -> Result<Self> {
        let udp = UdpSystem::default();
        let socket = ChannelSocket::bind_on(&udp, data)?;
        let mut world = World::default();
        let session = world.add_component(control);
        let mut multiplayer = MultiplayerSystem::default();
        let mut inbox = RxWorld::default();
        multiplayer.register_session_on(&mut world, &mut inbox, session, &udp);
        multiplayer.tick(&mut world, &mut inbox, 0.0);
        if let Some(message) =
            inbox
                .drain_ready_events()
                .into_iter()
                .find_map(|signal| match signal.event {
                    Some(EventSignal::MultiplayerError { message, .. }) => Some(message),
                    _ => None,
                })
        {
            return Err(message.into());
        }
        Ok(Self {
            world,
            session,
            multiplayer,
            inbox,
            socket,
            mode,
            delay,
            remote_data_port,
            peer: None,
            last_poll: Instant::now(),
        })
    }

    fn tick(&mut self, now: Instant) -> Result<()> {
        let dt = now.duration_since(self.last_poll).as_secs_f32();
        self.last_poll = now;
        self.multiplayer.tick(&mut self.world, &mut self.inbox, dt);
        for signal in self.inbox.drain_ready_events() {
            match signal.event {
                Some(EventSignal::MultiplayerPeerJoined { peer_id, name })
                    if self.peer.is_none() =>
                {
                    let control = self
                        .world
                        .get_component_by_id_as::<MultiplayerSessionComponent>(self.session)
                        .unwrap();
                    let address =
                        SocketAddr::new(control.peer_ips[&peer_id].parse()?, self.remote_data_port);
                    self.peer = Some(Peer::connect(
                        &mut self.socket,
                        peer_id,
                        address,
                        self.mode,
                        now,
                    )?);
                    eprintln!("Connected to {name}, typed UDP to {address}");
                }
                Some(EventSignal::MultiplayerPeerLeft { peer_id, .. })
                    if self.peer.as_ref().is_some_and(|peer| peer.id == peer_id) =>
                {
                    self.peer.take().unwrap().disconnect(&mut self.socket)?;
                    eprintln!("Peer left");
                }
                Some(EventSignal::MultiplayerError { phase, message }) => {
                    eprintln!("Multiplayer {phase}: {message}");
                }
                _ => {}
            }
        }
        self.socket.poll(now)?;
        self.socket.drain_events().for_each(drop);
        if let Some(peer) = &mut self.peer {
            peer.tick(&mut self.socket, self.mode, now, self.delay)?;
        }
        Ok(())
    }
}

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1).peekable();
    let client = args.peek().is_some_and(|arg| arg == "--client");
    if client {
        args.next();
    }
    let mode = match args.next().as_deref() {
        Some("session") => Mode::Session,
        Some("still") => Mode::Still,
        Some("motion") => Mode::Motion,
        _ => {
            return Err(
                "usage: sakura-twilight-test-host [--client] <session|still|motion> [delay-ms] [IPv4 address]"
                    .into(),
            );
        }
    };
    let delay_ms: u64 = args.next().as_deref().unwrap_or("0").parse()?;
    let ip: Ipv4Addr = args.next().as_deref().unwrap_or("127.0.0.1").parse()?;
    if delay_ms > 2000 || args.next().is_some() {
        return Err("expected 0–2000 ms delay and an optional IPv4 address".into());
    }
    let control = SocketAddr::from((ip, 7010));
    let (session, data, remote_data_port) = if client {
        eprintln!("Test client {mode:?} joining {control}, {delay_ms} ms outgoing pose delay");
        let bind_ip = if ip.is_loopback() {
            Ipv4Addr::LOCALHOST
        } else {
            Ipv4Addr::UNSPECIFIED
        };
        (
            MultiplayerSessionComponent::join(control.to_string()).with_player_name("Test client"),
            (bind_ip, 7012).into(),
            7011,
        )
    } else {
        eprintln!("Test host {mode:?} listening on {control}, {delay_ms} ms outgoing pose delay");
        (
            MultiplayerSessionComponent::host(control.to_string()).with_player_name("Test host"),
            (ip, 7011).into(),
            7012,
        )
    };
    let mut peer = TestPeer::bind(
        session,
        data,
        remote_data_port,
        mode,
        Duration::from_millis(delay_ms),
    )?;
    loop {
        peer.tick(Instant::now())?;
        thread::sleep(POLL_PERIOD);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mittens_engine::engine::{
        Universe,
        ecs::{
            component::MultiplayerRole,
            system::{
                TransformSystem, external_xr_frame::ExternalXrFrame, vr_types::XrGamepadState,
            },
        },
    };
    use std::net::UdpSocket;

    struct Run {
        displacement: f32,
        remote_height: f32,
        received: u64,
    }

    fn run(local_role: MultiplayerRole, mode: Mode, delay: Duration) -> Run {
        let reservations = [(); 3].map(|_| UdpSocket::bind("127.0.0.1:0").unwrap());
        let [control, host_data, client_data] = reservations
            .each_ref()
            .map(|socket| socket.local_addr().unwrap());
        drop(reservations);
        let (local_constructor, remote_session, remote_data, local_data_port) = match local_role {
            MultiplayerRole::Host => (
                "host",
                MultiplayerSessionComponent::join(control.to_string()),
                client_data,
                host_data.port(),
            ),
            MultiplayerRole::Client => (
                "join",
                MultiplayerSessionComponent::host(control.to_string()),
                host_data,
                client_data.port(),
            ),
        };
        let path = "examples/sakura-twilight.mms";
        let source = std::fs::read_to_string(path)
            .unwrap()
            .replace(
                "MultiplayerSession.from_env()",
                &format!("MultiplayerSession.{local_constructor}(\"{control}\")"),
            )
            .replace("7011", &host_data.port().to_string())
            .replace("7012", &client_data.port().to_string());
        let mut synthetic =
            TestPeer::bind(remote_session, remote_data, local_data_port, mode, delay).unwrap();
        let mut scene = Universe::new(World::default());
        let identity = openxr::Quaternionf {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            w: 1.0,
        };
        let views = [-0.032, 0.032].map(|x| openxr::View {
            pose: openxr::Posef {
                orientation: identity,
                position: openxr::Vector3f { x, y: 1.64, z: 0.0 },
            },
            fov: openxr::Fovf {
                angle_left: -0.7,
                angle_right: 0.7,
                angle_up: 0.7,
                angle_down: -0.7,
            },
        });
        let mut tracking = ExternalXrFrame {
            views: Some(views),
            gamepad: XrGamepadState {
                active: true,
                head_pose_rotation: Some([0.0, 0.0, 0.0, 1.0]),
                ..Default::default()
            },
            ..Default::default()
        };
        scene.systems.xr.set_external_frame(Some(tracking));
        scene.load_mms_source_at_path(&source, path).unwrap();
        scene.systems.set_vr_perf_enabled(true);
        let root = |scene: &Universe, label: &str| {
            scene
                .world
                .all_components()
                .find(|&id| scene.world.component_label(id) == Some(label))
                .unwrap()
        };
        let local = root(&scene, "local_root");
        let remote = root(&scene, "remote_root");
        let frame_interval = Duration::from_secs_f64(1.0 / 90.0);
        let mut network = Vec::new();
        let mut received = 0;
        let mut initial_z = 0.0;
        for i in 0..145 {
            let frame = Instant::now();
            if i == 55 {
                initial_z = TransformSystem::world_position(&scene.world, local).unwrap()[2];
                tracking.gamepad.hands[0].thumbstick = Some([0.0, 0.70]);
                scene.systems.xr.set_external_frame(Some(tracking));
            }
            synthetic.tick(frame).unwrap();
            scene.update(frame_interval.as_secs_f32(), &Default::default());
            if i >= 55 {
                let timings = scene.systems.vr_perf_pre_xr();
                network.push(timings.network.as_secs_f64() * 1_000.0);
                received += timings.typed_datagrams;
            }
            // The warm-up creates pose subscriptions. No delayed pose should
            // drive the receiving scene before the configured hold has elapsed.
            if mode == Mode::Motion
                && let Some(peer) = &synthetic.peer
                && frame.duration_since(peer.started) < delay
            {
                let height = TransformSystem::world_position(&scene.world, remote).unwrap()[1];
                assert!(height < 11.0);
            }
            thread::sleep(frame_interval.saturating_sub(frame.elapsed()));
        }
        assert!(synthetic.peer.is_some());
        network.sort_by(f64::total_cmp);
        let mean = network.iter().sum::<f64>() / network.len() as f64;
        let p95 = network[(network.len() * 95 / 100).min(network.len() - 1)];
        eprintln!(
            "XR {local_role:?}, synthetic {mode:?} with {delay:?} leg: {received} typed datagrams, network mean {mean:.3} ms, p95 {p95:.3} ms"
        );
        Run {
            displacement: initial_z
                - TransformSystem::world_position(&scene.world, local).unwrap()[2],
            remote_height: TransformSystem::world_position(&scene.world, remote).unwrap()[1],
            received,
        }
    }

    #[test]
    fn authored_host_and_client_move_locally_under_delayed_peer_poses() {
        let delay = Duration::from_millis(80);
        let session = run(MultiplayerRole::Client, Mode::Session, Duration::ZERO);
        let client = run(MultiplayerRole::Client, Mode::Motion, delay);
        let host = run(MultiplayerRole::Host, Mode::Motion, delay);
        assert!(session.remote_height < 11.0);
        assert!(session.displacement > 0.5);
        for run in [client, host] {
            assert!(run.remote_height > 11.0);
            assert!(run.received > session.received + 40);
            assert!((session.displacement - run.displacement).abs() < 0.05);
        }
    }
}
