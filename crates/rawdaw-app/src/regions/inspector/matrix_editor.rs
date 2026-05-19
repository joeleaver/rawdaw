//! Mod matrix editor — U6 of the synth-UI integration plan.
//!
//! Vital-style 16-slot list. Each row:
//!
//! ```text
//! [Source ▾]  →  [Destination ▾]  [Amount slider]  [×]
//! ```
//!
//! Empty slots (source = `None`) render dimmed so the user can see at
//! a glance which slots are active. The clear button (`×`) is a
//! shortcut for setting source back to `None`; the source dropdown's
//! `None` entry does the same.
//!
//! ## Wiring
//!
//! - **Source dropdown** → pushes [`WavetableParam::MatrixSource`]
//!   with the ordinal `f32` produced by
//!   [`encode_mod_source`](rawdaw_synth_wavetable::encode_mod_source).
//! - **Destination dropdown** → pushes
//!   [`WavetableParam::MatrixDestination`] with the flat-index `f32`
//!   from [`encode_mod_destination`].
//! - **Amount slider** → pushes [`WavetableParam::MatrixAmount`]
//!   with the raw `f32`.
//!
//! Audio-thread topo-sort handles cycle rejection — a cyclic
//! audio-rate routing (`Osc0 → PmAmountOf(0)`) gets silently
//! sanitized on the synth side in release. The UI doesn't try to
//! pre-validate; the patch poll will eventually re-publish the
//! sanitized state.

use rinch::prelude::*;
use rinch::core::reactive::Effect;

use rawdaw_synth_wavetable::{
    encode_mod_destination, encode_mod_source, ModDestination, ModSource, WavetableParam,
    WavetablePatch,
};

use crate::audio::AudioResources;
use crate::theme;

/// Number of slots in a wavetable mod matrix. Matches
/// `rawdaw_synth_wavetable::MOD_MATRIX_SLOTS` — duplicated here as
/// a `u8` so the constant can sit inside `#[component]` props
/// without exposing the synth crate's private const.
const NUM_SLOTS: u8 = 16;

#[component]
pub fn MatrixEditor(track_idx: usize, boot: WavetablePatch) -> NodeHandle {
    rsx! {
        div { style: "display: flex; flex-direction: column; gap: 4px;",
            // 16 rows, always rendered. Empty slots render dimmed.
            // No virtualization — 16 rows fit comfortably under
            // Rinch's per-frame budget.
            for slot in (0u8..NUM_SLOTS).collect::<Vec<u8>>() {
                MatrixSlotRow {
                    key: slot,
                    track_idx: track_idx,
                    slot_idx: slot,
                    boot: boot,
                }
            }
        }
    }
}

