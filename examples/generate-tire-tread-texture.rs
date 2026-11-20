//! Bake a tire tread strip, with width along U and circumference along V.
//! cargo run --example generate-tire-tread-texture -- [output-directory]
use image::{Rgba, RgbaImage};
use std::path::PathBuf;

const WIDTH: u32 = 128;
const HEIGHT: u32 = 256;
const ROWS: f32 = 16.0;
const SAMPLES: u32 = 4;
// Five segments per row. The two sloped segments advance the center by
// 18 percent of the row spacing and bring it back to the shoulder line.
const PROFILE: [f32; 6] = [0.0, 0.0, 0.18, 0.18, 0.0, 0.0];

fn tread_value(u: f32, v: f32) -> u32 {
    let across = u * (PROFILE.len() - 1) as f32;
    let segment = (across.floor() as usize).min(PROFILE.len() - 2);
    let start = PROFILE[segment];
    let offset = start + (PROFILE[segment + 1] - start) * across.fract();
    let phase = (v * ROWS - offset).rem_euclid(1.0);
    let distance = phase.min(1.0 - phase);
    if distance < 0.10 {
        148
    } else if distance < 0.15 {
        198
    } else {
        230
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let directory = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("assets/textures/vehicles"));
    std::fs::create_dir_all(&directory)?;
    let texture = RgbaImage::from_fn(WIDTH, HEIGHT, |x, y| {
        let mut total = 0;
        for sy in 0..SAMPLES {
            for sx in 0..SAMPLES {
                let u = (x as f32 + (sx as f32 + 0.5) / SAMPLES as f32) / WIDTH as f32;
                let v = (y as f32 + (sy as f32 + 0.5) / SAMPLES as f32) / HEIGHT as f32;
                total += tread_value(u, v);
            }
        }
        let value = (total / (SAMPLES * SAMPLES)) as u8;
        Rgba([value, value, value, 255])
    });
    let path = directory.join("tire-tread.png");
    texture.save(&path)?;
    println!("wrote {}", path.display());
    Ok(())
}
