//! Sample-rate conversion + channel remix.
//!
//! Decode is upstream ([`super::audio_decode`]); loudness normalization is
//! downstream (gain-policy at the playback graph). This stage is
//! deterministic and data-format-driven only — no gain changes.

use super::audio_decode::DecodedAudio;

#[derive(Debug, Clone, Copy)]
pub struct PlaybackFormat {
    pub sample_rate: u32,
    pub channels: u16,
}

/// PCM in the engine's playback format. Interleaved frame-major.
#[derive(Debug, Clone)]
pub struct ConvertedAudio {
    pub samples: std::sync::Arc<Vec<f32>>,
    pub channels: u16,
    pub sample_rate: u32,
}

#[derive(Debug, Clone)]
pub enum ConvertError {
    InvalidTarget(String),
    EmptyInput,
}

impl std::fmt::Display for ConvertError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConvertError::InvalidTarget(s) => write!(f, "invalid target: {s}"),
            ConvertError::EmptyInput => write!(f, "empty input"),
        }
    }
}

impl std::error::Error for ConvertError {}

/// Convert `decoded` into the engine's playback format. Channel remix is
/// applied before resample so the resampler sees the right channel count.
pub fn convert_sample_format(
    decoded: DecodedAudio,
    target: PlaybackFormat,
) -> Result<ConvertedAudio, ConvertError> {
    if target.sample_rate == 0 || target.channels == 0 {
        return Err(ConvertError::InvalidTarget(format!(
            "{target:?}"
        )));
    }
    if decoded.samples.is_empty() {
        return Err(ConvertError::EmptyInput);
    }

    let after_channels = remix_channels(&decoded.samples, decoded.channels, target.channels);
    let after_resample = if decoded.sample_rate == target.sample_rate {
        after_channels
    } else {
        resample_linear(
            &after_channels,
            target.channels,
            decoded.sample_rate,
            target.sample_rate,
        )
    };

    Ok(ConvertedAudio {
        samples: std::sync::Arc::new(after_resample),
        channels: target.channels,
        sample_rate: target.sample_rate,
    })
}

fn remix_channels(samples: &[f32], src: u16, dst: u16) -> Vec<f32> {
    if src == dst {
        return samples.to_vec();
    }
    let src_n = src as usize;
    let dst_n = dst as usize;
    let frames = samples.len() / src_n.max(1);

    if dst == 1 {
        // Downmix to mono via per-frame average.
        let mut out = Vec::with_capacity(frames);
        for f in 0..frames {
            let base = f * src_n;
            let mut sum = 0.0f32;
            for c in 0..src_n {
                sum += samples[base + c];
            }
            out.push(sum / src_n as f32);
        }
        out
    } else if src == 1 {
        // Upmix mono → multi by duplicating into every output channel.
        let mut out = Vec::with_capacity(frames * dst_n);
        for f in 0..frames {
            let s = samples[f];
            for _ in 0..dst_n {
                out.push(s);
            }
        }
        out
    } else {
        // General N→M: pick first `min(src, dst)` channels, zero-fill the
        // rest. Good-enough placeholder until a proper down/upmix matrix is
        // needed.
        let copy_n = src_n.min(dst_n);
        let mut out = Vec::with_capacity(frames * dst_n);
        for f in 0..frames {
            let base = f * src_n;
            for c in 0..copy_n {
                out.push(samples[base + c]);
            }
            for _ in copy_n..dst_n {
                out.push(0.0);
            }
        }
        out
    }
}

/// Linear resampler — interleaved PCM in, interleaved PCM out. Good enough
/// for phase 5; an upgrade to a sinc resampler can swap in later behind
/// the same signature.
fn resample_linear(samples: &[f32], channels: u16, src_sr: u32, dst_sr: u32) -> Vec<f32> {
    if src_sr == dst_sr {
        return samples.to_vec();
    }
    let ch = channels as usize;
    let in_frames = samples.len() / ch.max(1);
    if in_frames < 2 {
        return samples.to_vec();
    }

    let ratio = dst_sr as f64 / src_sr as f64;
    let out_frames = ((in_frames as f64) * ratio).round() as usize;
    let mut out = Vec::with_capacity(out_frames * ch);

    for f in 0..out_frames {
        let src_pos = f as f64 / ratio;
        let i0 = src_pos.floor() as usize;
        let i1 = (i0 + 1).min(in_frames - 1);
        let t = (src_pos - i0 as f64) as f32;
        for c in 0..ch {
            let s0 = samples[i0 * ch + c];
            let s1 = samples[i1 * ch + c];
            out.push(s0 + (s1 - s0) * t);
        }
    }
    out
}