#[component]
fn MatrixSlotRow(track_idx: usize, slot_idx: u8, boot: WavetablePatch) -> NodeHandle {
    let audio = use_store::<AudioResources>();
    // Per-closure clones because `AudioResources` is `Clone` (not
    // `Copy`); each onchange/onclick captures by move. Four
    // controls in this row, so four clones.
    let audio_source = audio.clone();
    let audio_dest = audio.clone();
    let audio_amount = audio.clone();
    let audio_clear = audio.clone();
    let initial = boot.matrix[slot_idx as usize];

    // U9 audio→UI bind for the matrix row's three controls. One
    // Effect per signal reads the matching slot field from the
    // editor's per-track `patch_signal` and `set_if_changed`s
    // the slot's String / f64 signal. Same drop contract as the
    // wavetable / drum ParamSlider Effects.
    let bind_handle = audio.wavetable_handles.get(&track_idx).cloned();

    // Per-slot signals. The Select component needs a `value_fn ||
    // String` for reactive sync; we hold owned `Signal<String>`
    // alongside the typed source / destination so updates from a
    // future polled-patch sync (U6+) can flow through one place.
    let source_sig = Signal::new(mod_source_value(initial.source).to_string());
    let dest_sig = Signal::new(mod_destination_value(initial.destination));
    let amount_sig = Signal::new(initial.amount as f64);

    // Hook each per-slot signal into the patch-poll path so a
    // preset swap (or future external push) re-syncs all three
    // controls without an editor remount. Effects leak with the
    // rsx subtree's lifetime, same as the wavetable/drum
    // ParamSlider Effects.
    if let Some(handle) = bind_handle {
        let h0 = handle.clone();
        let _ = Effect::new(move || {
            let patch = h0.patch_signal.get();
            if let Some(slot) = patch.matrix.get(slot_idx as usize) {
                source_sig.set_if_changed(mod_source_value(slot.source).to_string());
            }
        });
        let h1 = handle.clone();
        let _ = Effect::new(move || {
            let patch = h1.patch_signal.get();
            if let Some(slot) = patch.matrix.get(slot_idx as usize) {
                dest_sig.set_if_changed(mod_destination_value(slot.destination));
            }
        });
        let h2 = handle;
        let _ = Effect::new(move || {
            let patch = h2.patch_signal.get();
            if let Some(slot) = patch.matrix.get(slot_idx as usize) {
                amount_sig.set_if_changed(slot.amount as f64);
            }
        });
    }

    let slot_label = format!("{:02}", slot_idx + 1);
    // Fixed widths on every column except the source dropdown,
    // which gets the leftover 1fr. The amount slider gets a fixed
    // 96 px — wide enough to drag meaningfully, narrow enough that
    // the × button stays on-screen at the 600 px inspector width
    // even after the two dropdowns expand to fit their content.
    let row_style = "display: grid; \
         grid-template-columns: 20px 1fr 12px 1fr 96px 22px; \
         gap: 6px; align-items: center; \
         padding: 3px 0; min-width: 0;";
    let label_style = format!(
        "font-size: 10px; color: {text3}; \
         font-feature-settings: \"tnum\" 1; text-align: right;",
        text3 = theme::TEXT3,
    );
    let arrow_style = format!(
        "font-size: 10px; color: {text3}; text-align: center;",
        text3 = theme::TEXT3,
    );
    let clear_btn_style = format!(
        "width: 22px; height: 22px; padding: 0; \
         background: transparent; border: 1px solid {line}; \
         border-radius: 3px; cursor: pointer; \
         color: {text2}; font-size: 12px; line-height: 1; \
         font-family: inherit;",
        line = theme::LINE,
        text2 = theme::TEXT2,
    );

    rsx! {
        div { style: {row_style.to_string()},
            span { style: {label_style.clone()}, {slot_label.clone()} }
            // Source dropdown.
            Select {
                size: "sm",
                value_fn: move || source_sig.get(),
                data: source_options(),
                onchange: move |v: String| {
                    if let Some(src) = decode_source_str(&v) {
                        source_sig.set(v);
                        let _ = audio_source.push_wavetable_param(
                            track_idx,
                            WavetableParam::MatrixSource(slot_idx),
                            encode_mod_source(src),
                        );
                    }
                },
            }
            // Static arrow between the two dropdowns.
            span { style: {arrow_style.clone()}, "→" }
            // Destination dropdown.
            Select {
                size: "sm",
                value_fn: move || dest_sig.get(),
                data: destination_options(),
                onchange: move |v: String| {
                    if let Some(dst) = decode_destination_str(&v) {
                        dest_sig.set(v);
                        let _ = audio_dest.push_wavetable_param(
                            track_idx,
                            WavetableParam::MatrixDestination(slot_idx),
                            encode_mod_destination(dst),
                        );
                    }
                },
            }
            // Amount slider in a min-width:0 wrapper so it honors
            // the grid's 96 px cell rather than expanding to
            // intrinsic min-content. Range -1..=1 matches the
            // typical mod-matrix span; the synth clamps to -2..=2
            // so a wider-input variant added later still validates.
            div { style: "min-width: 0; display: flex; align-items: center;",
                Slider {
                    min: -1.0_f64,
                    max: 1.0_f64,
                    step: 0.01_f64,
                    value_signal: amount_sig,
                    size: "sm",
                    style: "width: 100%; min-width: 0;",
                    onchange: move |v: f64| {
                        amount_sig.set(v);
                        let _ = audio_amount.push_wavetable_param(
                            track_idx,
                            WavetableParam::MatrixAmount(slot_idx),
                            v as f32,
                        );
                    },
                }
            }
            // Clear button — sets source to None. Mirrors the
            // dropdown's None entry, but a one-click affordance
            // matches Vital's convention.
            button {
                r#type: "button",
                style: {clear_btn_style.clone()},
                onclick: move || {
                    source_sig.set(mod_source_value(ModSource::None).to_string());
                    let _ = audio_clear.push_wavetable_param(
                        track_idx,
                        WavetableParam::MatrixSource(slot_idx),
                        encode_mod_source(ModSource::None),
                    );
                },
                "×"
            }
        }
    }
}

// ── ModSource <-> string ──────────────────────────────────────────

