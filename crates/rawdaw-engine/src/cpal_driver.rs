//! Real-time audio driver backed by [`cpal`].
//!
//! Owns a cpal output `Stream` whose callback consumes the engine's
//! audio side via [`AudioEngine::process_block`]. The callback runs on
//! the OS audio thread; all per-block work is allocation-free and
//! lock-free (commands and events come in over rtrb SPSC ring buffers,
//! garbage flows out the same way).
//!
//! Lifecycle:
//!
//! 1. Call [`CpalDriver::probe_default_sample_rate`] to discover the
//!    output device's preferred rate.
//! 2. Build an [`Engine`] at that rate.
//! 3. Configure the engine (add nodes, connect, etc.), then call
//!    [`Engine::split`] to obtain `(AudioEngine, EngineHandle)`.
//! 4. Build the driver with [`CpalDriver::new`]; the audio side moves
//!    into the callback.
//! 5. Call [`CpalDriver::play`] to start the stream; push commands /
//!    events through the retained `EngineHandle`.
//!
//! v1 supports stereo f32 output only. Other cpal sample formats
//! return [`CpalDriverError::UnsupportedSampleFormat`]; broaden support
//! when a real device demands it.
//!
//! [`Engine`]: crate::engine::Engine
//! [`Engine::split`]: crate::engine::Engine::split

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{
    BuildStreamError, DefaultStreamConfigError, PauseStreamError, PlayStreamError, SampleFormat,
    Stream, StreamConfig,
};

// Re-exported so downstream crates can wire an error handler without
// pulling in `cpal` directly.
pub use cpal::StreamError;

use rawdaw_model::MusicalTime;

use crate::audio_engine::AudioEngine;
use crate::buffer::BufferMut;
use crate::context::ProcessContext;
use crate::graph::NodeId;
use crate::transport::Transport;

#[derive(Debug)]
pub enum CpalDriverError {
    NoOutputDevice,
    DefaultConfig(DefaultStreamConfigError),
    BuildStream(BuildStreamError),
    UnsupportedSampleFormat(SampleFormat),
    SampleRateMismatch { engine: u32, device: u32 },
    Play(PlayStreamError),
    Pause(PauseStreamError),
}

impl core::fmt::Display for CpalDriverError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NoOutputDevice => write!(f, "no default cpal output device available"),
            Self::DefaultConfig(e) => write!(f, "querying default output config failed: {e}"),
            Self::BuildStream(e) => write!(f, "building cpal output stream failed: {e}"),
            Self::UnsupportedSampleFormat(fmt) => {
                write!(f, "cpal device uses unsupported sample format {fmt:?}")
            }
            Self::SampleRateMismatch { engine, device } => write!(
                f,
                "engine sample rate {engine} doesn't match device rate {device}; \
                 construct the engine with the device's rate or pick a different device",
            ),
            Self::Play(e) => write!(f, "starting cpal stream failed: {e}"),
            Self::Pause(e) => write!(f, "pausing cpal stream failed: {e}"),
        }
    }
}

impl std::error::Error for CpalDriverError {}

/// Real-time audio driver. Holds the cpal `Stream`; dropping the driver
/// closes the stream and tears down the audio callback.
pub struct CpalDriver {
    stream: Stream,
    sample_rate: u32,
    channels: u16,
}

impl CpalDriver {
    /// Query the picked output device's preferred sample rate so the
    /// caller can build an `Engine` at the matching rate before
    /// constructing the driver. Uses the same picker as [`Self::new`],
    /// so probe and construction always agree on the device.
    pub fn probe_default_sample_rate() -> Result<u32, CpalDriverError> {
        let picked = pick_output_device()?;
        Ok(picked.config.sample_rate.0)
    }

    /// Open an f32 output device, build a stream wrapping `audio_engine`,
    /// and return a paused driver. The stream is paused at construction;
    /// call [`Self::play`] to start audio.
    ///
    /// Device selection prefers cpal's default device when its
    /// `default_output_config()` query succeeds; otherwise falls back to
    /// the first enumerated device with an f32 config. This matters on
    /// Linux: stock Kubuntu / Ubuntu desktops with PipeWire don't ship
    /// `pipewire-alsa` by default, leaving ALSA's `default` PCM bound
    /// to a non-working dmix→HDMI path. The fallback finds the `pulse`
    /// PCM (PipeWire's PulseAudio compatibility), which is universally
    /// present alongside `pipewire-pulse`.
    ///
    /// `master` is the graph node whose stereo output is routed to the
    /// device. It must produce a stereo port at index 0.
    ///
    /// `error_handler` is invoked for non-fatal stream errors emitted by
    /// cpal (device-disconnect, backend hiccups, etc). The closure runs
    /// on a cpal-managed thread, not the audio callback thread — locks
    /// and channel sends are fine. The driver does **not** swallow
    /// errors; the host gets to decide whether to log, display, or
    /// retry. Pass a no-op closure if errors should be ignored.
    pub fn new<F>(
        audio_engine: AudioEngine,
        master: NodeId,
        error_handler: F,
    ) -> Result<Self, CpalDriverError>
    where
        F: FnMut(StreamError) + Send + 'static,
    {
        let picked = pick_output_device()?;
        let PickedDevice {
            device,
            config,
            format,
        } = picked;
        let device_rate = config.sample_rate.0;
        let engine_rate = audio_engine.graph().sample_rate();
        if device_rate != engine_rate {
            return Err(CpalDriverError::SampleRateMismatch {
                engine: engine_rate,
                device: device_rate,
            });
        }
        let channels = config.channels;

        if format != SampleFormat::F32 {
            return Err(CpalDriverError::UnsupportedSampleFormat(format));
        }

        let stream = build_f32_stream(&device, &config, audio_engine, master, error_handler)?;
        Ok(Self {
            stream,
            sample_rate: device_rate,
            channels,
        })
    }

