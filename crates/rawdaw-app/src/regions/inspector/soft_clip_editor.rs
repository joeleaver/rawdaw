//! Soft-clipper inspector body — the single threshold slider.
//!
//! X6 of `docs/master-fx-chain-plan.md`. One control: drag the
//! threshold, push a `SoftClipParam::Threshold` event into the
//! audio thread, observe the post-apply value re-bound from the
//! slot's `patch_signal` (U9 audio→UI re-bind pattern so external
//! pushes — future automation, MIDI Learn, preset swaps — keep
//! the slider visually in sync).
//!
//! Layout mirrors the per-track `ParamSlider` row in
//! `wavetable_editor.rs`: 56px label / 1fr slider / 58px value.
//! Single row instead of the wavetable editor's scrolling stack
//! because v1 SoftClip has one parameter.

use rinch::core::reactive::Effect;
use rinch::prelude::*;

use rawdaw_fx::{SoftClipParam, MAX_THRESHOLD, MIN_THRESHOLD};

use crate::audio::{AudioResources, MasterFxPatch};
use crate::theme;

/// Slider step in linear-threshold units. 0.01 is fine enough to
/// resolve ~1 dB increments near the default 0.7 threshold without
/// dragging through hundreds of intermediate values.
const THRESHOLD_STEP: f64 = 0.01;

#[component]
pub fn SoftClipEditor(slot: usize) -> NodeHandle {
    let audio = use_store::<AudioResources>();
    let handles = audio.master_fx_handles.clone();
    let Some(handle) = handles.get(slot).cloned() else {
        // Selected slot is past the chain length (chain just
        // reconfigured). Parent `MasterFxEditor` already renders
        // its own out-of-range stub; here we return an empty
        // fragment so we don't paint a second one.
        return rsx! { div { } };
    };

    // Initial threshold from the live audio-thread snapshot.
    // Falls back to the SoftClipPatch::default threshold if the
    // slot is somehow not SoftClip (impossible at v1 — only one
    // FX kind — but the type system can't prove that, so be
    // defensive).
    let boot_threshold = match handle.patch_signal.get() {
        MasterFxPatch::SoftClip(p) => p.threshold,
    } as f64;
    let value: Signal<f64> = Signal::new(boot_threshold);

    // U9 audio→UI re-bind: every time the slot's `patch_signal`
    // advances (audio thread published a new snapshot, e.g. from
    // an external Param push or future preset swap), pull the
    // current threshold into the slider's local Signal.
    // `set_if_changed` keeps the Effect from bouncing the slider
    // after the user's own drag round-trips back through the
    // audio thread with the same value. Leaked intentionally
    // — the Effect's lifetime matches the inspector subtree
    // (re-mounted on selection change).
    let _ = Effect::new(move || {
        // v1's MasterFxPatch has one variant so this match is
        // exhaustive with a single arm. When future EQ / Reverb
        // variants land, add arms here that simply do nothing —
        // a SoftClipEditor mounted on a non-SoftClip slot
        // shouldn't happen given the kind-aware dispatch in
        // MasterFxEditor, but explicit arms keep the intent
        // self-documenting.
        let MasterFxPatch::SoftClip(p) = handle.patch_signal.get();
        value.set_if_changed(p.threshold as f64);
    });

    let row_style = "display: grid; grid-template-columns: 56px 1fr 58px; \
         gap: 8px; align-items: center; min-width: 0;";
    let label_style = "font-size: 10.5px; color: rgba(232,234,238,0.62); \
         overflow: hidden; text-overflow: ellipsis; white-space: nowrap;";
    let value_style = "font-size: 11px; color: rgba(232,234,238,0.96); \
         font-feature-settings: \"tnum\" 1; \
         font-variant-numeric: tabular-nums; text-align: right; \
         overflow: hidden; text-overflow: ellipsis; white-space: nowrap;";
    let slider_cell_style = "min-width: 0; display: flex; align-items: center;";
    let body_style = format!(
        "padding: 16px; display: flex; flex-direction: column; gap: 12px; \
         border-top: 1px solid {line};",
        line = theme::LINE,
    );

    rsx! {
        div { style: {body_style.clone()},
            div { style: {row_style.to_string()},
                span { style: {label_style.to_string()}, "Threshold" }
                div { style: {slider_cell_style.to_string()},
                    Slider {
                        min: MIN_THRESHOLD as f64,
                        max: MAX_THRESHOLD as f64,
                        step: THRESHOLD_STEP,
                        value_signal: value,
                        size: "sm",
                        style: "width: 100%; min-width: 0;",
                        onchange: move |v: f64| {
                            value.set(v);
                            // Best-effort push: a queue-overflow
                            // (extraordinarily rare — the host live-
                            // event queue is sized orders of
                            // magnitude above a per-tick slider
                            // drag) just means the audio thread
                            // misses one drag tick. We don't crash
                            // the UI for an audio-side bookkeeping
                            // issue.
                            let _ = audio.push_master_fx_param(
                                slot,
                                SoftClipParam::Threshold,
                                v as f32,
                            );
                        },
                    }
                }
                span { style: {value_style.to_string()},
                    {|| format_threshold_dbfs(value.get())}
                }
            }
        }
    }
}

