//! Drum patch editor — U7 of the synth-UI integration plan.
//!
//! Four sections stacked vertically:
//!
//! 1. **Kick** — start/end pitch, pitch decay, amp ADSR.
//! 2. **Snare** — body start/end pitch, body pitch decay, noise mix,
//!    noise HP cutoff + Q, amp ADSR.
//! 3. **Closed Hat** — HP cutoff + Q, amp ADSR.
//! 4. **Open Hat** — HP cutoff + Q, amp ADSR.
//!
//! Each control owns a `Signal<f64>` for instant visual feedback and
//! pushes a [`DrumParam`] event through
//! [`AudioResources::push_drum_param`] on every change. Mirror of U5's
//! `WavetableEditor` wiring; same layout idioms (60px label + 1fr
//! slider + 60px value display, `min-width: 0` on the slider cell).
//!
//! The U6 audio→UI poll path exists for drum tracks too (per-track
//! `DrumPoller` updates `DrumEditorHandle::patch_signal`), but the
//! per-slider signals aren't bound to it yet — deferred until
//! external sources (MIDI Learn / automation) land.

use rinch::prelude::*;
use rinch::core::reactive::Effect;

use rawdaw_synth_drum::{DrumParam, DrumPatch};

use super::preset_dropdown::{PresetDropdown, PresetKind};
use crate::audio::AudioResources;
use crate::theme;

#[component]
pub fn DrumEditor(track_idx: usize) -> NodeHandle {
    let audio = use_store::<AudioResources>();
    let Some(handle) = audio.drum_handles.get(&track_idx).cloned() else {
        return rsx! {
            div { style: {missing_style()},
                "No drum handle for this track."
            }
        };
    };
    let boot = handle.patch_signal.get();

    let kind: PresetKind = PresetKind::Drum;
    rsx! {
        div { style: {body_style()},
            PresetDropdown { track_idx: track_idx, kind: kind }
            SectionFrame { title: "Kick" }
            KickSection { track_idx: track_idx, boot: boot }
            SectionFrame { title: "Snare" }
            SnareSection { track_idx: track_idx, boot: boot }
            SectionFrame { title: "Closed Hat" }
            ClosedHatSection { track_idx: track_idx, boot: boot }
            SectionFrame { title: "Open Hat" }
            OpenHatSection { track_idx: track_idx, boot: boot }
        }
    }
}

fn body_style() -> String {
    format!(
        "flex: 1; overflow-y: auto; padding: 12px 16px; \
         display: flex; flex-direction: column; gap: 6px; \
         min-height: 0; background: {bg};",
        bg = theme::BG1,
    )
}

fn missing_style() -> String {
    "flex: 1; padding: 24px; \
     display: flex; align-items: center; justify-content: center; \
     font-size: 12.5px; color: rgba(232,234,238,0.42); \
     text-align: center;"
        .to_string()
}

#[component]
fn SectionFrame(title: String) -> NodeHandle {
    let row_style = format!(
        "display: flex; align-items: center; padding-top: 6px; \
         border-top: 1px solid {line}; margin-top: 6px;",
        line = theme::LINE_SOFT,
    );
    let title_style = "font-size: 10.5px; letter-spacing: 0.6px; \
         text-transform: uppercase; color: rgba(232,234,238,0.62); \
         font-weight: 600; padding: 4px 0 6px;";
    let title_owned = title.clone();
    rsx! {
        div { style: {row_style.clone()},
            span { style: {title_style.to_string()}, {title_owned.clone()} }
        }
    }
}

// ── Kick ──────────────────────────────────────────────────────────

