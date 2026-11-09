//! Bake a crescent's body/glow and pack them beside the existing cloud mask.
//! cargo run --release --example generate-twilight-moon -- [output-directory]
//! Packed PNG channels are linear data: R=cloud, G=body, B=glow, A=255.
//! The separate preview PNG is straight-alpha tinted art, not the runtime mask.
use image::{Rgba, RgbaImage};
use std::path::PathBuf;

const DOMAIN: f32 = 3.0;
const SUPERSAMPLE: u32 = 4;

fn crescent(p: [f32; 2]) -> bool {
    p[0] * p[0] + p[1] * p[1] <= 1.0 && (p[0] - 0.43).powi(2) + (p[1] - 0.14).powi(2) >= 0.94 * 0.94
}

fn body_mask(width: u32, height: u32) -> Vec<f32> {
    (0..height)
        .flat_map(|y| {
            (0..width).map(move |x| {
                let mut coverage = 0;
                for sy in 0..SUPERSAMPLE {
                    for sx in 0..SUPERSAMPLE {
                        let p = [
                            ((x as f32 + (sx as f32 + 0.5) / SUPERSAMPLE as f32) / width as f32
                                * 2.0
                                - 1.0)
                                * DOMAIN,
                            (1.0 - (y as f32 + (sy as f32 + 0.5) / SUPERSAMPLE as f32)
                                / height as f32
                                * 2.0)
                                * DOMAIN,
                        ];
                        coverage += u32::from(crescent(p));
                    }
                }
                coverage as f32 / (SUPERSAMPLE * SUPERSAMPLE) as f32
            })
        })
        .collect()
}

// Separate pixel sigmas keep the blur isotropic in moon coordinates even when
// the cloud panorama (and thus our packed image) is twice as wide as it is tall.
fn blur_axis(input: &[f32], width: u32, height: u32, sigma: f32, horizontal: bool) -> Vec<f32> {
    let radius = (sigma * 3.0).ceil() as i32;
    let mut kernel: Vec<f32> = (-radius..=radius)
        .map(|offset| (-0.5 * (offset as f32 / sigma).powi(2)).exp())
        .collect();
    let sum: f32 = kernel.iter().sum();
    kernel.iter_mut().for_each(|value| *value /= sum);
    let mut output = vec![0.0; input.len()];
    for y in 0..height as i32 {
        for x in 0..width as i32 {
            let mut value = 0.0;
            for (index, weight) in kernel.iter().enumerate() {
                let offset = index as i32 - radius;
                let xx = x + if horizontal { offset } else { 0 };
                let yy = y + if horizontal { 0 } else { offset };
                if xx >= 0 && yy >= 0 && xx < width as i32 && yy < height as i32 {
                    value += input[(yy as u32 * width + xx as u32) as usize] * weight;
                }
            }
            output[(y as u32 * width + x as u32) as usize] = value;
        }
    }
    output
}

fn moon_masks(width: u32, height: u32) -> (Vec<f32>, Vec<f32>) {
    let body = body_mask(width, height);
    let mut glow = vec![0.0; body.len()];
    for (sigma, strength) in [(0.06, 0.35), (0.18, 0.45), (0.35, 0.20)] {
        let x = blur_axis(
            &body,
            width,
            height,
            sigma * width as f32 / (2.0 * DOMAIN),
            true,
        );
        let xy = blur_axis(
            &x,
            width,
            height,
            sigma * height as f32 / (2.0 * DOMAIN),
            false,
        );
        for (value, blurred) in glow.iter_mut().zip(xy) {
            *value += blurred * strength;
        }
    }
    (body, glow)
}

fn byte(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn bake(clouds: &RgbaImage) -> (RgbaImage, RgbaImage) {
    let (width, height) = clouds.dimensions();
    let (body, glow) = moon_masks(width, height);
    let packed = RgbaImage::from_fn(width, height, |x, y| {
        let index = (y * width + x) as usize;
        Rgba([
            clouds.get_pixel(x, y)[0],
            byte(body[index]),
            byte(glow[index]),
            255,
        ])
    });
    // Square art preview; runtime channels deliberately share the panorama's
    // aspect ratio and are mapped back to square moon coordinates by the shader.
    let preview = RgbaImage::from_fn(height, height, |x, y| {
        let index = (y * width + x * width / height) as usize;
        let alpha = byte(body[index] + glow[index] * 0.8);
        if alpha == 0 {
            Rgba([0, 0, 0, 0])
        } else {
            Rgba([237, 226, 255, alpha])
        }
    });
    (packed, preview)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let directory = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("assets/textures/sky"));
    let clouds = image::open("assets/textures/space/sky-cloud-mask.png")?.to_rgba8();
    std::fs::create_dir_all(&directory)?;
    let (packed, preview) = bake(&clouds);
    packed.save(directory.join("kawaii-twilight-masks.png"))?;
    preview.save(directory.join("crescent-moon-glow.png"))?;
    println!(
        "wrote twilight masks and crescent preview to {}",
        directory.display()
    );
    Ok(())
}

#[test]
fn bake_preserves_clouds_and_has_padded_crescent_shaped_glow() {
    let clouds = RgbaImage::from_fn(192, 96, |x, y| Rgba([(x + y) as u8, 0, 0, 255]));
    let (packed, preview) = bake(&clouds);
    assert_eq!(packed, bake(&clouds).0, "deterministic bake");
    for (a, b) in packed.pixels().zip(clouds.pixels()) {
        assert_eq!(a[0], b[0]);
        assert_eq!(a[3], 255);
    }
    assert!(packed.pixels().any(|p| p[1] > 0 && p[1] < 255));
    assert!(packed.pixels().any(|p| p[1] == 255));
    assert!(packed.pixels().any(|p| p[1] == 0 && p[2] > 0));
    for x in 0..preview.width() {
        for y in [0, preview.height() - 1] {
            assert_eq!(preview.get_pixel(x, y)[3], 0);
        }
    }
    for y in 0..preview.height() {
        for x in [0, preview.width() - 1] {
            assert_eq!(preview.get_pixel(x, y)[3], 0);
        }
    }
    // At equal distances from the disk center, the halo must favor the actual
    // crescent on the left, not form the old symmetric disk-centered halo.
    assert!(packed.get_pixel(69, 48)[2] > packed.get_pixel(122, 48)[2]);
    assert_eq!(packed.get_pixel(102, 48)[1], 0, "cutout stays empty");
}
