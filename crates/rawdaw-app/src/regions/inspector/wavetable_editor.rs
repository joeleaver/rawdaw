//! Wavetable patch editor — U5 scalar controls (no mod matrix yet).
//!
//! Four sections stacked vertically:
//!
//! 1. **Oscillators** — three rows, each with sliders for tune
//!    (-24..=24 st), fine (-100..=100 cents), level (0..=1).
//! 2. **Envelopes** — three columns (ENV1 / ENV2 / ENV3), each
//!    with Attack / Decay / Sustain / Release sliders.
//! 3. **Filter** — cutoff (20..=20 000 Hz, linear range; log-scale
//!    presentation is a U5+ polish item) and resonance (0..=4).
//! 4. **LFO** — rate (0..=20 Hz).
//!
//! ## Wiring
//!
//! Each slider owns a `Signal<f64>` seeded from the audio thread's
//! current patch snapshot (read via the editor's
//! [`WavetableEditorHandle::patch_signal`]). Slider drag updates
//! the signal locally for instant visual feedback and pushes a
//! [`WavetableParam`] event through [`AudioResources::push_wavetable_param`]
//! at `current_sample_clock + 1` so the audio thread applies the
//! change in the next block.
//!
//! Patch-mirror polling (see [`audio::wavetable_poller`]) keeps the
//! handle's signal in sync with the audio thread; future MIDI Learn
//! / automation paths will land on `patch_signal` and a follow-on
//! Effect can sync per-slider signals from there. U5 ships the
//! one-directional UI→audio path only; the audio→UI path's
//! infrastructure is installed but isn't observed by sliders yet.

use rinch::prelude::*;
use rinch::core::reactive::Effect;

use rawdaw_synth_wavetable::{WavetablePatch, WavetableParam};

use super::matrix_editor::MatrixEditor;
use super::preset_dropdown::{PresetDropdown, PresetKind};
use crate::audio::AudioResources;
use crate::theme;

/// Real wavetable patch editor body — replaces the U4 placeholder
/// once `track_idx` resolves to a known [`WavetableEditorHandle`].
/// Falls back to a small "no synth" panel when the index is wrong
/// (defensive — the parent gates on a valid handle).
#[component]
pub fn WavetableEditor(track_idx: usize) -> NodeHandle {
    let audio = use_store::<AudioResources>();
    let Some(handle) = audio.wavetable_handles.get(&track_idx).cloned() else {
        return rsx! {
            div { style: {missing_style()},
                "No wavetable handle for this track."
            }
        };
    };
    // Single read of the boot patch — every slider seeds its own
    // `Signal<f64>` from this snapshot. The handle's signal stays
    // live so external updates (poller-driven) remain observable;
    // U5 just doesn't bind per-slider signals to it (deferred to a
    // follow-on Effect when MIDI Learn lands).
    let boot = handle.patch_signal.get();

    let kind: PresetKind = PresetKind::Wavetable;
    rsx! {
        div { style: {body_style()},
            PresetDropdown { track_idx: track_idx, kind: kind }
            SectionFrame { title: "Oscillators" }
            OscillatorRows { track_idx: track_idx, boot: boot }
            SectionFrame { title: "Envelopes" }
            EnvelopeColumns { track_idx: track_idx, boot: boot }
            SectionFrame { title: "Filter" }
            FilterRows { track_idx: track_idx, boot: boot }
            SectionFrame { title: "LFO" }
            LfoRow { track_idx: track_idx, boot: boot }
            SectionFrame { title: "Mod Matrix" }
            MatrixEditor { track_idx: track_idx, boot: boot }
        }
    }
}

/// One section header — title bar above the section's controls. The
/// rinch `#[component]` macro's `children: &[NodeHandle]` magic
/// requires a `Component for Foo` impl with imperative
/// `for child in children { node.append_child(child); }` to use it;
/// rather than reach for that anti-pattern, the editor places this
/// header as a sibling of each section's body in the parent rsx and
/// lets the body components own their own outer divs.
#[component]
fn SectionFrame(title: String) -> NodeHandle {
    let row_style = format!(
        "display: flex; align-items: center; padding-top: 4px; \
         border-top: 1px solid {line}; margin-top: 6px;",
        line = theme::LINE_SOFT,
    );
    let title_style = "font-size: 10.5px; letter-spacing: 0.6px; \
         text-transform: uppercase; color: rgba(232,234,238,0.42); \
         font-weight: 600; padding: 4px 0 6px;";
    let title_owned = title.clone();
    rsx! {
        div { style: {row_style.clone()},
            span { style: {title_style.to_string()}, {title_owned.clone()} }
        }
    }
}