#[component]
fn KickSection(track_idx: usize, boot: DrumPatch) -> NodeHandle {
    let k = boot.kick;
    rsx! {
        div { style: "display: flex; flex-direction: column; gap: 3px;",
            ParamSliderDrum {
                label: "Start", unit: "Hz",
                min: 20.0_f64, max: 1000.0_f64, step: 1.0_f64,
                initial: k.start_hz as f64,
                track_idx: track_idx, param: DrumParam::KickStartHz,
            }
            ParamSliderDrum {
                label: "End", unit: "Hz",
                min: 20.0_f64, max: 500.0_f64, step: 1.0_f64,
                initial: k.end_hz as f64,
                track_idx: track_idx, param: DrumParam::KickEndHz,
            }
            ParamSliderDrum {
                label: "Pitch dec", unit: "s",
                min: 0.001_f64, max: 1.0_f64, step: 0.001_f64,
                initial: k.pitch_decay_s as f64,
                track_idx: track_idx, param: DrumParam::KickPitchDecayS,
            }
            ParamSliderDrum {
                label: "Attack", unit: "s",
                min: 0.0_f64, max: 1.0_f64, step: 0.001_f64,
                initial: k.amp.attack_s as f64,
                track_idx: track_idx, param: DrumParam::KickAmpAttackS,
            }
            ParamSliderDrum {
                label: "Decay", unit: "s",
                min: 0.0_f64, max: 2.0_f64, step: 0.001_f64,
                initial: k.amp.decay_s as f64,
                track_idx: track_idx, param: DrumParam::KickAmpDecayS,
            }
            ParamSliderDrum {
                label: "Sustain", unit: "",
                min: 0.0_f64, max: 1.0_f64, step: 0.01_f64,
                initial: k.amp.sustain_level as f64,
                track_idx: track_idx, param: DrumParam::KickAmpSustain,
            }
            ParamSliderDrum {
                label: "Release", unit: "s",
                min: 0.0_f64, max: 2.0_f64, step: 0.001_f64,
                initial: k.amp.release_s as f64,
                track_idx: track_idx, param: DrumParam::KickAmpRelease,
            }
        }
    }
}

// ── Snare ─────────────────────────────────────────────────────────

#[component]
fn SnareSection(track_idx: usize, boot: DrumPatch) -> NodeHandle {
    let s = boot.snare;
    rsx! {
        div { style: "display: flex; flex-direction: column; gap: 3px;",
            ParamSliderDrum {
                label: "Body st", unit: "Hz",
                min: 60.0_f64, max: 1000.0_f64, step: 1.0_f64,
                initial: s.body_start_hz as f64,
                track_idx: track_idx, param: DrumParam::SnareBodyStartHz,
            }
            ParamSliderDrum {
                label: "Body end", unit: "Hz",
                min: 60.0_f64, max: 800.0_f64, step: 1.0_f64,
                initial: s.body_end_hz as f64,
                track_idx: track_idx, param: DrumParam::SnareBodyEndHz,
            }
            ParamSliderDrum {
                label: "Pitch dec", unit: "s",
                min: 0.001_f64, max: 0.5_f64, step: 0.001_f64,
                initial: s.body_pitch_decay_s as f64,
                track_idx: track_idx, param: DrumParam::SnareBodyPitchDecayS,
            }
            ParamSliderDrum {
                label: "Noise mix", unit: "",
                min: 0.0_f64, max: 1.0_f64, step: 0.01_f64,
                initial: s.noise_mix as f64,
                track_idx: track_idx, param: DrumParam::SnareNoiseMix,
            }
            ParamSliderDrum {
                label: "Noise HP", unit: "Hz",
                min: 100.0_f64, max: 10_000.0_f64, step: 10.0_f64,
                initial: s.noise_hp_hz as f64,
                track_idx: track_idx, param: DrumParam::SnareNoiseHpHz,
            }
            ParamSliderDrum {
                label: "Noise Q", unit: "",
                min: 0.0_f64, max: 4.0_f64, step: 0.01_f64,
                initial: s.noise_hp_q as f64,
                track_idx: track_idx, param: DrumParam::SnareNoiseHpQ,
            }
            ParamSliderDrum {
                label: "Attack", unit: "s",
                min: 0.0_f64, max: 1.0_f64, step: 0.001_f64,
                initial: s.amp.attack_s as f64,
                track_idx: track_idx, param: DrumParam::SnareAmpAttackS,
            }
            ParamSliderDrum {
                label: "Decay", unit: "s",
                min: 0.0_f64, max: 2.0_f64, step: 0.001_f64,
                initial: s.amp.decay_s as f64,
                track_idx: track_idx, param: DrumParam::SnareAmpDecayS,
            }
            ParamSliderDrum {
                label: "Sustain", unit: "",
                min: 0.0_f64, max: 1.0_f64, step: 0.01_f64,
                initial: s.amp.sustain_level as f64,
                track_idx: track_idx, param: DrumParam::SnareAmpSustain,
            }
            ParamSliderDrum {
                label: "Release", unit: "s",
                min: 0.0_f64, max: 2.0_f64, step: 0.001_f64,
                initial: s.amp.release_s as f64,
                track_idx: track_idx, param: DrumParam::SnareAmpRelease,
            }
        }
    }
}

