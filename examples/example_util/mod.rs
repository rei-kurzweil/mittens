use cat_engine::engine;

fn hash_u32(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb_352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846c_a68b);
    x ^= x >> 16;
    x
}

fn rand01(seed: u32) -> f32 {
    (hash_u32(seed) as f32) / (u32::MAX as f32)
}

#[derive(Debug, Clone, Copy)]
pub struct CloudRingParams {
    pub cloud_count: u32,
    pub radius: f32,
    pub center_y: f32,
    pub puffs_per_cloud: u32,
    /// 0.0 = perfectly evenly spaced, 1.0 = up to one full step of jitter.
    pub angle_jitter: f32,
    /// Probability a given cluster is placed higher than `center_y`.
    pub high_y_probability: f32,
    /// Multiplier applied to `center_y` when a cluster is chosen to be high.
    pub high_y_multiplier: f32,
    /// Seed used for deterministic layout variation.
    pub seed: u32,
}

impl Default for CloudRingParams {
    fn default() -> Self {
        Self {
            cloud_count: 5,
            radius: 26.0,
            center_y: 2.0,
            puffs_per_cloud: 28,
            angle_jitter: 0.0,
            high_y_probability: 0.0,
            high_y_multiplier: 1.0,
            seed: 0xC10u32,
        }
    }
}

/// Spawns a ring of "cloud" puff clusters under an existing background root.
///
/// Assumes `bg_root` is a `BackgroundComponent` (usually `with_occlusion_and_lighting()`).
pub fn spawn_cloud_ring(
    universe: &mut engine::Universe,
    bg_root: engine::ecs::ComponentId,
    p: CloudRingParams,
) {
    if p.cloud_count == 0 || p.radius == 0.0 {
        return;
    }

    let cube_mesh = universe
        .render_assets
        .get_mesh(engine::graphics::BuiltinMeshType::Cube);

    let step = std::f32::consts::TAU / (p.cloud_count as f32);
    let angle_jitter = p.angle_jitter.clamp(0.0, 1.0);
    let high_y_probability = p.high_y_probability.clamp(0.0, 1.0);

    for i in 0..p.cloud_count {
        let seed_i = p.seed ^ i.wrapping_mul(0x9E37_79B9);
        let jitter = (rand01(seed_i ^ 0xa53a_9d2d) - 0.5) * step * angle_jitter;
        let a = (i as f32) * step + jitter;
        let cx = p.radius * a.cos();
        let cz = p.radius * a.sin();

        let cy = if rand01(seed_i ^ 0x7f4a_7c15) < high_y_probability {
            p.center_y * p.high_y_multiplier
        } else {
            p.center_y
        };

        let center_tx = universe
            .world
            .register(engine::ecs::component::TransformComponent::new().with_position(cx, cy, cz));
        let _ = universe.attach(bg_root, center_tx);

        let base_seed = seed_i ^ 0x243f_6a88;
        for puff_i in 0..p.puffs_per_cloud {
            let seed = base_seed ^ puff_i.wrapping_mul(1_103_515_245);

            let ox = (rand01(seed ^ 0x68bc_21eb) - 0.5) * 9.0;
            let oy = (rand01(seed ^ 0x02e5_be93) - 0.5) * 4.0;
            let oz = (rand01(seed ^ 0xa1d3_4f2b) - 0.5) * 9.0;

            let base = 0.7 + rand01(seed ^ 0x9e37_79b9) * 2.8;
            let sx = base * (0.7 + rand01(seed ^ 0x243f_6a88) * 0.9);
            let sy = base * (0.6 + rand01(seed ^ 0x85a3_08d3) * 1.0);
            let sz = base * (0.7 + rand01(seed ^ 0x1319_8a2e) * 0.9);

            let tx = universe.world.register(
                engine::ecs::component::TransformComponent::new()
                    .with_position(ox, oy, oz)
                    .with_scale(sx, sy, sz),
            );
            let renderable =
                universe
                    .world
                    .register(engine::ecs::component::RenderableComponent::new(
                        engine::graphics::primitives::Renderable::new(
                            cube_mesh,
                            engine::graphics::primitives::MaterialHandle::TOON_MESH,
                        ),
                    ));

            let t = rand01(seed ^ 0x7f4a_7c15);
            let r = 0.70 + 0.10 * t;
            let g = 0.72 + 0.10 * t;
            let b = 0.80 + 0.12 * t;
            let color = universe
                .world
                .register(engine::ecs::component::ColorComponent::rgba(r, g, b, 1.0));

            let _ = universe.attach(center_tx, tx);
            let _ = universe.attach(tx, renderable);
            let _ = universe.attach(renderable, color);
        }
    }
}