fn body_style() -> String {
    format!(
        "flex: 1; overflow-y: auto; padding: 12px 16px; \
         display: flex; flex-direction: column; gap: 18px; \
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
fn OscillatorRows(track_idx: usize, boot: WavetablePatch) -> NodeHandle {
    // Bound literals: the rinch macro auto-wraps `0u8` / `1u8` /
    // `2u8` written inline into `Some(_)`, which then mismatches
    // OscRow's `osc_idx: u8`. Routing through a `let` keeps the
    // type as plain `u8` (the wrap only fires for literals).
    let osc0: u8 = 0;
    let osc1: u8 = 1;
    let osc2: u8 = 2;
    rsx! {
        div { style: "display: flex; flex-direction: column; gap: 4px;",
            OscRow { track_idx: track_idx, osc_idx: osc0, boot: boot }
            OscRow { track_idx: track_idx, osc_idx: osc1, boot: boot }
            OscRow { track_idx: track_idx, osc_idx: osc2, boot: boot }
        }
    }
}

#[component]
fn OscRow(track_idx: usize, osc_idx: u8, boot: WavetablePatch) -> NodeHandle {
    let p = boot.osc_params[osc_idx as usize];
    let group_label = format!("OSC {}", osc_idx + 1);
    let header_style = format!(
        "font-size: 11px; color: rgba(232,234,238,0.62); \
         letter-spacing: 0.3px; font-weight: 500; \
         padding: 4px 0 2px; \
         border-top: 1px solid {line_soft};",
        line_soft = theme::LINE_SOFT,
    );
    rsx! {
        div { style: "display: flex; flex-direction: column; gap: 3px;",
            div { style: {header_style.clone()}, {group_label.clone()} }
            ParamSlider {
                label: "Tune", unit: "st",
                min: -24.0_f64, max: 24.0_f64, step: 1.0_f64,
                initial: p.tune_semitones as f64,
                track_idx: track_idx,
                param: WavetableParam::OscTune(osc_idx),
            }
            ParamSlider {
                label: "Fine", unit: "ct",
                min: -100.0_f64, max: 100.0_f64, step: 1.0_f64,
                initial: p.fine_cents as f64,
                track_idx: track_idx,
                param: WavetableParam::OscFineCents(osc_idx),
            }
            ParamSlider {
                label: "Level", unit: "",
                min: 0.0_f64, max: 1.0_f64, step: 0.01_f64,
                initial: p.level as f64,
                track_idx: track_idx,
                param: WavetableParam::OscLevel(osc_idx),
            }
        }
    }
}

#[component]
fn EnvelopeColumns(track_idx: usize, boot: WavetablePatch) -> NodeHandle {
    let row_style = "display: grid; grid-template-columns: repeat(3, 1fr); gap: 12px;";
    // Same `let` trick as in OscillatorRows — sidesteps the rinch
    // macro's literal-numeric Option-wrap.
    let env0: u8 = 0;
    let env1: u8 = 1;
    let env2: u8 = 2;
    rsx! {
        div { style: {row_style.to_string()},
            EnvelopeColumn { track_idx: track_idx, env_idx: env0, boot: boot }
            EnvelopeColumn { track_idx: track_idx, env_idx: env1, boot: boot }
            EnvelopeColumn { track_idx: track_idx, env_idx: env2, boot: boot }
        }
    }
}

#[component]
fn EnvelopeColumn(track_idx: usize, env_idx: u8, boot: WavetablePatch) -> NodeHandle {
    let e = boot.env_params[env_idx as usize];
    let label = format!("ENV {}", env_idx + 1);
    let label_style = "font-size: 11px; color: rgba(232,234,238,0.62); \
         letter-spacing: 0.3px; font-weight: 500; \
         padding-bottom: 4px;";
    rsx! {
        div { style: "display: flex; flex-direction: column; gap: 4px;",
            div { style: {label_style.to_string()}, {label.clone()} }
            ParamSlider {
                label: "Attack", unit: "s",
                min: 0.0_f64, max: 4.0_f64, step: 0.01_f64,
                initial: e.attack_s as f64,
                track_idx: track_idx,
                param: WavetableParam::EnvAttackS(env_idx),
            }
            ParamSlider {
                label: "Decay", unit: "s",
                min: 0.0_f64, max: 4.0_f64, step: 0.01_f64,
                initial: e.decay_s as f64,
                track_idx: track_idx,
                param: WavetableParam::EnvDecayS(env_idx),
            }
            ParamSlider {
                label: "Sustain", unit: "",
                min: 0.0_f64, max: 1.0_f64, step: 0.01_f64,
                initial: e.sustain_level as f64,
                track_idx: track_idx,
                param: WavetableParam::EnvSustain(env_idx),
            }
            ParamSlider {
                label: "Release", unit: "s",
                min: 0.0_f64, max: 4.0_f64, step: 0.01_f64,
                initial: e.release_s as f64,
                track_idx: track_idx,
                param: WavetableParam::EnvReleaseS(env_idx),
            }
        }
    }
}

#[component]
fn FilterRows(track_idx: usize, boot: WavetablePatch) -> NodeHandle {
    rsx! {
        div { style: "display: flex; flex-direction: column; gap: 4px;",
            ParamSlider {
                label: "Cutoff", unit: "Hz",
                // Linear span over the audible range — true log scaling
                // is a U5+ polish item (Rinch Slider doesn't support
                // log natively today; we'd map slider 0..1 → exp).
                min: 20.0_f64, max: 20_000.0_f64, step: 1.0_f64,
                initial: boot.filter_cutoff_hz as f64,
                track_idx: track_idx,
                param: WavetableParam::FilterCutoffHz,
            }
            ParamSlider {
                label: "Resonance", unit: "",
                min: 0.0_f64, max: 4.0_f64, step: 0.01_f64,
                initial: boot.filter_resonance as f64,
                track_idx: track_idx,
                param: WavetableParam::FilterResonance,
            }
        }
    }
}

#[component]
fn LfoRow(track_idx: usize, boot: WavetablePatch) -> NodeHandle {
    rsx! {
        div { style: "display: flex; flex-direction: column; gap: 4px;",
            ParamSlider {
                label: "Rate", unit: "Hz",
                min: 0.0_f64, max: 20.0_f64, step: 0.01_f64,
                initial: boot.lfo_rate_hz as f64,
                track_idx: track_idx,
                param: WavetableParam::LfoRateHz,
            }
        }
    }
}

/// One labeled slider row. Owns its `Signal<f64>` for instant
/// visual feedback during drag; pushes a [`WavetableParam`] event
/// through [`AudioResources`] on every change so the audio thread
/// applies the value in the next block.
///
/// `min` / `max` / `step` are `Option<f64>` so the rinch macro's
/// numeric-literal auto-wrap (any `min: 24.0` at a call site rolls
/// up to `Some(24.0)`) types this prop through cleanly.
///
/// Layout: `[label]  [────────slider────────]  [value unit]`.
#[component]
fn ParamSlider(
    label: String,
    unit: String,
    min: Option<f64>,
    max: Option<f64>,
    step: Option<f64>,
    initial: f64,
    track_idx: usize,
    param: WavetableParam,
) -> NodeHandle {
    let audio = use_store::<AudioResources>();
    let value = Signal::new(initial);

    // U9 audio→UI bind: when `patch_signal` advances (preset
    // swap, future MIDI Learn / automation), pull the field's
    // new value into this slider's Signal. `set_if_changed`
    // keeps the Effect from bouncing the slider after the user's
    // own drag round-trips back through the audio thread with
    // the same value. The Effect leaks intentionally: each
    // editor's lifetime is bounded by the inspector's re-mount
    // on selection change, so the Effect drops with the rsx
    // subtree alongside the captured `value` Signal.
    if let Some(handle) = audio.wavetable_handles.get(&track_idx).cloned() {
        let _ = Effect::new(move || {
            let patch = handle.patch_signal.get();
            value.set_if_changed(param.read_from(&patch) as f64);
        });
    }

    // `min-width: 0` on the grid + middle-cell wrapper is the CSS
    // idiom that lets a 1fr column actually shrink to its allocated
    // width. Without it the Rinch Slider's intrinsic min-content
    // pushes the value column off-screen (visible during U5 dev:
    // OSC sliders extended past the inspector edge, hiding `0 st`).
    let row_style = "display: grid; grid-template-columns: 56px 1fr 58px; \
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
                        // Best-effort push: a queue-overflow error
                        // (extraordinarily rare — the event queue
                        // is sized orders of magnitude above a
                        // per-tick slider drag) only means the audio
                        // thread misses one drag tick. We don't
                        // crash the UI for an audio-side bookkeeping
                        // issue.
                        let _ = audio.push_wavetable_param(track_idx, param, v as f32);
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
    // Three decimals for fractional units (s, level); whole-number
    // for tune/fine/Hz so the readout doesn't jitter on integer
    // sliders.
    let body = if unit.is_empty() || unit == "Hz" || unit == "st" || unit == "ct" {
        format!("{:.0}", v)
    } else {
        format!("{:.2}", v)
    };
    if unit.is_empty() {
        body
    } else {
        format!("{body} {unit}")
    }
}
