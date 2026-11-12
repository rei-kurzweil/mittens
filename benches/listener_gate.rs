//! Microbenchmark for listener-gate overhead versus skipped per-mover query work.
//!
//! This deliberately uses synthetic query work: the generic swept-zone path is
//! not wired into the runtime yet, so this measures the gate/filtering strategy,
//! not collision-query performance. Run with:
//!
//!     cargo bench --bench listener_gate

use std::hint::black_box;
use std::time::{Duration, Instant};

use mittens_engine::engine::ecs::{
    RxWorld, Signal, SignalEmitter, SignalKind, World,
    component::TransformComponent,
};

const GROUPS: usize = 100;
const MOVERS_PER_GROUP: usize = 20;
const ROUNDS: usize = 100;

fn noop_handler(_: &mut World, _: &mut dyn SignalEmitter, _: &Signal) {}

#[derive(Clone, Copy)]
enum ListenerLayout {
    None,
    OneGroup,
    Global,
}

fn fixture(layout: ListenerLayout) -> (World, RxWorld, Vec<mittens_engine::engine::ecs::ComponentId>) {
    let mut world = World::default();
    let mut movers = Vec::with_capacity(GROUPS * MOVERS_PER_GROUP);
    let mut groups = Vec::with_capacity(GROUPS);

    for _ in 0..GROUPS {
        let group = world.add_component(TransformComponent::new());
        groups.push(group);
        for _ in 0..MOVERS_PER_GROUP {
            let mover = world.add_component(TransformComponent::new());
            world.set_parent(mover, Some(group)).unwrap();
            movers.push(mover);
        }
    }

    let mut rx = RxWorld::default();
    match layout {
        ListenerLayout::None => {}
        ListenerLayout::OneGroup => {
            rx.add_handler_closure(SignalKind::ZoneSwept, groups[0], |_, _, _| {});
        }
        ListenerLayout::Global => {
            rx.add_global_handler(SignalKind::ZoneSwept, noop_handler);
        }
    }
    (world, rx, movers)
}

/// Stable CPU work standing in for a small candidate/narrow-phase query.
/// Replace this with actual sweep calls once the generic path is integrated.
fn simulated_query_work(seed: usize) -> usize {
    let mut value = black_box(seed.wrapping_add(1));
    for _ in 0..128 {
        value = value
            .wrapping_mul(6_364_136_223_846_793_005usize)
            .wrapping_add(1);
    }
    black_box(value)
}

fn run_without_gate(movers: &[mittens_engine::engine::ecs::ComponentId]) -> (Duration, usize, usize) {
    let start = Instant::now();
    let mut queries = 0;
    let mut checksum = 0usize;
    for _ in 0..ROUNDS {
        for (index, _) in movers.iter().enumerate() {
            checksum ^= simulated_query_work(index);
            queries += 1;
        }
    }
    (start.elapsed(), queries, black_box(checksum))
}

fn run_with_gate(
    world: &World,
    gate: &mittens_engine::engine::ecs::ListenerGate,
    movers: &[mittens_engine::engine::ecs::ComponentId],
) -> (Duration, usize, usize) {
    let start = Instant::now();
    let mut queries = 0;
    let mut checksum = 0usize;
    if gate.any() {
        for _ in 0..ROUNDS {
            for (index, &mover_scope) in movers.iter().enumerate() {
                if gate.allows(world, mover_scope) {
                    checksum ^= simulated_query_work(index);
                    queries += 1;
                }
            }
        }
    }
    (start.elapsed(), queries, black_box(checksum))
}

fn report(name: &str, layout: ListenerLayout) {
    let (world, rx, movers) = fixture(layout);
    let gate = rx.listener_gate(SignalKind::ZoneSwept);
    let (baseline_time, baseline_queries, baseline_checksum) = run_without_gate(&movers);
    let (gated_time, gated_queries, gated_checksum) = run_with_gate(&world, &gate, &movers);

    let expected_gated_queries = match layout {
        ListenerLayout::None => 0,
        ListenerLayout::OneGroup => MOVERS_PER_GROUP * ROUNDS,
        ListenerLayout::Global => GROUPS * MOVERS_PER_GROUP * ROUNDS,
    };
    assert_eq!(gated_queries, expected_gated_queries, "wrong listener filtering result");
    if gated_queries == baseline_queries {
        assert_eq!(gated_checksum, baseline_checksum);
    }

    println!(
        "{name:>12}: ungated={baseline_time:?} ({baseline_queries} queries), \
         gated={gated_time:?} ({gated_queries} queries), gate.any()={} ",
        gate.any()
    );
}

fn main() {
    println!(
        "ListenerGate microbenchmark: {GROUPS} groups × {MOVERS_PER_GROUP} movers, \
         {ROUNDS} rounds; query work is synthetic, not collision physics."
    );
    report("no listeners", ListenerLayout::None);
    report("one group", ListenerLayout::OneGroup);
    report("global", ListenerLayout::Global);
}