// ── Closed Hat ────────────────────────────────────────────────────

#[component]
fn ClosedHatSection(track_idx: usize, boot: DrumPatch) -> NodeHandle {
    let h = boot.closed_hat;
    rsx! {
        div { style: "display: flex; flex-direction: column; gap: 3px;",
            ParamSliderDrum {
                label: "HP", unit: "Hz",
                min: 1000.0_f64, max: 12_000.0_f64, step: 10.0_f64,
                initial: h.hp_hz as f64,
                track_idx: track_idx, param: DrumParam::ClosedHatHpHz,
            }
            ParamSliderDrum {
                label: "HP Q", unit: "",
                min: 0.0_f64, max: 4.0_f64, step: 0.01_f64,
                initial: h.hp_q as f64,
                track_idx: track_idx, param: DrumParam::ClosedHatHpQ,
            }
            ParamSliderDrum {
                label: "Attack", unit: "s",
                min: 0.0_f64, max: 0.5_f64, step: 0.001_f64,
                initial: h.amp.attack_s as f64,
                track_idx: track_idx, param: DrumParam::ClosedHatAmpAttackS,
            }
            ParamSliderDrum {
                label: "Decay", unit: "s",
                min: 0.0_f64, max: 1.0_f64, step: 0.001_f64,
                initial: h.amp.decay_s as f64,
                track_idx: track_idx, param: DrumParam::ClosedHatAmpDecayS,
            }
            ParamSliderDrum {
                label: "Sustain", unit: "",
                min: 0.0_f64, max: 1.0_f64, step: 0.01_f64,
                initial: h.amp.sustain_level as f64,
                track_idx: track_idx, param: DrumParam::ClosedHatAmpSustain,
            }
            ParamSliderDrum {
                label: "Release", unit: "s",
                min: 0.0_f64, max: 1.0_f64, step: 0.001_f64,
                initial: h.amp.release_s as f64,
                track_idx: track_idx, param: DrumParam::ClosedHatAmpRelease,
            }
        }
    }
}

// ── Open Hat ──────────────────────────────────────────────────────

