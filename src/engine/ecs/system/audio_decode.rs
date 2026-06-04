//! PCM decode via symphonia.
//!
//! Decode-only — sample-rate conversion and channel remix live in
//! [`super::audio_sample_format_convert`]; loudness normalization is a
//! separate gain-policy stage on the audio render thread.
//! See docs/spec/audio-sources.md and docs/task/audio-decode-convert-normalize-split.md.

use std::fs::File;
use std::path::Path;

use symphonia::core::audio::{AudioBufferRef, Signal};
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use symphonia::core::sample::Sample;

/// Decoded PCM in the source asset's native format. Interleaved `f32`
/// regardless of the encoded sample type — sample-format conversion is
/// the next stage's job. Multi-channel data is interleaved frame-major.
#[derive(Debug, Clone)]
pub struct DecodedAudio {
    pub samples: Vec<f32>,
    pub channels: u16,
    pub sample_rate: u32,
}

#[derive(Debug, Clone)]
pub enum DecodeError {
    OpenFailed(String),
    ProbeFailed(String),
    NoTrack,
    DecoderInitFailed(String),
    DecodeFailed(String),
    Unsupported(String),
}

impl std::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DecodeError::OpenFailed(s) => write!(f, "open failed: {s}"),
            DecodeError::ProbeFailed(s) => write!(f, "probe failed: {s}"),
            DecodeError::NoTrack => write!(f, "no default track"),
            DecodeError::DecoderInitFailed(s) => write!(f, "decoder init failed: {s}"),
            DecodeError::DecodeFailed(s) => write!(f, "decode failed: {s}"),
            DecodeError::Unsupported(s) => write!(f, "unsupported: {s}"),
        }
    }
}

impl std::error::Error for DecodeError {}

/// Decode a complete audio file into interleaved `f32` PCM.
pub fn decode_audio_file(path: impl AsRef<Path>) -> Result<DecodedAudio, DecodeError> {
    let path = path.as_ref();
    let file = File::open(path).map_err(|e| DecodeError::OpenFailed(e.to_string()))?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .map_err(|e| DecodeError::ProbeFailed(e.to_string()))?;

    let mut format = probed.format;
    let track = format.default_track().ok_or(DecodeError::NoTrack)?;
    let track_id = track.id;
    let codec_params = track.codec_params.clone();
    let sample_rate = codec_params
        .sample_rate
        .ok_or_else(|| DecodeError::Unsupported("missing sample_rate".into()))?;
    let channels = codec_params
        .channels
        .map(|c| c.count() as u16)
        .ok_or_else(|| DecodeError::Unsupported("missing channel layout".into()))?;

    let mut decoder = symphonia::default::get_codecs()
        .make(&codec_params, &DecoderOptions::default())
        .map_err(|e| DecodeError::DecoderInitFailed(e.to_string()))?;

    let mut samples: Vec<f32> = Vec::new();

    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(SymphoniaError::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                break;
            }
            Err(SymphoniaError::ResetRequired) => {
                // The decoder requested reset; restart and continue from current point.
                decoder.reset();
                continue;
            }
            Err(e) => return Err(DecodeError::DecodeFailed(e.to_string())),
        };
        if packet.track_id() != track_id {
            continue;
        }

        let decoded = match decoder.decode(&packet) {
            Ok(d) => d,
            Err(SymphoniaError::DecodeError(_)) => {
                // Skip undecodable packet; keep going.
                continue;
            }
            Err(e) => return Err(DecodeError::DecodeFailed(e.to_string())),
        };

        interleave_into(&decoded, &mut samples);
    }

    Ok(DecodedAudio {
        samples,
        channels,
        sample_rate,
    })
}

/// Push interleaved f32 frames from a typed AudioBufferRef into `out`.
fn interleave_into(buf: &AudioBufferRef<'_>, out: &mut Vec<f32>) {
    match buf {
        AudioBufferRef::F32(b) => interleave_planar::<f32>(b, out),
        AudioBufferRef::F64(b) => interleave_planar::<f64>(b, out),
        AudioBufferRef::S8(b) => interleave_planar::<i8>(b, out),
        AudioBufferRef::S16(b) => interleave_planar::<i16>(b, out),
        AudioBufferRef::S24(b) => interleave_planar::<symphonia::core::sample::i24>(b, out),
        AudioBufferRef::S32(b) => interleave_planar::<i32>(b, out),
        AudioBufferRef::U8(b) => interleave_planar::<u8>(b, out),
        AudioBufferRef::U16(b) => interleave_planar::<u16>(b, out),
        AudioBufferRef::U24(b) => interleave_planar::<symphonia::core::sample::u24>(b, out),
        AudioBufferRef::U32(b) => interleave_planar::<u32>(b, out),
    }
}

fn interleave_planar<S>(buf: &symphonia::core::audio::AudioBuffer<S>, out: &mut Vec<f32>)
where
    S: Sample + symphonia::core::conv::IntoSample<f32>,
{
    let channels = buf.spec().channels.count();
    let frames = buf.frames();
    out.reserve(frames * channels);
    for f in 0..frames {
        for c in 0..channels {
            let s: S = buf.chan(c)[f];
            out.push(s.into_sample());
        }
    }
}