    pub fn play(&self) -> Result<(), CpalDriverError> {
        self.stream.play().map_err(CpalDriverError::Play)
    }

    pub fn pause(&self) -> Result<(), CpalDriverError> {
        self.stream.pause().map_err(CpalDriverError::Pause)
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn channels(&self) -> u16 {
        self.channels
    }
}

fn build_f32_stream<F>(
    device: &cpal::Device,
    config: &StreamConfig,
    mut audio_engine: AudioEngine,
    master: NodeId,
    error_handler: F,
) -> Result<Stream, CpalDriverError>
where
    F: FnMut(StreamError) + Send + 'static,
{
    let sample_rate = config.sample_rate.0;
    let device_channels = config.channels as usize;
    let max_block = audio_engine.graph().max_block_size();
    let transport = audio_engine.transport_handle();

    // Planar scratch sized for the engine's stride. Lives in the
    // closure for the stream's lifetime; allocated once on this thread,
    // then handed off into the audio callback as part of the closure's
    // moved state. No allocations occur in the callback after this.
    let mut scratch: Vec<f32> = vec![0.0; 2 * max_block];
    let mut absolute_time: u64 = 0;

    device
        .build_output_stream::<f32, _, _>(
            config,
            move |out: &mut [f32], _info| {
                let total_frames = out.len() / device_channels;
                if total_frames == 0 {
                    return;
                }

                // Read transport once per callback. Stopped resets the
                // driver's local time so the next Play starts from 0;
                // Paused freezes it. The engine performs its own
                // gating per-state (event drain, output clear,
                // sample_clock store) inside `process_block`.
                let state = transport.get();
                if matches!(state, Transport::Stopped) {
                    absolute_time = 0;
                }
                let playing = matches!(state, Transport::Playing);

                // cpal's per-callback buffer can be larger than the
                // engine's `max_block_size` (pulse typically asks for
                // 1024+ frames; max_block_size is 256). Loop until we
                // fill `out`, calling `process_block` once per
                // sub-block so the engine never sees more than
                // `max_block` frames at a time and the back of `out`
                // never plays stale data.
                let mut frames_done = 0usize;
                while frames_done < total_frames {
                    let this_block = (total_frames - frames_done).min(max_block);

                    // Zero the engine scratch's active region for both channels.
                    for ch in 0..2 {
                        let start = ch * max_block;
                        for s in scratch[start..start + this_block].iter_mut() {
                            *s = 0.0;
                        }
                    }

                    let buf = BufferMut::new(&mut scratch, 2, this_block, max_block);
                    let ctx = ProcessContext {
                        sample_rate,
                        block_size: this_block,
                        absolute_time_samples: absolute_time,
                        musical_time: MusicalTime::ZERO,
                        bpm: 120.0,
                        playing,
                    };
                    audio_engine.process_block(master, buf, ctx);

                    // Interleave into the correct slice of `out`. We
                    // build a temporary sub-slice rather than letting
                    // `interleave_stereo_into_device` index from zero,
                    // so the function stays simple.
                    let out_start = frames_done * device_channels;
                    let out_end = out_start + this_block * device_channels;
                    interleave_stereo_into_device(
                        &scratch,
                        max_block,
                        this_block,
                        device_channels,
                        &mut out[out_start..out_end],
                    );

                    // Advance the driver's wall clock only while Playing.
                    // Paused freezes it; Stopped was reset above and stays
                    // at 0 until the next state change.
                    if playing {
                        absolute_time = absolute_time.saturating_add(this_block as u64);
                    }
                    frames_done += this_block;
                }
            },
            error_handler,
            None,
        )
        .map_err(CpalDriverError::BuildStream)
}

/// Convert the engine's planar stereo output into the device's
/// interleaved buffer.
///
/// `planar[0..stride]` is left, `planar[stride..2*stride]` is right;
/// only the first `frames` samples per channel are valid. The device
/// buffer is interleaved with `device_channels` per frame; channels
/// beyond the first two are zeroed (the engine has no mapping for
/// surround in v1). A mono device gets `0.5 * (L + R)`.
fn interleave_stereo_into_device(
    planar: &[f32],
    stride: usize,
    frames: usize,
    device_channels: usize,
    out: &mut [f32],
) {
    let (left, right) = planar.split_at(stride);
    for i in 0..frames {
        let l = left[i];
        let r = right[i];
        let base = i * device_channels;
        match device_channels {
            0 => {}
            1 => out[base] = 0.5 * (l + r),
            _ => {
                out[base] = l;
                out[base + 1] = r;
                for c in 2..device_channels {
                    out[base + c] = 0.0;
                }
            }
        }
    }
}

/// An output device that's been confirmed to expose a usable
/// `default_output_config()`. Returned by [`pick_output_device`].
struct PickedDevice {
    device: cpal::Device,
    config: StreamConfig,
    format: SampleFormat,
}

/// Find a working output device.
///
/// Strategy:
///
/// 1. Try `host.default_output_device()` and query its config. If both
///    succeed, that's the pick — matches macOS / Windows and Linux
///    setups where ALSA's `default` PCM is healthy.
/// 2. Otherwise enumerate `host.output_devices()` and return the first
///    one whose `default_output_config()` returns Ok AND uses f32.
///    Captures the stock-Kubuntu case where ALSA's `default` is bound
///    to dmix→HDMI but `pulse` (PipeWire's PulseAudio compat) is
///    enumerable and works.
/// 3. If neither succeeds, return `NoOutputDevice`.
fn pick_output_device() -> Result<PickedDevice, CpalDriverError> {
    let host = cpal::default_host();

    // Path 1: trust the default device if its config query succeeds.
    if let Some(device) = host.default_output_device()
        && let Ok(supported) = device.default_output_config()
    {
        let format = supported.sample_format();
        return Ok(PickedDevice {
            device,
            config: supported.config(),
            format,
        });
    }

    // Path 2: enumerate; take the first f32 device with a working
    // config. cpal exposes `pulse` here on PipeWire/PulseAudio
    // systems — that's what we end up with on stock Kubuntu.
    let devices = host
        .output_devices()
        .map_err(|_| CpalDriverError::NoOutputDevice)?;
    let mut first_non_f32: Option<PickedDevice> = None;
    for device in devices {
        let Ok(supported) = device.default_output_config() else {
            continue;
        };
        let format = supported.sample_format();
        let cfg = supported.config();
        let picked = PickedDevice {
            device,
            config: cfg,
            format,
        };
        if format == SampleFormat::F32 {
            return Ok(picked);
        }
        if first_non_f32.is_none() {
            first_non_f32 = Some(picked);
        }
    }
    // No f32 device found — surface the first non-f32 we saw so the
    // caller can report `UnsupportedSampleFormat` with a real format,
    // rather than the more generic `NoOutputDevice`.
    first_non_f32.ok_or(CpalDriverError::NoOutputDevice)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interleave_writes_stereo_to_first_two_channels() {
        let stride = 4;
        let frames = 3;
        // Left: 0.1, 0.2, 0.3, (stale); Right: 0.4, 0.5, 0.6, (stale).
        let planar = [0.1, 0.2, 0.3, 9.9, 0.4, 0.5, 0.6, 9.9];
        let mut out = vec![0.0_f32; frames * 2];
        interleave_stereo_into_device(&planar, stride, frames, 2, &mut out);
        assert_eq!(out, vec![0.1, 0.4, 0.2, 0.5, 0.3, 0.6]);
    }

    #[test]
    fn interleave_downmixes_to_mono_device() {
        let stride = 2;
        let frames = 2;
        let planar = [1.0, 2.0, 3.0, 5.0]; // L=[1,2], R=[3,5]
        let mut out = vec![0.0_f32; frames];
        interleave_stereo_into_device(&planar, stride, frames, 1, &mut out);
        // mono = 0.5 * (L + R) per frame
        assert_eq!(out, vec![2.0, 3.5]);
    }

    #[test]
    fn interleave_zeroes_surplus_channels_on_multichannel_devices() {
        let stride = 2;
        let frames = 2;
        let planar = [0.7, 0.8, 0.1, 0.2];
        let mut out = vec![42.0_f32; frames * 4];
        interleave_stereo_into_device(&planar, stride, frames, 4, &mut out);
        // Frame 0: L, R, 0, 0; Frame 1: L, R, 0, 0.
        assert_eq!(out, vec![0.7, 0.1, 0.0, 0.0, 0.8, 0.2, 0.0, 0.0]);
    }

    #[test]
    fn interleave_handles_zero_channel_device() {
        // Pathological case; nothing should be written.
        let planar = [0.5, 0.5];
        let mut out: Vec<f32> = Vec::new();
        interleave_stereo_into_device(&planar, 1, 1, 0, &mut out);
        assert!(out.is_empty());
    }
}
