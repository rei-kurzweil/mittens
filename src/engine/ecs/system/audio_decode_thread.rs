//! Audio decoding worker thread.
//!
//! Owns the symphonia decoder. Receives `LoadClipRequest`s from the main
//! thread via mpsc and ships completed assets to the audio render thread
//! over a wait-free `rtrb` producer.
//!
//! Phase 5 ships full-buffer messages only (short clips). Streaming for
//! long BGM lands when needed — same protocol, smaller chunks.

use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::thread::JoinHandle;

use super::audio_decode::{decode_audio_file, DecodeError};
use super::audio_sample_format_convert::{convert_sample_format, ConvertError, PlaybackFormat};

/// Request from main → decode thread.
#[derive(Debug)]
pub struct LoadClipRequest {
    pub clip_id: u64,
    pub uri: String,
    pub target: PlaybackFormat,
}

/// Completion message from decode → RT thread.
#[derive(Debug, Clone)]
pub enum LoadedClipMessage {
    Loaded {
        clip_id: u64,
        samples: Arc<Vec<f32>>,
        channels: u16,
        sample_rate: u32,
    },
    Failed {
        clip_id: u64,
        reason: String,
    },
}

/// Sender + handle to the worker.
pub struct DecodeThreadHandle {
    pub tx: Sender<LoadClipRequest>,
    pub thread: Option<JoinHandle<()>>,
}

impl DecodeThreadHandle {
    pub fn shutdown(mut self) {
        drop(self.tx);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// Spawn the worker. Completion messages go to `complete_tx` (main
/// thread). The main thread forwards them to the audio RT thread.
pub fn spawn_decode_thread(complete_tx: Sender<LoadedClipMessage>) -> DecodeThreadHandle {
    let (tx, rx): (Sender<LoadClipRequest>, Receiver<LoadClipRequest>) = channel();

    let thread = std::thread::Builder::new()
        .name("cat-engine-audio-decode".into())
        .spawn(move || worker_main(rx, complete_tx))
        .expect("failed to spawn audio decode thread");

    DecodeThreadHandle {
        tx,
        thread: Some(thread),
    }
}

fn worker_main(rx: Receiver<LoadClipRequest>, complete_tx: Sender<LoadedClipMessage>) {
    while let Ok(req) = rx.recv() {
        let LoadClipRequest {
            clip_id,
            uri,
            target,
        } = req;

        let msg = match decode_and_convert(&uri, target) {
            Ok((samples, channels, sample_rate)) => LoadedClipMessage::Loaded {
                clip_id,
                samples,
                channels,
                sample_rate,
            },
            Err(reason) => LoadedClipMessage::Failed { clip_id, reason },
        };
        // If the receiver has been dropped (e.g. shutdown), exit quietly.
        if complete_tx.send(msg).is_err() {
            break;
        }
    }
}

fn decode_and_convert(
    uri: &str,
    target: PlaybackFormat,
) -> Result<(Arc<Vec<f32>>, u16, u32), String> {
    let decoded = decode_audio_file(uri).map_err(|e: DecodeError| e.to_string())?;
    let converted =
        convert_sample_format(decoded, target).map_err(|e: ConvertError| e.to_string())?;
    Ok((converted.samples, converted.channels, converted.sample_rate))
}
