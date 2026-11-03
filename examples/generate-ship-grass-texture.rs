//! Deterministic seamless pale grass with short blades at varied angles.
//! cargo run --release --example generate-ship-grass-texture -- [output-directory]
use image::{Rgba, RgbaImage};
use std::{f32::consts::TAU, path::PathBuf};

const SIZE: u32 = 512;
const PERIOD: i32 = SIZE as i32 - 1;
const SEED: u32 = 0x5348_4950;

fn hash(x: u32, y: u32) -> f32 {
    let mut n = x.wrapping_mul(0x9e37_79b9) ^ y.wrapping_mul(0x85eb_ca6b) ^ SEED;
    n ^= n >> 16;
    n = n.wrapping_mul(0x7feb_352d);
    n ^= n >> 15;
    (n & 0xffff) as f32 / 65535.0
}
fn noise(u: f32, v: f32, cells: u32) -> f32 {
    let x = u * cells as f32;
    let y = v * cells as f32;
    let ix = x.floor() as u32;
    let iy = y.floor() as u32;
    let sx = x.fract().powi(2) * (3.0 - 2.0 * x.fract());
    let sy = y.fract().powi(2) * (3.0 - 2.0 * y.fract());
    let a = hash(ix % cells, iy % cells);
    let b = hash((ix + 1) % cells, iy % cells);
    let c = hash(ix % cells, (iy + 1) % cells);
    let d = hash((ix + 1) % cells, (iy + 1) % cells);
    (a + (b - a) * sx) * (1.0 - sy) + (c + (d - c) * sx) * sy
}
fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    std::array::from_fn(|i| a[i] + (b[i] - a[i]) * t.clamp(0.0, 1.0))
}
fn random(state: &mut u32) -> f32 {
    *state ^= *state << 13;
    *state ^= *state >> 17;
    *state ^= *state << 5;
    (*state & 0xffffff) as f32 / 16777215.0
}
fn grass() -> RgbaImage {
    let mut image = RgbaImage::from_fn(SIZE, SIZE, |x, y| {
        let u = x as f32 / PERIOD as f32;
        let v = y as f32 / PERIOD as f32;
        let broad = noise(u, v, 4);
        let patch = noise(u, v, 9);
        let fine = noise(u, v, 48);
        let green = mix([170.0, 181.0, 114.0], [210.0, 204.0, 149.0], broad);
        let color = mix(green, [163.0, 145.0, 104.0], (patch - 0.45).max(0.0) * 0.85);
        Rgba([
            color[0] as u8 + (fine * 4.0) as u8,
            color[1] as u8 + (fine * 4.0) as u8,
            color[2] as u8 + (fine * 4.0) as u8,
            255,
        ])
    });
    let mut state = SEED;
    for _ in 0..1700 {
        let cx = random(&mut state) * PERIOD as f32;
        let cy = random(&mut state) * PERIOD as f32;
        let angle = random(&mut state) * TAU;
        let length = 3.0 + random(&mut state) * 6.0;
        let width = 0.45 + random(&mut state) * 0.65;
        let color = mix(
            [114.0, 139.0, 77.0],
            [223.0, 207.0, 142.0],
            random(&mut state),
        );
        let dx = angle.cos();
        let dy = angle.sin();
        let radius = length.ceil() as i32 + 2;
        for y in cy as i32 - radius..=cy as i32 + radius {
            for x in cx as i32 - radius..=cx as i32 + radius {
                let px = x as f32 + 0.5 - cx;
                let py = y as f32 + 0.5 - cy;
                let along = (px * dx + py * dy).clamp(-length / 2.0, length / 2.0);
                let distance = ((px - along * dx).powi(2) + (py - along * dy).powi(2)).sqrt();
                let alpha = (width + 0.65 - distance).clamp(0.0, 1.0) * 0.65;
                let pixel =
                    image.get_pixel_mut(x.rem_euclid(PERIOD) as u32, y.rem_euclid(PERIOD) as u32);
                for channel in 0..3 {
                    pixel[channel] = (pixel[channel] as f32 * (1.0 - alpha)
                        + color[channel] * alpha)
                        .round() as u8;
                }
            }
        }
    }
    // Exact matching opposite borders, including strokes that cross a seam.
    for y in 0..SIZE - 1 {
        image.put_pixel(SIZE - 1, y, *image.get_pixel(0, y));
    }
    for x in 0..SIZE {
        image.put_pixel(x, SIZE - 1, *image.get_pixel(x, 0));
    }
    image
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("assets/textures/grass"));
    std::fs::create_dir_all(&output)?;
    grass().save(output.join("ship-park-grass.png"))?;
    println!("wrote {}/ship-park-grass.png", output.display());
    Ok(())
}

#[test]
fn generated_grass_has_matching_edges_and_varied_color() {
    let image = grass();
    for i in 0..SIZE {
        assert_eq!(image.get_pixel(0, i), image.get_pixel(SIZE - 1, i));
        assert_eq!(image.get_pixel(i, 0), image.get_pixel(i, SIZE - 1));
    }
    assert_ne!(image.get_pixel(30, 70), image.get_pixel(130, 170));
}
