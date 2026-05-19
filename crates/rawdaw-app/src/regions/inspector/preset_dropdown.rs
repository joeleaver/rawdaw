//! Preset switcher dropdown — U8.
//!
//! One row at the top of each synth editor with a `Select` listing
//! the factory presets shipped under `assets/presets/`. Picking a
//! preset calls
//! [`AudioResources::apply_wavetable_preset`](crate::audio::AudioResources::apply_wavetable_preset)
//! or [`AudioResources::apply_drum_preset`](crate::audio::AudioResources::apply_drum_preset)
//! which flatten the patch into per-field `BlockMessage::Param`
//! events and push them at `sample_clock + 1` so the change lands
//! atomically inside the next audio block.
//!
//! The dropdown's `Signal<String>` updates immediately on pick;
//! the editor's per-control signals are not yet bound to the
//! patch_signal poll path (deferred from U5/U7), so the slider
//! UI will appear stale until the editor re-mounts. The plan
//! addresses this in a future "bind slider signals to patch poll"
//! pass — out of scope for U8, called out in the project status
//! memory.

use rinch::prelude::*;

use crate::audio::AudioResources;
use crate::presets;
use crate::theme;

/// Which preset bank this dropdown drives.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum PresetKind {
    #[default]
    Wavetable,
    Drum,
}

#[component]
pub fn PresetDropdown(track_idx: usize, kind: PresetKind) -> NodeHandle {
    let audio = use_store::<AudioResources>();
    let audio_for_change = audio.clone();
    let selected = Signal::new("default".to_string());

    // Pre-compute option list; SelectOption is `Clone + PartialEq`
    // so we can pass an owned Vec to the Select component.
    let data: Vec<SelectOption> = match kind {
        PresetKind::Wavetable => presets::wavetable_presets()
            .iter()
            .map(|p| SelectOption::new(p.name, capitalize(p.name)))
            .collect(),
        PresetKind::Drum => presets::drum_presets()
            .iter()
            .map(|p| SelectOption::new(p.name, capitalize(p.name)))
            .collect(),
    };

    let row_style = format!(
        "display: grid; grid-template-columns: 64px 1fr; \
         gap: 10px; align-items: center; \
         padding: 8px 0 10px; \
         border-bottom: 1px solid {line};",
        line = theme::LINE,
    );
    let label_style = format!(
        "font-size: 10.5px; color: {text2}; \
         letter-spacing: 0.4px; text-transform: uppercase;",
        text2 = theme::TEXT2,
    );

    rsx! {
        div { style: {row_style.clone()},
            span { style: {label_style.clone()}, "Preset" }
            Select {
                size: "sm",
                value_fn: move || selected.get(),
                data: data,
                onchange: move |v: String| {
                    selected.set(v.clone());
                    let result = match kind {
                        PresetKind::Wavetable => presets::wavetable_preset_by_name(&v)
                            .map(|p| audio_for_change.apply_wavetable_preset(track_idx, p.data)),
                        PresetKind::Drum => presets::drum_preset_by_name(&v)
                            .map(|p| audio_for_change.apply_drum_preset(track_idx, p.data)),
                    };
                    // Best-effort: a preset lookup miss or event-queue
                    // overflow only means the audio thread missed the
                    // patch swap. Surface neither as a UI failure for
                    // now; future preset-error toasts can read off a
                    // dedicated channel.
                    let _ = result;
                },
            }
        }
    }
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}
