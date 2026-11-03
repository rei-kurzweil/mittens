//! Offline spherical nebula bake: cloudy black/purple space without shader noise.
//! cargo run --release --example generate-ship-nebula-texture -- [output-directory]
use image::{Rgba, RgbaImage};
use std::{
    f32::consts::{PI, TAU},
    path::PathBuf,
};

const WIDTH: u32 = 1024;
const HEIGHT: u32 = 512;
const SEED: u32 = 0x4e45_4255;

fn hash(x: i32, y: i32, z: i32) -> f32 {
    let mut n = (x as u32).wrapping_mul(0x9e37_79b9)
        ^ (y as u32).wrapping_mul(0x85eb_ca6b)
        ^ (z as u32).wrapping_mul(0xc2b2_ae35)
        ^ SEED;
    n ^= n >> 16;
    n = n.wrapping_mul(0x7feb_352d);
    n ^= n >> 15;
    (n & 0xffffff) as f32 / 16777215.0
}
fn smooth(t: f32) -> f32 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}
fn noise(p: [f32; 3]) -> f32 {
    let cell = p.map(|v| v.floor() as i32);
    let t = std::array::from_fn::<_, 3, _>(|i| smooth(p[i] - p[i].floor()));
    let mut value = 0.0;
    for z in 0..2 {
        for y in 0..2 {
            for x in 0..2 {
                let weights = [x, y, z].map(|v| v as usize);
                let weight: f32 = (0..3)
                    .map(|i| if weights[i] == 0 { 1.0 - t[i] } else { t[i] })
                    .product();
                value += hash(cell[0] + x, cell[1] + y, cell[2] + z) * weight;
            }
        }
    }
    value
}
fn fbm(mut p: [f32; 3]) -> f32 {
    let mut value = 0.0;
    let mut weight = 0.55;
    for _ in 0..6 {
        value += noise(p) * weight;
        // Rotate the octave frame to avoid an axis-aligned grid of cloud blobs.
        p = [
            p[1] * 1.73 + p[2] * 0.91 + 17.1,
            p[2] * 1.73 - p[0] * 0.91 - 9.7,
            p[0] * 1.73 + p[1] * 0.91 + 23.4,
        ];
        weight *= 0.48;
    }
    value
}
fn color(direction: [f32; 3]) -> [u8; 4] {
    let warp =
        std::array::from_fn::<_, 3, _>(|i| fbm(direction.map(|v| v * 2.4 + i as f32 * 17.3)) - 0.5);
    let p = std::array::from_fn::<_, 3, _>(|i| direction[i] * 4.5 + warp[i] * 2.5);
    let clouds = fbm(p);
    let fine = fbm(p.map(|v| v * 2.7 + 11.6));
    let band_distance = direction[1] + direction[0] * 0.34 + direction[2] * 0.18 + warp[0] * 0.65;
    let band = (-band_distance.powi(2) * 5.5).exp();
    let density = ((clouds - 0.34) * 2.6).clamp(0.0, 1.0) * band;
    let wisps = ((fine - 0.40) * 4.0).clamp(0.0, 1.0) * density.powf(1.6);
    let dust = (1.0 - ((clouds - 0.53) * 10.0).abs()).clamp(0.0, 1.0);
    let glow = density.powf(1.5) * (0.72 + 0.28 * dust);
    let rgb = [
        0.003 + glow * 0.38 + wisps * 0.20,
        0.001 + glow * 0.075 + wisps * 0.09,
        0.009 + glow * 0.62 + wisps * 0.28,
    ];
    [
        (rgb[0] * 255.0).round() as u8,
        (rgb[1] * 255.0).round() as u8,
        (rgb[2] * 255.0).round() as u8,
        255,
    ]
}
fn nebula() -> RgbaImage {
    let mut image = RgbaImage::from_fn(WIDTH, HEIGHT, |x, y| {
        // Noise lives on a unit sphere: both the longitude seam and pole
        // continuity are intrinsic to the bake, rather than feathered borders.
        let longitude = (x as f32 / (WIDTH - 1) as f32 - 0.5) * TAU;
        let latitude = y as f32 / (HEIGHT - 1) as f32 * PI;
        let direction = if y == 0 {
            [0.0, 1.0, 0.0]
        } else if y == HEIGHT - 1 {
            [0.0, -1.0, 0.0]
        } else {
            [
                latitude.sin() * longitude.cos(),
                latitude.cos(),
                latitude.sin() * longitude.sin(),
            ]
        };
        Rgba(color(direction))
    });
    for y in 0..HEIGHT {
        image.put_pixel(WIDTH - 1, y, *image.get_pixel(0, y));
    }
    image
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let directory = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("assets/textures/space"));
    std::fs::create_dir_all(&directory)?;
    nebula().save(directory.join("ship-nebula.png"))?;
    println!("wrote {}/ship-nebula.png", directory.display());
    Ok(())
}

#[test]
fn panorama_has_a_closed_longitude_seam_and_uniform_poles() {
    let image = nebula();
    for y in 0..HEIGHT {
        assert_eq!(image.get_pixel(0, y), image.get_pixel(WIDTH - 1, y));
    }
    for x in 0..WIDTH {
        assert_eq!(image.get_pixel(x, 0), image.get_pixel(0, 0));
        assert_eq!(
            image.get_pixel(x, HEIGHT - 1),
            image.get_pixel(0, HEIGHT - 1)
        );
    }
    let dark = image.pixels().filter(|p| p[2] < 8).count();
    let glow = image.pixels().filter(|p| p[2] > 60).count();
    assert!(
        dark > 1000 && glow > 1000,
        "retain black space and readable nebula clouds"
    );
}