/// Variant key used as the Select option `value`. Stable for
/// onchange decoding.
pub(super) fn mod_source_value(src: ModSource) -> &'static str {
    match src {
        ModSource::None => "none",
        ModSource::Env1 => "env1",
        ModSource::Env2 => "env2",
        ModSource::Env3 => "env3",
        ModSource::Lfo1 => "lfo1",
        ModSource::Osc0 => "osc0",
        ModSource::Osc1 => "osc1",
        ModSource::Osc2 => "osc2",
    }
}

fn mod_source_label(src: ModSource) -> &'static str {
    match src {
        ModSource::None => "—",
        ModSource::Env1 => "ENV 1 (amp)",
        ModSource::Env2 => "ENV 2",
        ModSource::Env3 => "ENV 3",
        ModSource::Lfo1 => "LFO 1",
        ModSource::Osc0 => "OSC 1",
        ModSource::Osc1 => "OSC 2",
        ModSource::Osc2 => "OSC 3",
    }
}

pub(super) fn decode_source_str(s: &str) -> Option<ModSource> {
    match s {
        "none" => Some(ModSource::None),
        "env1" => Some(ModSource::Env1),
        "env2" => Some(ModSource::Env2),
        "env3" => Some(ModSource::Env3),
        "lfo1" => Some(ModSource::Lfo1),
        "osc0" => Some(ModSource::Osc0),
        "osc1" => Some(ModSource::Osc1),
        "osc2" => Some(ModSource::Osc2),
        _ => None,
    }
}

fn source_options() -> Vec<SelectOption> {
    [
        ModSource::None,
        ModSource::Env1,
        ModSource::Env2,
        ModSource::Env3,
        ModSource::Lfo1,
        ModSource::Osc0,
        ModSource::Osc1,
        ModSource::Osc2,
    ]
    .into_iter()
    .map(|s| SelectOption::new(mod_source_value(s), mod_source_label(s)))
    .collect()
}

// ── ModDestination <-> string ─────────────────────────────────────

pub(super) fn mod_destination_value(dst: ModDestination) -> String {
    match dst {
        ModDestination::FilterCutoff => "filter_cutoff".to_string(),
        ModDestination::FilterResonance => "filter_resonance".to_string(),
        ModDestination::OscLevel(i) => format!("osc_level_{i}"),
        ModDestination::OscTune(i) => format!("osc_tune_{i}"),
        ModDestination::OscFineTune(i) => format!("osc_fine_{i}"),
        ModDestination::PmAmountOf(i) => format!("pm_of_{i}"),
        ModDestination::AmAmountOf(i) => format!("am_of_{i}"),
        ModDestination::RmAmountOf(i) => format!("rm_of_{i}"),
        ModDestination::LfoRate => "lfo_rate".to_string(),
    }
}

fn mod_destination_label(dst: ModDestination) -> String {
    match dst {
        ModDestination::FilterCutoff => "Filter Cutoff".to_string(),
        ModDestination::FilterResonance => "Filter Resonance".to_string(),
        ModDestination::OscLevel(i) => format!("OSC {} Level", i + 1),
        ModDestination::OscTune(i) => format!("OSC {} Tune", i + 1),
        ModDestination::OscFineTune(i) => format!("OSC {} Fine", i + 1),
        ModDestination::PmAmountOf(i) => format!("PM into OSC {}", i + 1),
        ModDestination::AmAmountOf(i) => format!("AM into OSC {}", i + 1),
        ModDestination::RmAmountOf(i) => format!("RM into OSC {}", i + 1),
        ModDestination::LfoRate => "LFO 1 Rate".to_string(),
    }
}

pub(super) fn decode_destination_str(s: &str) -> Option<ModDestination> {
    match s {
        "filter_cutoff" => Some(ModDestination::FilterCutoff),
        "filter_resonance" => Some(ModDestination::FilterResonance),
        "lfo_rate" => Some(ModDestination::LfoRate),
        s if s.starts_with("osc_level_") => parse_indexed(s, "osc_level_", ModDestination::OscLevel),
        s if s.starts_with("osc_tune_") => parse_indexed(s, "osc_tune_", ModDestination::OscTune),
        s if s.starts_with("osc_fine_") => {
            parse_indexed(s, "osc_fine_", ModDestination::OscFineTune)
        }
        s if s.starts_with("pm_of_") => parse_indexed(s, "pm_of_", ModDestination::PmAmountOf),
        s if s.starts_with("am_of_") => parse_indexed(s, "am_of_", ModDestination::AmAmountOf),
        s if s.starts_with("rm_of_") => parse_indexed(s, "rm_of_", ModDestination::RmAmountOf),
        _ => None,
    }
}

