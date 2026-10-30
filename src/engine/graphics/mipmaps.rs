//! CPU-generated RGBA8 mip levels, created once per uploaded texture.
pub struct Level {
    pub offset: usize,
    pub width: u32,
    pub height: u32,
}

fn decode(value: u8, srgb: bool) -> f32 {
    let value = value as f32 / 255.0;
    if !srgb {
        value
    } else if value <= 0.04045 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}
fn encode(value: f32, srgb: bool) -> u8 {
    let value = value.clamp(0.0, 1.0);
    let value = if !srgb {
        value
    } else if value <= 0.0031308 {
        value * 12.92
    } else {
        1.055 * value.powf(1.0 / 2.4) - 0.055
    };
    (value * 255.0).round() as u8
}

/// Generate a complete mip chain from tightly packed RGBA8 pixels.
pub fn generate(
    rgba: &[u8],
    width: u32,
    height: u32,
    srgb: bool,
) -> Result<(Vec<u8>, Vec<Level>), String> {
    let expected = (width as usize)
        .checked_mul(height as usize)
        .and_then(|n| n.checked_mul(4));
    if width == 0 || height == 0 || expected != Some(rgba.len()) {
        return Err("invalid RGBA8 texture dimensions/data".into());
    }
    let mut bytes = rgba.to_vec();
    let mut levels = vec![Level {
        offset: 0,
        width,
        height,
    }];
    let (mut w, mut h) = (width, height);
    while w > 1 || h > 1 {
        let (nw, nh) = ((w / 2).max(1), (h / 2).max(1));
        let src = levels.last().unwrap().offset;
        let mut next = Vec::with_capacity(nw as usize * nh as usize * 4);
        for y in 0..nh {
            for x in 0..nw {
                // Area-weighted box filtering includes all texels for odd extents.
                let (x0, x1) = (
                    x as f32 * w as f32 / nw as f32,
                    (x + 1) as f32 * w as f32 / nw as f32,
                );
                let (y0, y1) = (
                    y as f32 * h as f32 / nh as f32,
                    (y + 1) as f32 * h as f32 / nh as f32,
                );
                let mut sum = [0.0; 4];
                let mut total = 0.0;
                for sy in y0.floor() as u32..(y1.ceil() as u32).min(h) {
                    for sx in x0.floor() as u32..(x1.ceil() as u32).min(w) {
                        let weight = (x1.min((sx + 1) as f32) - x0.max(sx as f32))
                            * (y1.min((sy + 1) as f32) - y0.max(sy as f32));
                        let offset = src + (sy as usize * w as usize + sx as usize) * 4;
                        for channel in 0..4 {
                            sum[channel] +=
                                decode(bytes[offset + channel], srgb && channel < 3) * weight;
                        }
                        total += weight;
                    }
                }
                for channel in 0..4 {
                    next.push(encode(sum[channel] / total, srgb && channel < 3));
                }
            }
        }
        levels.push(Level {
            offset: bytes.len(),
            width: nw,
            height: nh,
        });
        bytes.extend_from_slice(&next);
        (w, h) = (nw, nh);
    }
    Ok((bytes, levels))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn odd_dimensions_include_edge_texels_and_preserve_alpha() {
        let mut source = vec![0u8; 3 * 3 * 4];
        source[(3 * 3 - 1) * 4] = 255;
        for pixel in source.chunks_exact_mut(4) {
            pixel[3] = 255;
        }
        let (data, levels) = generate(&source, 3, 3, false).unwrap();
        assert_eq!((levels.len(), levels[1].width, levels[1].height), (2, 1, 1));
        assert_eq!(&data[levels[1].offset..], &[28, 0, 0, 255]);
    }
    #[test]
    fn srgb_averages_in_linear_light() {
        let (data, levels) = generate(&[0, 0, 0, 0, 255, 255, 255, 255], 2, 1, true).unwrap();
        assert_eq!(&data[levels[1].offset..], &[188, 188, 188, 128]);
    }
    #[test]
    fn rejects_invalid_dimensions_and_data() {
        assert!(generate(&[], 0, 1, false).is_err());
        assert!(generate(&[0; 4], 2, 1, false).is_err());
    }
}
