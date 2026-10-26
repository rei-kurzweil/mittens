//! Generate an asymmetric white background using the actual 🌸 color-font glyph.
//! cargo run --release --example generate-cherry-blossoms -- --help
use image::{Rgba, RgbaImage, imageops::FilterType};
use std::{error::Error, path::PathBuf};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

struct Options {
    width: u32,
    height: u32,
    seed: u32,
    density: f32,
    sparse_left: bool,
    font: PathBuf,
    output: PathBuf,
}

impl Options {
    fn parse() -> Result<Option<Self>> {
        let mut options = Self {
            width: 2048,
            height: 2048,
            seed: 42,
            density: 1.0,
            sparse_left: false,
            font: [
                "/usr/share/fonts/noto/NotoColorEmoji.ttf",
                "/usr/share/fonts/truetype/noto/NotoColorEmoji.ttf",
                "/usr/local/share/fonts/NotoColorEmoji.ttf",
            ]
            .into_iter()
            .map(PathBuf::from)
            .find(|path| path.is_file())
            .unwrap_or_else(|| PathBuf::from("NotoColorEmoji.ttf")),
            output: "assets/textures/cherry-blossoms/blossoms-albedo.png".into(),
        };
        let mut args = std::env::args().skip(1);
        while let Some(arg) = args.next() {
            if arg == "--help" || arg == "-h" {
                println!(
                    "Generate a white background with clustered Noto 🌸 emojis.\n\
                     --width N          Width in pixels (default 2048)\n\
                     --height N         Height in pixels (default 2048)\n\
                     --seed N           Reproducible pattern seed (default 42)\n\
                     --density N        Blossom count multiplier, 0..10 (default 1)\n\
                     --sparse-side SIDE Almost-white side: left or right (default right)\n\
                     --font PATH        NotoColorEmoji.ttf (auto-detected on Linux)\n\
                     --output PATH      PNG path (default assets/textures/cherry-blossoms/blossoms-albedo.png)"
                );
                return Ok(None);
            }
            let value = args
                .next()
                .ok_or_else(|| format!("missing value for {arg}"))?;
            match arg.as_str() {
                "--width" => options.width = value.parse()?,
                "--height" => options.height = value.parse()?,
                "--seed" => options.seed = value.parse()?,
                "--density" => options.density = value.parse()?,
                "--font" => options.font = value.into(),
                "--output" => options.output = value.into(),
                "--sparse-side" => match value.as_str() {
                    "left" => options.sparse_left = true,
                    "right" => options.sparse_left = false,
                    _ => return Err("--sparse-side must be left or right".into()),
                },
                _ => return Err(format!("unknown option: {arg}").into()),
            }
        }
        if !(64..=8192).contains(&options.width) || !(64..=8192).contains(&options.height) {
            return Err("dimensions must be between 64 and 8192 pixels".into());
        }
        if !options.density.is_finite() || !(0.0..=10.0).contains(&options.density) {
            return Err("density must be finite and between 0 and 10".into());
        }
        Ok(Some(options))
    }
}

fn hash(mut n: u32) -> u32 {
    n ^= n >> 16;
    n = n.wrapping_mul(0x7feb352d);
    n ^= n >> 15;
    n = n.wrapping_mul(0x846ca68b);
    n ^ (n >> 16)
}

fn unit(n: u32) -> f32 {
    (n >> 8) as f32 / 16_777_216.0
}

struct Random(u32);
impl Random {
    fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_add(0x9e3779b9);
        unit(hash(self.0))
    }
}

