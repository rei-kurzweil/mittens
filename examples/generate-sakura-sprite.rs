//! Render Noto Color Emoji's 🌸 into a 256×256 straight-alpha texture.
//! cargo run --release --example generate-sakura-sprite -- [--font PATH] [--output PATH]
use image::{Rgba, RgbaImage, imageops::FilterType};
use std::{error::Error, path::PathBuf};

fn main() -> Result<(), Box<dyn Error>> {
    let mut font = [
        "/usr/share/fonts/noto/NotoColorEmoji.ttf",
        "/usr/share/fonts/truetype/noto/NotoColorEmoji.ttf",
        "/usr/local/share/fonts/NotoColorEmoji.ttf",
    ]
    .into_iter()
    .map(PathBuf::from)
    .find(|path| path.is_file())
    .unwrap_or_else(|| PathBuf::from("NotoColorEmoji.ttf"));
    let mut output = PathBuf::from("assets/textures/cherry-blossoms/sakura-blossom-256.png");
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--font" => font = args.next().ok_or("--font requires a path")?.into(),
            "--output" => output = args.next().ok_or("--output requires a path")?.into(),
            "--help" | "-h" => {
                println!(
                    "Generate a transparent 256×256 Noto 🌸 texture.\n--font PATH  Bitmap NotoColorEmoji.ttf (auto-detected)\n--output PATH  PNG destination"
                );
                return Ok(());
            }
            _ => return Err(format!("unknown option: {arg}").into()),
        }
    }
    let font_bytes = std::fs::read(&font).map_err(|error| {
        format!(
            "{}: {error}; pass --font /path/to/NotoColorEmoji.ttf",
            font.display()
        )
    })?;
    let face = ttf_parser::Face::parse(&font_bytes, 0)?;
    let glyph = face.glyph_index('🌸').ok_or("font has no 🌸 glyph")?;
    let bitmap = face
        .glyph_raster_image(glyph, u16::MAX)
        .ok_or("use bitmap NotoColorEmoji.ttf (PNG glyph required)")?;
    if bitmap.format != ttf_parser::RasterImageFormat::PNG {
        return Err("expected PNG color glyph".into());
    }
    let glyph = image::load_from_memory(bitmap.data)?.to_rgba8();
    let mut bounds = [glyph.width(), glyph.height(), 0, 0];
    for (x, y, pixel) in glyph.enumerate_pixels().filter(|(_, _, p)| p[3] > 0) {
        let _ = pixel;
        bounds[0] = bounds[0].min(x);
        bounds[1] = bounds[1].min(y);
        bounds[2] = bounds[2].max(x);
        bounds[3] = bounds[3].max(y);
    }
    if bounds[0] > bounds[2] || bounds[1] > bounds[3] {
        return Err("glyph is empty".into());
    }
    let mut cropped = image::imageops::crop_imm(
        &glyph,
        bounds[0],
        bounds[1],
        bounds[2] - bounds[0] + 1,
        bounds[3] - bounds[1] + 1,
    )
    .to_image();
    // Resize premultiplied channels, then restore straight alpha for the renderer.
    for pixel in cropped.pixels_mut() {
        for c in 0..3 {
            pixel[c] = (pixel[c] as f32 * pixel[3] as f32 / 255.).round() as u8;
        }
    }
    let scale = 224. / cropped.width().max(cropped.height()) as f32;
    let width = (cropped.width() as f32 * scale).round() as u32;
    let height = (cropped.height() as f32 * scale).round() as u32;
    let mut sprite = image::imageops::resize(&cropped, width, height, FilterType::Lanczos3);
    for pixel in sprite.pixels_mut() {
        if pixel[3] > 0 {
            for c in 0..3 {
                pixel[c] = (pixel[c] as f32 * 255. / pixel[3] as f32)
                    .clamp(0., 255.)
                    .round() as u8;
            }
        }
    }
    let mut canvas = RgbaImage::from_pixel(256, 256, Rgba([255, 183, 213, 0]));
    image::imageops::replace(
        &mut canvas,
        &sprite,
        ((256 - width) / 2) as i64,
        ((256 - height) / 2) as i64,
    );
    // Bleed nearby edge RGB into transparent texels without changing alpha.
    // Bilinear sampling then avoids a black fringe around petals.
    let mut valid: Vec<bool> = canvas.pixels().map(|p| p[3] > 0).collect();
    for _ in 0..8 {
        let previous = canvas.clone();
        let previous_valid = valid.clone();
        for y in 0..256i32 {
            for x in 0..256i32 {
                let index = (y * 256 + x) as usize;
                if previous_valid[index] {
                    continue;
                }
                if let Some((nx, ny)) = [(x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)]
                    .into_iter()
                    .find(|&(nx, ny)| {
                        (0..256).contains(&nx)
                            && (0..256).contains(&ny)
                            && previous_valid[(ny * 256 + nx) as usize]
                    })
                {
                    let source = previous.get_pixel(nx as u32, ny as u32);
                    let target = canvas.get_pixel_mut(x as u32, y as u32);
                    target.0[..3].copy_from_slice(&source.0[..3]);
                    valid[index] = true;
                }
            }
        }
    }
    if let Some(parent) = output.parent().filter(|path| !path.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    canvas.save(&output)?;
    println!(
        "wrote {} (256×256 transparent Noto 🌸, font {})",
        output.display(),
        font.display()
    );
    Ok(())
}