fn parse_indexed(s: &str, prefix: &str, ctor: fn(u8) -> ModDestination) -> Option<ModDestination> {
    s.strip_prefix(prefix)
        .and_then(|i| i.parse::<u8>().ok())
        .map(ctor)
}

fn destination_options() -> Vec<SelectOption> {
    let mut out = vec![
        SelectOption::new(
            mod_destination_value(ModDestination::FilterCutoff),
            mod_destination_label(ModDestination::FilterCutoff),
        ),
        SelectOption::new(
            mod_destination_value(ModDestination::FilterResonance),
            mod_destination_label(ModDestination::FilterResonance),
        ),
    ];
    for i in 0..3u8 {
        out.push(SelectOption::new(
            mod_destination_value(ModDestination::OscLevel(i)),
            mod_destination_label(ModDestination::OscLevel(i)),
        ));
    }
    for i in 0..3u8 {
        out.push(SelectOption::new(
            mod_destination_value(ModDestination::OscTune(i)),
            mod_destination_label(ModDestination::OscTune(i)),
        ));
    }
    for i in 0..3u8 {
        out.push(SelectOption::new(
            mod_destination_value(ModDestination::OscFineTune(i)),
            mod_destination_label(ModDestination::OscFineTune(i)),
        ));
    }
    for i in 0..3u8 {
        out.push(SelectOption::new(
            mod_destination_value(ModDestination::PmAmountOf(i)),
            mod_destination_label(ModDestination::PmAmountOf(i)),
        ));
    }
    for i in 0..3u8 {
        out.push(SelectOption::new(
            mod_destination_value(ModDestination::AmAmountOf(i)),
            mod_destination_label(ModDestination::AmAmountOf(i)),
        ));
    }
    for i in 0..3u8 {
        out.push(SelectOption::new(
            mod_destination_value(ModDestination::RmAmountOf(i)),
            mod_destination_label(ModDestination::RmAmountOf(i)),
        ));
    }
    out.push(SelectOption::new(
        mod_destination_value(ModDestination::LfoRate),
        mod_destination_label(ModDestination::LfoRate),
    ));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_string_round_trips_every_variant() {
        for src in [
            ModSource::None,
            ModSource::Env1,
            ModSource::Env2,
            ModSource::Env3,
            ModSource::Lfo1,
            ModSource::Osc0,
            ModSource::Osc1,
            ModSource::Osc2,
        ] {
            let s = mod_source_value(src);
            assert_eq!(decode_source_str(s), Some(src), "round trip failed for {s}");
        }
    }

    #[test]
    fn destination_string_round_trips_every_variant() {
        let mut dests = vec![
            ModDestination::FilterCutoff,
            ModDestination::FilterResonance,
            ModDestination::LfoRate,
        ];
        for i in 0..3u8 {
            dests.push(ModDestination::OscLevel(i));
            dests.push(ModDestination::OscTune(i));
            dests.push(ModDestination::OscFineTune(i));
            dests.push(ModDestination::PmAmountOf(i));
            dests.push(ModDestination::AmAmountOf(i));
            dests.push(ModDestination::RmAmountOf(i));
        }
        for dst in dests {
            let s = mod_destination_value(dst);
            assert_eq!(
                decode_destination_str(&s),
                Some(dst),
                "round trip failed for {s}"
            );
        }
    }

    #[test]
    fn destination_option_list_covers_every_variant() {
        // The dropdown must offer every variant the synth supports;
        // otherwise the user can't represent a patch they could
        // construct from MIDI Learn / preset load.
        let opts = destination_options();
        // 2 (filter) + 6×3 (osc-targeted) + 1 (lfo) = 21
        assert_eq!(opts.len(), 21);
        // Every option's value must round-trip.
        for opt in &opts {
            assert!(
                decode_destination_str(&opt.value).is_some(),
                "destination option {} must decode",
                opt.value
            );
        }
    }

    #[test]
    fn source_option_list_covers_every_variant() {
        let opts = source_options();
        assert_eq!(opts.len(), 8);
        for opt in &opts {
            assert!(
                decode_source_str(&opt.value).is_some(),
                "source option {} must decode",
                opt.value
            );
        }
    }
}
