use cat_engine::engine;

#[derive(Debug, Clone, Copy)]
pub struct PerlinNoise {
    perm: [u8; 512],
}

impl PerlinNoise {
    pub fn new(seed: u32) -> Self {
        let mut base = [0u8; 256];
        for (i, slot) in base.iter_mut().enumerate() {
            *slot = i as u8;
        }

        let mut state = seed;
        for i in (1..256).rev() {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let j = (state as usize) % (i + 1);
            base.swap(i, j);
        }

        let mut perm = [0u8; 512];
        for i in 0..512 {
            perm[i] = base[i & 255];
        }

        Self { perm }
    }

    pub fn sample2(&self, x: f32, y: f32) -> f32 {
        let xi = x.floor() as i32 & 255;
        let yi = y.floor() as i32 & 255;

        let xf = x - x.floor();
        let yf = y - y.floor();

        let u = fade(xf);
        let v = fade(yf);

        let aa = self.perm[(self.perm[xi as usize] as usize + yi as usize) & 255];
        let ab = self.perm[(self.perm[xi as usize] as usize + ((yi + 1) as usize)) & 255];
        let ba = self.perm[(self.perm[((xi + 1) & 255) as usize] as usize + yi as usize) & 255];
        let bb =
            self.perm[(self.perm[((xi + 1) & 255) as usize] as usize + ((yi + 1) as usize)) & 255];

        let x1 = lerp(grad2(aa, xf, yf), grad2(ba, xf - 1.0, yf), u);
        let x2 = lerp(grad2(ab, xf, yf - 1.0), grad2(bb, xf - 1.0, yf - 1.0), u);
        lerp(x1, x2, v)
    }

    pub fn fractal2(&self, x: f32, y: f32, octaves: u32, lacunarity: f32, gain: f32) -> f32 {
        let mut freq = 1.0;
        let mut amp = 1.0;
        let mut total = 0.0;
        let mut norm = 0.0;

        for _ in 0..octaves.max(1) {
            total += self.sample2(x * freq, y * freq) * amp;
            norm += amp;
            freq *= lacunarity;
            amp *= gain;
        }

        if norm > 0.0 { total / norm } else { 0.0 }
    }
}

fn fade(t: f32) -> f32 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn grad2(hash: u8, x: f32, y: f32) -> f32 {
    match hash & 0x7 {
        0 => x + y,
        1 => -x + y,
        2 => x - y,
        3 => -x - y,
        4 => x,
        5 => -x,
        6 => y,
        _ => -y,
    }
}

pub fn spawn_perlin_cube_patch(
    universe: &mut engine::Universe,
    anchor: [f32; 3],
    width: usize,
    depth: usize,
) {
    use engine::ecs::component::{ColorComponent, RenderableComponent, TransformComponent};

    if width == 0 || depth == 0 {
        return;
    }

    let noise = PerlinNoise::new(0xCA7F_2026);
    let spacing = 0.22_f32;
    let cube_size = 0.20_f32;
    let sample_scale = 0.052_f32;
    let height_scale = 2.6_f32;
    let half_w = (width as f32 - 1.0) * spacing * 0.5;
    let half_d = (depth as f32 - 1.0) * spacing * 0.5;

    let root = universe
        .world
        .add_component(TransformComponent::new().with_position(anchor[0], anchor[1], anchor[2]));
    universe.add(root);

    for z in 0..depth {
        for x in 0..width {
            let nx = x as f32 * sample_scale;
            let nz = z as f32 * sample_scale;
            let h = noise.fractal2(nx, nz, 5, 2.0, 0.5);
            let y = h * height_scale;

            let tx = universe.world.add_component(
                TransformComponent::new()
                    .with_position(x as f32 * spacing - half_w, y, z as f32 * spacing - half_d)
                    .with_scale(cube_size, cube_size, cube_size),
            );
            let renderable = universe.world.add_component(RenderableComponent::cube());

            let shade = ((h + 1.0) * 0.5).clamp(0.0, 1.0);
            let r = 0.10 + 0.18 * shade;
            let g = 0.28 + 0.55 * shade;
            let b = 0.40 + 0.22 * shade;
            let color = universe
                .world
                .add_component(ColorComponent::rgba(r, g, b, 1.0));

            let _ = universe.attach(root, tx);
            let _ = universe.attach(tx, renderable);
            let _ = universe.attach(renderable, color);
        }
    }
}
