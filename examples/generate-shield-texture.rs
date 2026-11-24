//! Bake a tileable lilac Voronoi texture for the energy shield.
//! cargo run --release --example generate-shield-texture -- [output.png]
use image::{Rgb, RgbImage};
use std::path::PathBuf;

const COLUMNS: i32 = 8;
const ROWS: i32 = 5;
const HALF_LINE_WIDTH: f32 = 0.092;

fn noise(x: i32, y: i32, salt: u32) -> f32 {
    let mut n = (x.rem_euclid(COLUMNS) as u32).wrapping_mul(0x9e37_79b9)
        ^ (y.rem_euclid(ROWS) as u32).wrapping_mul(0x85eb_ca6b)
        ^ salt;
    n = (n ^ (n >> 16)).wrapping_mul(0x7feb_352d);
    n = (n ^ (n >> 15)).wrapping_mul(0x846c_a68b);
    ((n ^ (n >> 16)) >> 8) as f32 / 16_777_216.0
}

fn triangle(phase: f32) -> f32 {
    4.0 * (phase.rem_euclid(1.0) - 0.5).abs() - 1.0
}

fn smoothstep(start: f32, end: f32, x: f32) -> f32 {
    let t = ((x - start) / (end - start)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn blend(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    std::array::from_fn(|channel| a[channel] + (b[channel] - a[channel]) * t)
}

fn sample(u: f32, v: f32) -> Rgb<u8> {
    // Integer frequencies keep the bends continuous across the UV seam.
    // Triangle waves make angular breaks along otherwise straight cell walls.
    let x = u * COLUMNS as f32
        + 0.095 * triangle(19.0 * v + 11.0 * u)
        + 0.062 * triangle(31.0 * v - 13.0 * u);
    let y = v * ROWS as f32
        + 0.105 * triangle(17.0 * u - 9.0 * v)
        + 0.054 * triangle(27.0 * u + 13.0 * v);
    let cell_x = x.floor() as i32;
    let cell_y = y.floor() as i32;
    let mut sites = [([0.0, 0.0], 0.0, 0.0); 25];
    let mut nearest = 0;
    for (index, (dx, dy)) in (-2..=2)
        .flat_map(|dy| (-2..=2).map(move |dx| (dx, dy)))
        .enumerate()
    {
        let column = cell_x + dx;
        let row = cell_y + dy;
        let offset = [
            column as f32 + 0.16 + 0.68 * noise(column, row, 1123) - x,
            row as f32 + 0.16 + 0.68 * noise(column, row, 3313) - y,
        ];
        let distance_squared = offset[0] * offset[0] + offset[1] * offset[1];
        sites[index] = (offset, distance_squared, noise(column, row, 5939));
        if distance_squared < sites[nearest].1 {
            nearest = index;
        }
    }

    let (center, nearest_distance, variation) = sites[nearest];
    // Distance to the cell walls measured along each site's bisector normal.
    // This keeps the line wide even when the sites are unevenly spaced.
    let mut edge = f32::INFINITY;
    for (index, (offset, distance_squared, _)) in sites.into_iter().enumerate() {
        if index != nearest {
            let separation = (offset[0] - center[0]).hypot(offset[1] - center[1]);
            edge = edge.min((distance_squared - nearest_distance) / (2.0 * separation));
        }
    }
    let rim = 1.0 - smoothstep(HALF_LINE_WIDTH * 0.8, HALF_LINE_WIDTH * 1.25, edge);
    let glow = 1.0 - smoothstep(HALF_LINE_WIDTH, HALF_LINE_WIDTH + 0.125, edge);
    let core = 1.0 - smoothstep(0.024, HALF_LINE_WIDTH, edge);
    let fill = blend([108.0, 72.0, 155.0], [153.0, 106.0, 190.0], variation);
    let fill = blend(fill, [184.0, 131.0, 227.0], glow * 0.65);
    let line = blend([202.0, 153.0, 241.0], [244.0, 220.0, 255.0], core);
    Rgb(blend(fill, line, rim).map(|channel| channel.round() as u8))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("assets/textures/energy-shield-voronoi.png"));
    let (width, height) = (512, 320);
    let image = RgbImage::from_fn(width, height, |x, y| {
        sample(
            (x as f32 + 0.5) / width as f32,
            (y as f32 + 0.5) / height as f32,
        )
    });
    if let Some(parent) = output.parent().filter(|path| !path.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    image.save(&output)?;
    println!("wrote {} ({width}×{height})", output.display());
    Ok(())
}
