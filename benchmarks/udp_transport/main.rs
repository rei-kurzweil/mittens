//! Built by scripts/benchmark_udp_transport.sh with cached, locked dependencies.
#![allow(dead_code)]
mod avatars;
mod eye;
mod math;
mod protocol;
mod transport;
mod udp;
mod udp_channels;
mod workload;
use std::{
    hint::black_box,
    io,
    net::UdpSocket,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    thread,
    time::{Duration, Instant},
};
use transport::{Mode, Receivers, Transport};
use workload::{Kind, Workload};

fn cpu_ns() -> u64 {
    let mut t = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    assert_eq!(
        unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &mut t) },
        0
    );
    t.tv_sec as u64 * 1_000_000_000 + t.tv_nsec as u64
}
fn sleep_until(deadline: Instant) {
    if let Some(delay) = deadline.checked_duration_since(Instant::now()) {
        thread::sleep(delay);
    }
}
fn median(sorted: &[f64]) -> f64 {
    if sorted.is_empty() {
        return f64::NAN;
    }
    let middle = sorted.len() / 2;
    if sorted.len() % 2 == 0 {
        (sorted[middle - 1] + sorted[middle]) / 2.
    } else {
        sorted[middle]
    }
}
fn percentile(sorted: &[f64], quantile: f64) -> f64 {
    if sorted.is_empty() {
        return f64::NAN;
    }
    sorted[((sorted.len() - 1) as f64 * quantile).round() as usize]
}
struct Config {
    pairs: Vec<usize>,
    avatars: Option<String>,
    seconds: f64,
    trials: usize,
    rounds: usize,
    scale: usize,
    poll_hz: f64,
    poses: bool,
    modes: Vec<Mode>,
    replay_only: bool,
}
impl Config {
    fn parse() -> Self {
        let mut c = Self {
            pairs: vec![1, 8],
            avatars: None,
            seconds: 1.0,
            trials: 5,
            rounds: 500,
            scale: 1,
            poll_hz: 90.,
            poses: false,
            modes: vec![Mode::Separate, Mode::Shared],
            replay_only: false,
        };
        let mut args = std::env::args().skip(1);
        while let Some(arg) = args.next() {
            if arg == "--replay-only" {
                c.replay_only = true;
                continue;
            }
            if arg == "--poses" {
                c.poses = true;
                continue;
            }
            if arg == "--help" {
                println!(
                    "--avatars vr2|vr4|mixed4 (optional) --pairs 1,8 --seconds 1 --trials 5 --rounds 500 --rate-scale 1 --poll-hz 90 --mode both|all|separate|shared|runtime --poses --replay-only"
                );
                std::process::exit(0);
            }
            let value = args
                .next()
                .unwrap_or_else(|| panic!("missing value for {arg}"));
            match arg.as_str() {
                "--avatars" => {
                    c.pairs = vec![if value == "vr2" { 2 } else { 4 }];
                    c.avatars = Some(value);
                }
                "--pairs" => c.pairs = value.split(',').map(|v| v.parse().unwrap()).collect(),
                "--seconds" => c.seconds = value.parse().unwrap(),
                "--trials" => c.trials = value.parse().unwrap(),
                "--rounds" => c.rounds = value.parse().unwrap(),
                "--rate-scale" => c.scale = value.parse().unwrap(),
                "--poll-hz" => c.poll_hz = value.parse().unwrap(),
                "--mode" => {
                    c.modes = match value.as_str() {
                        "both" => vec![Mode::Separate, Mode::Shared],
                        "separate" => vec![Mode::Separate],
                        "shared" => vec![Mode::Shared],
                        "runtime" => vec![Mode::Runtime],
                        "all" => vec![Mode::Separate, Mode::Shared, Mode::Runtime],
                        _ => panic!("unknown mode"),
                    }
                }
                _ => panic!("unknown argument {arg}"),
            }
        }
        assert!(c.pairs.iter().all(|p| (1..=128).contains(p)) && !c.pairs.is_empty());
        assert!(c.seconds.is_finite() && c.seconds > 0. && c.seconds <= 30.);
        assert!(c.poll_hz.is_finite() && c.poll_hz > 0. && c.poll_hz <= 10_000.);
        assert!(c.trials > 0 && c.rounds > 0 && (1..=100).contains(&c.scale));
        if let Some(case) = &c.avatars {
            assert!(
                !c.poses,
                "--poses describes the legacy JSON workload, not typed avatars"
            );
            c.pairs = vec![match case.as_str() {
                "vr2" => 2,
                "vr4" | "mixed4" => 4,
                _ => panic!("unknown avatar scenario"),
            }];
        }
        c
    }
}
fn replay(mode: Mode, work: &Workload, rounds: usize) -> (f64, usize, u64) {
    let mut transport = Transport::new(mode, &work.kinds);
    let bytes = transport.scratch_bytes();
    let mut checksum = 0u64;
    // Warm decoder, input payloads, and scratch buffers before measurement.
    let mut expected = 0u64;
    for event in &work.events {
        let (seq, value) = black_box(
            transport
                .replay(event.route, event.kind, &event.bytes)
                .unwrap(),
        );
        expected = expected.wrapping_add(value).wrapping_add(seq as u64);
    }
    let started = Instant::now();
    for _ in 0..rounds {
        for event in &work.events {
            let (seq, value) = transport
                .replay(event.route, event.kind, black_box(&event.bytes))
                .unwrap();
            checksum = checksum.wrapping_add(value).wrapping_add(seq as u64);
        }
    }
    black_box(checksum);
    (
        started.elapsed().as_nanos() as f64 / (rounds * work.events.len()) as f64,
        bytes,
        expected,
    )
}
struct UdpResult {
    cpu_ms: f64,
    wall_s: f64,
    sent: usize,
    eye: usize,
    session: usize,
    avatar: usize,
    invalid: usize,
    duplicate: usize,
    send_errors: usize,
    latency: Vec<f64>,
    eye_latency: Vec<f64>,
    session_latency: Vec<f64>,
    checksum: u64,
}
fn loopback(mode: Mode, work: Arc<Workload>, poll_hz: f64) -> io::Result<UdpResult> {
    let receivers = Receivers::bind(mode, &work.kinds)?;
    let addresses = receivers.addresses()?;
    let sender = UdpSocket::bind("127.0.0.1:0")?;
    sender.set_nonblocking(true)?;
    let timestamps = Arc::new(
        (0..work.events.len())
            .map(|_| AtomicU64::new(0))
            .collect::<Vec<_>>(),
    );
    let producer_work = Arc::clone(&work);
    let producer_timestamps = Arc::clone(&timestamps);
    let epoch = Instant::now() + Duration::from_millis(20);
    let producer = thread::spawn(move || {
        let (mut sent, mut errors) = (0, 0);
        for (index, event) in producer_work.events.iter().enumerate() {
            sleep_until(epoch + event.at);
            producer_timestamps[index]
                .store(epoch.elapsed().as_nanos() as u64 + 1, Ordering::Release);
            match sender.send_to(&event.bytes, addresses[event.route]) {
                Ok(size) if size == event.bytes.len() => sent += 1,
                _ => errors += 1,
            }
        }
        (sent, errors)
    });
    let mut transport = Transport::new(mode, &work.kinds);
    let mut result = UdpResult {
        cpu_ms: 0.,
        wall_s: 0.,
        sent: 0,
        eye: 0,
        session: 0,
        avatar: 0,
        invalid: 0,
        duplicate: 0,
        send_errors: 0,
        latency: Vec::with_capacity(work.events.len()),
        eye_latency: Vec::with_capacity(work.events.len()),
        session_latency: Vec::with_capacity(work.events.len()),
        checksum: 0,
    };
    let mut seen = vec![false; work.events.len()];
    sleep_until(epoch);
    let cpu = cpu_ns();
    let wall = Instant::now();
    let period = Duration::from_secs_f64(poll_hz.recip());
    let end = epoch + work.duration + Duration::from_millis(100);
    let mut tick = 0u32;
    while Instant::now() < end {
        transport.poll(
            &receivers,
            &work.kinds,
            tick as usize % receivers.len(),
            |kind, sample| {
                let Some((sequence, value)) = sample else {
                    result.invalid += 1;
                    return;
                };
                if sequence >= seen.len() {
                    result.invalid += 1;
                    return;
                }
                if seen[sequence] {
                    result.duplicate += 1;
                    return;
                }
                seen[sequence] = true;
                let sent_at = timestamps[sequence].load(Ordering::Acquire);
                if sent_at == 0 {
                    result.invalid += 1;
                    return;
                }
                let elapsed = epoch.elapsed().as_nanos() as u64;
                let latency = elapsed.saturating_sub(sent_at - 1) as f64 / 1000.;
                result.latency.push(latency);
                match kind {
                    Kind::Eye | Kind::Htc => {
                        result.eye += 1;
                        result.eye_latency.push(latency);
                    }
                    Kind::Avatar => {
                        result.avatar += 1;
                    }
                    Kind::Session => {
                        result.session += 1;
                        result.session_latency.push(latency);
                    }
                }
                result.checksum = result
                    .checksum
                    .wrapping_add(value)
                    .wrapping_add(sequence as u64);
            },
        )?;
        tick += 1;
        sleep_until(epoch + period * tick);
    }
    result.cpu_ms = (cpu_ns() - cpu) as f64 / 1e6;
    result.wall_s = wall.elapsed().as_secs_f64();
    (result.sent, result.send_errors) = producer.join().expect("producer failed");
    result.latency.sort_by(f64::total_cmp);
    result.eye_latency.sort_by(f64::total_cmp);
    result.session_latency.sort_by(f64::total_cmp);
    Ok(result)
}
fn main() -> io::Result<()> {
    let config = Config::parse();
    println!(
        "# UDP buffer-locality experiment; production eye/session/typed decoders, equal socket count/order/budget"
    );
    println!(
        "# scenario={}; pairs column denotes participants for avatar scenarios",
        config.avatars.as_deref().unwrap_or("synthetic-pairs")
    );
    if config.avatars.is_none() {
        println!(
            "# legacy workload: eye=120Hz session=20Hz per pair, scale={}, poll={}Hz, poses={}; times are receiver-thread CPU",
            config.scale, config.poll_hz, config.poses
        );
    }
    println!(
        "kind,pairs,mode,trial,packets,scratch_bytes,ns_per_packet,cpu_ms,wall_s,eye_received,session_received,sent,send_errors,dropped,invalid,duplicate,latency_p50_us,latency_p95_us,latency_p99_us,eye_p95_us,session_p95_us,checksum,avatar_received,scenario"
    );
    if let Some(case) = &config.avatars {
        eprintln!(
            "Avatar scenario={case}: one VR observer; CPU covers receive/decode, excludes rendering/IK and sender encoding"
        );
    }
    let scenario = config.avatars.as_deref().unwrap_or("synthetic-pairs");
    for pairs in config.pairs.iter().copied() {
        let work = Arc::new(if let Some(case) = &config.avatars {
            avatars::workload(case, config.seconds, config.scale)
        } else {
            Workload::new(pairs, config.seconds, config.scale, config.poses)
        });
        println!(
            "# offered_packets={} offered_datagram_bytes={} poll_hz={} rate_scale={}",
            work.events.len(),
            work.events.iter().map(|e| e.bytes.len()).sum::<usize>(),
            config.poll_hz,
            config.scale
        );
        let mut summaries: Vec<(Mode, Vec<f64>, Vec<f64>)> = config
            .modes
            .iter()
            .copied()
            .map(|mode| (mode, vec![], vec![]))
            .collect();
        // One excluded warmup trial for each mode, with the same workload.
        for mode in config.modes.iter().copied() {
            if mode != Mode::Runtime {
                replay(mode, &work, config.rounds.min(50));
            }
            if !config.replay_only {
                black_box(loopback(mode, Arc::clone(&work), config.poll_hz)?);
            }
        }
        for trial in 0..config.trials {
            let mut modes = config.modes.clone();
            if trial % 2 != 0 {
                modes.reverse();
            }
            for mode in modes {
                let summary = summaries.iter_mut().find(|(m, _, _)| *m == mode).unwrap();
                let (scratch, checksum) = if mode == Mode::Runtime {
                    let checksum = work
                        .events
                        .iter()
                        .map(|event| {
                            let (seq, value) = workload::decode(event.kind, &event.bytes).unwrap();
                            value.wrapping_add(seq as u64)
                        })
                        .fold(0u64, u64::wrapping_add);
                    (transport::SCRATCH, checksum)
                } else {
                    let (ns, scratch, checksum) = replay(mode, &work, config.rounds);
                    println!(
                        "replay,{pairs},{},{trial},{},{scratch},{ns:.3},,,,,,,,,,,,,,,{checksum},0,{scenario}",
                        mode.name(),
                        work.events.len() * config.rounds
                    );
                    summary.1.push(ns);
                    (scratch, checksum)
                };
                if !config.replay_only {
                    let result = loopback(mode, Arc::clone(&work), config.poll_hz)?;
                    let received = result.eye + result.session + result.avatar;
                    let drops = result.sent.saturating_sub(received);
                    println!(
                        "udp,{pairs},{},{trial},{received},{scratch},,{:.4},{:.6},{},{},{},{},{drops},{},{},{:.3},{:.3},{:.3},{:.3},{:.3},{},{},{scenario}",
                        mode.name(),
                        result.cpu_ms,
                        result.wall_s,
                        result.eye,
                        result.session,
                        result.sent,
                        result.send_errors,
                        result.invalid,
                        result.duplicate,
                        percentile(&result.latency, 0.5),
                        percentile(&result.latency, 0.95),
                        percentile(&result.latency, 0.99),
                        percentile(&result.eye_latency, 0.95),
                        percentile(&result.session_latency, 0.95),
                        result.checksum,
                        result.avatar
                    );
                    // Never treat reduced work caused by drops as a speedup.
                    assert_eq!(
                        result.invalid + result.duplicate,
                        0,
                        "invalid/duplicate workload"
                    );
                    if drops == 0 && result.send_errors == 0 {
                        assert_eq!(result.checksum, checksum, "semantic workload mismatch");
                        summary.2.push(result.cpu_ms);
                    }
                }
            }
        }
        for (mode, mut replay, mut cpu) in summaries {
            replay.sort_by(f64::total_cmp);
            cpu.sort_by(f64::total_cmp);
            eprintln!(
                "SUMMARY pairs={pairs} mode={} replay_ns/pkt median={:.3} min={:.3} max={:.3}; UDP receiver_cpu_ms median={:.4} ({}/{} loss-free trials)",
                mode.name(),
                median(&replay),
                replay.first().copied().unwrap_or(f64::NAN),
                replay.last().copied().unwrap_or(f64::NAN),
                median(&cpu),
                cpu.len(),
                config.trials
            );
        }
    }
    Ok(())
}