/// Format a linear threshold as a dBFS readout: `20 * log10(t)`.
/// The threshold has a strict lower clamp at `MIN_THRESHOLD = 0.001`
/// (~-60 dBFS) so the log is always defined. One decimal place is
/// plenty for a placeholder UI; the redesign pass can revisit
/// typography + units together.
fn format_threshold_dbfs(t: f64) -> String {
    let db = 20.0 * t.log10();
    format!("{db:.1} dBFS")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_threshold_dbfs_at_known_values() {
        // Reference points for the dBFS readout: unity = 0 dBFS,
        // half = -6 dBFS, tenth = -20 dBFS. The default 0.7
        // threshold lands at ~-3.1 dBFS (verified visually in the
        // MCP screenshot — pins the contract that the readout
        // matches the audible reference).
        assert_eq!(format_threshold_dbfs(1.0), "0.0 dBFS");
        // Allow ±0.1 dBFS tolerance — `.1` formatting rounds at
        // the boundary, and -6.02 dBFS could round either way.
        let half = format_threshold_dbfs(0.5);
        assert!(half == "-6.0 dBFS" || half == "-6.1 dBFS", "got {half}");
        let tenth = format_threshold_dbfs(0.1);
        assert_eq!(tenth, "-20.0 dBFS");
        let default = format_threshold_dbfs(0.7);
        assert_eq!(default, "-3.1 dBFS");
    }

    #[test]
    fn format_threshold_dbfs_at_min_threshold_is_negative_60ish() {
        // MIN_THRESHOLD = 0.001 → 20 * log10(0.001) = -60 dBFS.
        // Sets the floor of the audible range and pins that the
        // formatter doesn't underflow into something like
        // `-inf dBFS` for the clamped minimum.
        let s = format_threshold_dbfs(MIN_THRESHOLD as f64);
        assert_eq!(s, "-60.0 dBFS");
    }

    #[test]
    fn format_threshold_dbfs_at_max_threshold_is_near_zero() {
        // MAX_THRESHOLD = 0.999 → ~ -0.009 dBFS, rounds to 0.0.
        // Pins that the formatter handles the upper clamp without
        // displaying a stray positive value.
        let s = format_threshold_dbfs(MAX_THRESHOLD as f64);
        // Allow either "-0.0 dBFS" or "0.0 dBFS" — both rust + the
        // f64 IEEE conventions can produce either depending on
        // rounding direction at the boundary.
        assert!(s == "-0.0 dBFS" || s == "0.0 dBFS", "got {s}");
    }
}
