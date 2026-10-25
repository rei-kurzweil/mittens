//! Deterministic seamless pool-tile textures; all material source data is retained. Regenerate with:
//! cargo run --release --example generate-pool-tiles
use image::{Rgba, RgbaImage};
use std::path::Path;

const SIZE: u32 = 512;
fn smooth(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
fn height(u: f32, v: f32) -> f32 {
    let u = u.rem_euclid(1.0);
    let v = v.rem_euclid(1.0);
    smooth(0.009, 0.035, u.min(1.0 - u).min(v.min(1.0 - v)))
}
fn byte(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0).round() as u8
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = Path::new("assets/textures/pool");
    std::fs::create_dir_all(path)?;
    let mut albedo = RgbaImage::new(SIZE, SIZE);
    let mut normal = RgbaImage::new(SIZE, SIZE);
    let mut surface = RgbaImage::new(SIZE, SIZE);
    let mut roughness = RgbaImage::new(SIZE, SIZE);
    let step = 1.0 / SIZE as f32;
    for y in 0..SIZE {
        for x in 0..SIZE {
            let u = (x as f32 + 0.5) * step;
            let v = (y as f32 + 0.5) * step;
            let h = height(u, v);
            // One millimetre bevel height for a 25 cm tile. Central differences
            // give a tangent-space normal; all borders repeat seamlessly.
            let dx = (height(u + step, v) - height(u - step, v)) / (2.0 * step) * 0.004;
            let dy = (height(u, v + step) - height(u, v - step)) / (2.0 * step) * 0.004;
            let length = (dx * dx + dy * dy + 1.0).sqrt();
            let n = [
                byte(0.5 - 0.5 * dx / length),
                byte(0.5 - 0.5 * dy / length),
                byte(0.5 + 0.5 / length),
            ];
            let hash = x
                .wrapping_mul(374761393)
                .wrapping_add(y.wrapping_mul(668265263));
            let noise = ((hash ^ (hash >> 13)) & 255) as f32 / 255.0 - 0.5;
            let color = byte(0.74 + 0.21 * h + noise * 0.006 * h);
            albedo.put_pixel(x, y, Rgba([color, color, color, 255]));
            normal.put_pixel(x, y, Rgba([n[0], n[1], n[2], 255]));
            // The first custom texture contract has one RGBA8 image. RGB holds
            // normal data, A holds grayscale albedo (not opacity in this shader).
            surface.put_pixel(x, y, Rgba([n[0], n[1], n[2], color]));
            let r = byte(0.78 - 0.58 * h);
            roughness.put_pixel(x, y, Rgba([r, r, r, 255]));
        }
    }
    for (name, image) in [
        ("white-tiles-albedo.png", albedo),
        ("white-tiles-normal.png", normal),
        ("white-tiles-surface.png", surface),
        ("white-tiles-roughness.png", roughness),
    ] {
        image.save(path.join(name))?;
        println!("wrote {}", path.join(name).display());
    }
    Ok(())
}
