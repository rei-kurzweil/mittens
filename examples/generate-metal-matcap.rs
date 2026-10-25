//! Procedural chrome environment/matcap. Regenerate with:
//! cargo run --release --example generate-metal-matcap
use image::{Rgba, RgbaImage};
fn smooth(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
fn byte(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0).round() as u8
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let size = 512;
    let mut chrome = RgbaImage::new(size, size);
    for y in 0..size {
        for x in 0..size {
            let u = (x as f32 + 0.5) / size as f32;
            let v = (y as f32 + 0.5) / size as f32;
            let nx = 2.0 * u - 1.0;
            let ny = 1.0 - 2.0 * v;
            let nz = (1.0 - nx * nx - ny * ny).max(0.0).sqrt();
            let sky = smooth(-0.4, 0.8, ny);
            let horizon = (-((ny + 0.05) / 0.13).powi(2)).exp();
            let band = (-((nx + 0.38) / 0.09).powi(2)).exp() * smooth(-0.75, 0.65, ny);
            let softbox = (-((nx - 0.3) / 0.22).powi(2) - ((ny - 0.55) / 0.28).powi(2)).exp();
            let sheen =
                0.06 + 0.40 * sky + 0.18 * horizon + 0.78 * band + 0.72 * softbox + 0.05 * nz;
            chrome.put_pixel(
                x,
                y,
                Rgba([
                    byte(sheen * 0.94),
                    byte(sheen * 0.98),
                    byte(sheen * 1.04),
                    255,
                ]),
            );
        }
    }
    std::fs::create_dir_all("assets/textures/pool")?;
    chrome.save("assets/textures/pool/chrome-matcap.png")?;
    println!("wrote assets/textures/pool/chrome-matcap.png");
    Ok(())
}