#[component]
fn OpenHatSection(track_idx: usize, boot: DrumPatch) -> NodeHandle {
    let h = boot.open_hat;
    rsx! {
        div { style: "display: flex; flex-direction: column; gap: 3px;",
            ParamSliderDrum {
                label: "HP", unit: "Hz",
                min: 1000.0_f64, max: 12_000.0_f64, step: 10.0_f64,
                initial: h.hp_hz as f64,
                track_idx: track_idx, param: DrumParam::OpenHatHpHz,
            }
            ParamSliderDrum {
                label: "HP Q", unit: "",
                min: 0.0_f64, max: 4.0_f64, step: 0.01_f64,
                initial: h.hp_q as f64,
                track_idx: track_idx, param: DrumParam::OpenHatHpQ,
            }
            ParamSliderDrum {
                label: "Attack", unit: "s",
                min: 0.0_f64, max: 0.5_f64, step: 0.001_f64,
                initial: h.amp.attack_s as f64,
                track_idx: track_idx, param: DrumParam::OpenHatAmpAttackS,
            }
            ParamSliderDrum {
                label: "Decay", unit: "s",
                min: 0.0_f64, max: 2.0_f64, step: 0.001_f64,
                initial: h.amp.decay_s as f64,
                track_idx: track_idx, param: DrumParam::OpenHatAmpDecayS,
            }
            ParamSliderDrum {
                label: "Sustain", unit: "",
                min: 0.0_f64, max: 1.0_f64, step: 0.01_f64,
                initial: h.amp.sustain_level as f64,
                track_idx: track_idx, param: DrumParam::OpenHatAmpSustain,
            }
            ParamSliderDrum {
                label: "Release", unit: "s",
                min: 0.0_f64, max: 2.0_f64, step: 0.001_f64,
                initial: h.amp.release_s as f64,
                track_idx: track_idx, param: DrumParam::OpenHatAmpRelease,
            }
        }
    }
}

/// One labeled drum-param slider row. Same layout as the wavetable
/// editor's `ParamSlider`; specialized to `DrumParam` so the rinch
/// macro can carry the variant as a `#[component]` prop.
#[component]
fn ParamSliderDrum(
    label: String,
    unit: String,
    min: Option<f64>,
    max: Option<f64>,
    step: Option<f64>,
    initial: f64,
    track_idx: usize,
    param: DrumParam,
) -> NodeHandle {
    let audio = use_store::<AudioResources>();
    let value = Signal::new(initial);

    // U9 audio→UI bind — mirror of the wavetable ParamSlider
    // pattern. See `wavetable_editor::ParamSlider` for the
    // rationale.
    if let Some(handle) = audio.drum_handles.get(&track_idx).cloned() {
        let _ = Effect::new(move || {
            let patch = handle.patch_signal.get();
            value.set_if_changed(param.read_from(&patch) as f64);
        });
    }

    let row_style = "display: grid; grid-template-columns: 64px 1fr 58px; \
         gap: 8px; align-items: center; min-width: 0;";
    let label_style = "font-size: 10.5px; color: rgba(232,234,238,0.62); \
         overflow: hidden; text-overflow: ellipsis; white-space: nowrap;";
    let value_style = "font-size: 11px; color: rgba(232,234,238,0.96); \
         font-feature-settings: \"tnum\" 1; \
         font-variant-numeric: tabular-nums; text-align: right; \
         overflow: hidden; text-overflow: ellipsis; white-space: nowrap;";
    let slider_cell_style = "min-width: 0; display: flex; align-items: center;";

    let label_owned = label.clone();
    let unit_for_display = unit.clone();

    rsx! {
        div { style: {row_style.to_string()},
            span { style: {label_style.to_string()}, {label_owned.clone()} }
            div { style: {slider_cell_style.to_string()},
                Slider {
                    min: min,
                    max: max,
                    step: step,
                    value_signal: value,
                    size: "sm",
                    style: "width: 100%; min-width: 0;",
                    onchange: move |v: f64| {
                        value.set(v);
                        let _ = audio.push_drum_param(track_idx, param, v as f32);
                    },
                }
            }
            span { style: {value_style.to_string()},
                {|| format_value(value.get(), &unit_for_display)}
            }
        }
    }
}

fn format_value(v: f64, unit: &str) -> String {
    let body = if unit.is_empty() {
        format!("{:.2}", v)
    } else if unit == "Hz" {
        format!("{:.0}", v)
    } else {
        // seconds — three decimals for the short envelopes drums use.
        format!("{:.3}", v)
    };
    if unit.is_empty() {
        body
    } else {
        format!("{body} {unit}")
    }
}
