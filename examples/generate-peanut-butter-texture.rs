//! Procedural jar artwork, independent of fonts and external image assets.
//! cargo run --release --example generate-peanut-butter-texture -- [output-directory]
//! Palette, badge geometry and bitmap lettering below are intended for iteration.
use image::{Rgba, RgbaImage};
use std::path::PathBuf;

const RED: [f32; 3] = [190.0, 20.0, 30.0];
const WRAPPER: [f32; 3] = [238.0, 145.0, 148.0];
const INK: [f32; 3] = [91.0, 42.0, 20.0];
const CREAM: [f32; 3] = [255.0, 235.0, 177.0];

fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    std::array::from_fn(|i| a[i] + (b[i] - a[i]) * t.clamp(0.0, 1.0))
}

// A peanut silhouette with two rounded lobes and a pinched waist. Rotate the
// sample frame so the nut sits diagonally within its circular medallion.
fn peanut(x: f32, y: f32) -> bool {
    let u = x * 0.866 + y * 0.5;
    let v = -x * 0.5 + y * 0.866;
    let angle = u.atan2(v);
    let radius = 0.185 + 0.075 * (2.0 * angle).cos();
    u * u + v * v < radius * radius
}

fn badge(x: f32, y: f32, background: [f32; 3]) -> [f32; 3] {
    let radius = (x * x + y * y).sqrt();
    if radius > 0.36 {
        return background;
    }
    if radius > 0.335 {
        return CREAM;
    }
    if radius > 0.319 {
        return INK;
    }
    // Warm cream-to-orange gradient, confined to the circle behind the nut.
    let gradient = ((x - y) * 0.65 + 0.5).clamp(0.0, 1.0);
    let color = mix([255.0, 241.0, 184.0], [237.0, 150.0, 71.0], gradient);
    if peanut(x, y) { INK } else { color }
}

fn artwork(lid: bool, x: f32, y: f32) -> [f32; 3] {
    if lid {
        // The outer cap remains solid red; only the circular printed seal varies.
        badge(x, y, RED)
    } else {
        let base = mix(WRAPPER, [248.0, 177.0, 167.0], y + 0.5);
        if y.abs() > 0.45 {
            return RED;
        }
        if y.abs() > 0.425 {
            return CREAM;
        }
        // Repeat two front/back medallions around the cylindrical wrapper.
        let local_x = ((x + 0.5) * 2.0).fract() - 0.5;
        badge(local_x * 1.7, y * 1.7, base)
    }
}

fn raster(width: u32, height: u32, lid: bool) -> RgbaImage {
    // 2x2 supersampling keeps silhouette and circular borders clean at shelf size.
    RgbaImage::from_fn(width, height, |px, py| {
        let mut sum = [0.0; 3];
        for sy in 0..2 {
            for sx in 0..2 {
                let x = (px as f32 + (sx as f32 + 0.5) / 2.0) / width as f32 - 0.5;
                let y = (py as f32 + (sy as f32 + 0.5) / 2.0) / height as f32 - 0.5;
                let color = artwork(lid, x, y);
                for i in 0..3 {
                    sum[i] += color[i];
                }
            }
        }
        Rgba([
            (sum[0] / 4.0).round() as u8,
            (sum[1] / 4.0).round() as u8,
            (sum[2] / 4.0).round() as u8,
            255,
        ])
    })
}

fn glyph(letter: char) -> [u8; 7] {
    match letter {
        'A' => [14, 17, 17, 31, 17, 17, 17],
        'B' => [30, 17, 17, 30, 17, 17, 30],
        'C' => [14, 17, 16, 16, 16, 17, 14],
        'D' => [30, 17, 17, 17, 17, 17, 30],
        'E' => [31, 16, 16, 30, 16, 16, 31],
        'I' => [31, 4, 4, 4, 4, 4, 31],
        'K' => [17, 18, 20, 24, 20, 18, 17],
        'M' => [17, 27, 21, 21, 17, 17, 17],
        'N' => [17, 25, 25, 21, 19, 19, 17],
        'P' => [30, 17, 17, 30, 16, 16, 16],
        'R' => [30, 17, 17, 30, 20, 18, 17],
        'S' => [15, 16, 16, 14, 1, 1, 30],
        'T' => [31, 4, 4, 4, 4, 4, 4],
        'U' => [17, 17, 17, 17, 17, 17, 14],
        'Y' => [17, 17, 10, 4, 4, 4, 4],
        _ => [0; 7],
    }
}

fn text(image: &mut RgbaImage, value: &str, center: i32, top: i32, scale: i32) {
    let width = (value.len() as i32 * 6 - 1) * scale;
    for (index, letter) in value.chars().enumerate() {
        for (row, bits) in glyph(letter).iter().enumerate() {
            for column in 0..5 {
                if bits & (1 << (4 - column)) == 0 {
                    continue;
                }
                for y in 0..scale {
                    for x in 0..scale {
                        let px = center - width / 2 + (index as i32 * 6 + column) * scale + x;
                        let py = top + row as i32 * scale + y;
                        if px >= 0
                            && py >= 0
                            && px < image.width() as i32
                            && py < image.height() as i32
                        {
                            image.put_pixel(px as u32, py as u32, Rgba([91, 42, 20, 255]));
                        }
                    }
                }
            }
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("assets/textures/groceries"));
    std::fs::create_dir_all(&output)?;
    let lid = raster(512, 512, true);
    let mut wrapper = raster(1024, 512, false);
    for center in [256, 768] {
        text(&mut wrapper, "DECKSIDE", center, 44, 3);
        text(&mut wrapper, "PEANUT BUTTER", center, 411, 4);
        text(&mut wrapper, "CREAMY", center, 452, 2);
    }
    lid.save(output.join("peanut-butter-lid.png"))?;
    wrapper.save(output.join("peanut-butter-wrapper.png"))?;
    println!(
        "wrote peanut-butter lid and wrapper to {}",
        output.display()
    );
    Ok(())
}