fn smooth(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn noise(x: f32, y: f32, seed: u32) -> f32 {
    let ix = x.floor() as i32;
    let iy = y.floor() as i32;
    let sample = |dx, dy| {
        unit(hash(
            (ix + dx) as u32 ^ ((iy + dy) as u32).wrapping_mul(374761393) ^ seed,
        ))
    };
    let tx = smooth(0.0, 1.0, x.fract());
    let ty = smooth(0.0, 1.0, y.fract());
    let top = sample(0, 0) * (1.0 - tx) + sample(1, 0) * tx;
    let bottom = sample(0, 1) * (1.0 - tx) + sample(1, 1) * tx;
    top * (1.0 - ty) + bottom * ty
}

fn density(u: f32, v: f32, seed: u32) -> f32 {
    let clusters = 0.55 * noise(u * 5.0, v * 5.0, seed)
        + 0.3 * noise(u * 13.0, v * 13.0, seed ^ 12345)
        + 0.15 * noise(u * 31.0, v * 31.0, seed ^ 67890);
    let vertical = (1.0 - smooth(0.0, 0.96, v)).powf(1.8);
    let horizontal = (1.0 - smooth(0.02, 0.86, u)).powf(2.0);
    // Large flowers need much more breathing room. The middle band keeps a
    // healthy population of small flowers before the final fade to white.
    let spacing = (50.0 / blossom_size(u)).powf(1.65).min(1.4);
    vertical * horizontal * (0.15 + 1.2 * clusters) * spacing
}

fn blossom_size(u: f32) -> f32 {
    // About 8–10× the original size on the full side, then small in the
    // middle, tapering only gently again toward the sparse side.
    let middle_to_sparse = 56.0 - 22.0 * smooth(0.48, 0.85, u);
    middle_to_sparse + 340.0 * (1.0 - smooth(0.10, 0.38, u))
}

fn blossom(font: &std::path::Path) -> Result<RgbaImage> {
    let bytes = std::fs::read(font).map_err(|e| {
        format!(
            "cannot read {}: {e}; pass --font /path/to/NotoColorEmoji.ttf",
            font.display()
        )
    })?;
    let face = ttf_parser::Face::parse(&bytes, 0)?;
    let glyph = face.glyph_index('🌸').ok_or("font has no 🌸 glyph")?;
    let bitmap = face
        .glyph_raster_image(glyph, u16::MAX)
        .ok_or("font has no bitmap for 🌸; use the bitmap NotoColorEmoji.ttf font")?;
    if bitmap.format != ttf_parser::RasterImageFormat::PNG {
        return Err("expected a PNG color glyph; use NotoColorEmoji.ttf".into());
    }
    let mut image = image::load_from_memory(bitmap.data)?.to_rgba8();
    // Filter premultiplied color so transparent glyph borders do not create dark halos.
    for p in image.pixels_mut() {
        for channel in 0..3 {
            p[channel] = (p[channel] as f32 * p[3] as f32 / 255.0).round() as u8;
        }
    }
    Ok(image)
}

fn sample(image: &RgbaImage, x: f32, y: f32) -> [f32; 4] {
    let ix = x.floor() as i32;
    let iy = y.floor() as i32;
    let mut result = [0.0; 4];
    for dy in 0..=1 {
        for dx in 0..=1 {
            let px = ix + dx;
            let py = iy + dy;
            if px < 0 || py < 0 || px >= image.width() as i32 || py >= image.height() as i32 {
                continue;
            }
            let weight = (if dx == 0 { 1.0 - x.fract() } else { x.fract() })
                * (if dy == 0 { 1.0 - y.fract() } else { y.fract() });
            for (channel, value) in result.iter_mut().enumerate() {
                *value += image.get_pixel(px as u32, py as u32)[channel] as f32 * weight;
            }
        }
    }
    result
}

fn stamp(canvas: &mut RgbaImage, glyph: &RgbaImage, cx: f32, cy: f32, angle: f32, opacity: f32) {
    let (sin, cos) = angle.sin_cos();
    let half_width = glyph.width() as f32 * 0.5;
    let half_height = glyph.height() as f32 * 0.5;
    let radius = half_width.hypot(half_height).ceil() as i32;
    let center_x = cx.round() as i32;
    let center_y = cy.round() as i32;
    for y in (center_y - radius).max(0)..=(center_y + radius).min(canvas.height() as i32 - 1) {
        for x in (center_x - radius).max(0)..=(center_x + radius).min(canvas.width() as i32 - 1) {
            let dx = x as f32 - cx;
            let dy = y as f32 - cy;
            let sx = cos * dx + sin * dy + half_width - 0.5;
            let sy = -sin * dx + cos * dy + half_height - 0.5;
            if sx < 0.0
                || sy < 0.0
                || sx > glyph.width() as f32 - 1.0
                || sy > glyph.height() as f32 - 1.0
            {
                continue;
            }
            let color = sample(glyph, sx, sy);
            let alpha = color[3] / 255.0 * opacity;
            let dest = canvas.get_pixel_mut(x as u32, y as u32);
            for channel in 0..3 {
                dest[channel] = (color[channel] * opacity + dest[channel] as f32 * (1.0 - alpha))
                    .clamp(0.0, 255.0)
                    .round() as u8;
            }
        }
    }
}

fn grey_swatches(canvas: &mut RgbaImage) {
    // Five touching squares, flush with the bottom-left edge, dark to light.
    let side = (canvas.width().min(canvas.height()) / 16).max(1);
    for (index, shade) in [64, 104, 144, 184, 224].into_iter().enumerate() {
        for y in canvas.height() - side..canvas.height() {
            for x in index as u32 * side..(index as u32 + 1) * side {
                canvas.put_pixel(x, y, Rgba([shade, shade, shade, 255]));
            }
        }
    }
}

fn main() -> Result<()> {
    let Some(options) = Options::parse()? else {
        return Ok(());
    };
    let glyph = blossom(&options.font)?;
    // Enlarge every size equally while keeping the established placements and
    // large-to-small progression, so blossoms cover more of the white space.
    let scale = 2.0 * options.width.min(options.height) as f32 / 2048.0;
    // Cache a range of sizes before rotating each instance independently.
    let sprites: Vec<_> = (0..64)
        .map(|i| {
            // Log spacing preserves detail among the small sizes while also
            // covering the much larger foreground blossoms.
            let width = ((20.0 * 26.0_f32.powf(i as f32 / 63.0)) * scale)
                .round()
                .max(2.0) as u32;
            let height = (width as f32 * glyph.height() as f32 / glyph.width() as f32)
                .round()
                .max(2.0) as u32;
            image::imageops::resize(&glyph, width, height, FilterType::Triangle)
        })
        .collect();
    let mut canvas = RgbaImage::from_pixel(options.width, options.height, Rgba([255; 4]));
    let mut random = Random(options.seed);
    let mut count = 0;
    // Normalized budget keeps the composition similar at different resolutions.
    for _ in 0..(8500.0 * options.density) as usize {
        let u = random.next();
        let v = random.next();
        if random.next() >= density(u, v, options.seed) {
            continue;
        }
        let size_noise = noise(u * 8.0, v * 8.0, options.seed ^ 54321);
        let variation = 0.8 + 0.4 * (0.55 * random.next() + 0.45 * size_noise);
        let width = blossom_size(u) * variation;
        let size = ((width / 20.0).ln() / 26.0_f32.ln() * 63.0)
            .round()
            .clamp(0.0, 63.0) as usize;
        let angle = random.next() * std::f32::consts::TAU;
        let opacity = 0.45 + 0.55 * random.next();
        let x = if options.sparse_left { 1.0 - u } else { u };
        stamp(
            &mut canvas,
            &sprites[size],
            x * options.width as f32,
            v * options.height as f32,
            angle,
            opacity,
        );
        count += 1;
    }
    grey_swatches(&mut canvas);
    if let Some(parent) = options
        .output
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)?;
    }
    canvas.save(&options.output)?;
    println!(
        "wrote {} ({}×{}, {count} blossoms, seed {}, font {})",
        options.output.display(),
        options.width,
        options.height,
        options.seed,
        options.font.display()
    );
    Ok(())
}
