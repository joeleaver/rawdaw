//! K2 MIDI input picker for the TopBar.
//!
//! Reads the available device list (refreshed at render time via
//! [`AudioResources::available_midi_inputs`]) and binds the dropdown's
//! selected value to [`AudioResources::current_midi_device`] reactively.
//! Picking an entry calls `set_midi_device(Some(name))`; picking the
//! "None" sentinel calls `set_midi_device(None)` to disconnect.
//!
//! Visual styling is intentionally placeholder per the
//! `project_ui_redesign_pending` memo. The redesign pass replaces
//! chrome, not API.

use rinch::prelude::*;

use crate::audio::AudioResources;
use crate::theme;

use super::Tag;

/// Sentinel value for the "no MIDI device" option in [`MidiPicker`].
/// Picked as an empty string so it's distinguishable from any real
/// device name (midir wraps a non-empty string for every enumerated
/// port — see `MidiInputDevice::name`).
const MIDI_NONE_VALUE: &str = "";

#[component]
pub(super) fn MidiPicker() -> NodeHandle {
    let wrap_style = format!(
        "display: flex; align-items: center; gap: 6px; \
         padding: 0 6px 0 9px; border-radius: 4px; \
         background: {bg0}; border: 1px solid {line}; \
         font-size: 12px; color: rgba(232,234,238,0.96);",
        bg0 = theme::BG0,
        line = theme::LINE,
    );
    rsx! {
        div { style: {wrap_style.clone()},
            Tag { text: "MIDI" }
            Select {
                size: "sm",
                value_fn: {|| use_store::<AudioResources>()
                    .current_midi_device
                    .get()
                    .unwrap_or_else(|| MIDI_NONE_VALUE.to_string())
                },
                data: midi_picker_options(),
                onchange: move |v: String| {
                    let audio = use_store::<AudioResources>();
                    if v == MIDI_NONE_VALUE {
                        audio.set_midi_device(None);
                    } else {
                        audio.set_midi_device(Some(&v));
                    }
                },
            }
        }
    }
}

/// Build the option list for [`MidiPicker`]. The "None" sentinel
/// lives first so the dropdown opens with disconnect as the
/// affordance when no device is selected. midir's enumeration runs
/// every render — cheap; one ALSA query.
fn midi_picker_options() -> Vec<SelectOption> {
    let audio = use_store::<AudioResources>();
    let mut out: Vec<SelectOption> =
        vec![SelectOption::new(MIDI_NONE_VALUE, "None")];
    for name in audio.available_midi_inputs() {
        out.push(SelectOption::new(name.clone(), name));
    }
    out
}
