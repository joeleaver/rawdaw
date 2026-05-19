//! Factory preset bank — U8 of the synth-UI integration plan.
//!
//! Embeds JSON patch files from `assets/presets/{wavetable,drum}/*.json`
//! into the binary at compile time via `include_str!`. Each preset
//! parses once at first call into a strongly-typed `…PatchData` and
//! pairs with a display name; both per-synth lists are returned by
//! [`wavetable_presets()`] / [`drum_presets()`].
//!
//! v1 is read-only and binary-embedded — no filesystem path, no
//! user-saved presets. v2 (out of scope here) can fan out to a
//! user-preset directory that overlays the factory bank.
//!
//! ## Why include_str! and not a filesystem read at startup
//!
//! - Self-contained binary: no `assets/` folder to ship alongside.
//! - Parse failures surface at compile time the moment a preset is
//!   added — the test in this module deserializes every preset and
//!   fails the build if one is malformed.
//! - Cheap at runtime: parsing happens once per preset on first
//!   access; clones are `Copy` on the patch types.

use std::sync::OnceLock;

use rawdaw_model::patch::drum::DrumPatchData;
use rawdaw_model::patch::wavetable::WavetablePatchData;

/// One preset entry — display name + parsed patch data.
#[derive(Debug, Clone)]
pub struct WavetablePreset {
    pub name: &'static str,
    pub data: WavetablePatchData,
}

#[derive(Debug, Clone)]
pub struct DrumPreset {
    pub name: &'static str,
    pub data: DrumPatchData,
}

/// Factory wavetable presets, parsed once on first call. The order
/// here is the display order in the editor's preset dropdown —
/// "default" first so the dropdown's initial value matches the
/// patch every track boots with.
pub fn wavetable_presets() -> &'static [WavetablePreset] {
    static PRESETS: OnceLock<Vec<WavetablePreset>> = OnceLock::new();
    PRESETS.get_or_init(|| {
        vec![
            WavetablePreset {
                name: "default",
                data: parse_wavetable(include_str!("../assets/presets/wavetable/default.json")),
            },
            WavetablePreset {
                name: "pluck",
                data: parse_wavetable(include_str!("../assets/presets/wavetable/pluck.json")),
            },
            WavetablePreset {
                name: "pluck-bass",
                data: parse_wavetable(include_str!("../assets/presets/wavetable/pluck-bass.json")),
            },
            WavetablePreset {
                name: "bass",
                data: parse_wavetable(include_str!("../assets/presets/wavetable/bass.json")),
            },
            WavetablePreset {
                name: "sub",
                data: parse_wavetable(include_str!("../assets/presets/wavetable/sub.json")),
            },
            WavetablePreset {
                name: "lead",
                data: parse_wavetable(include_str!("../assets/presets/wavetable/lead.json")),
            },
            WavetablePreset {
                name: "wobble",
                data: parse_wavetable(include_str!("../assets/presets/wavetable/wobble.json")),
            },
            WavetablePreset {
                name: "poly",
                data: parse_wavetable(include_str!("../assets/presets/wavetable/poly.json")),
            },
            WavetablePreset {
                name: "bell",
                data: parse_wavetable(include_str!("../assets/presets/wavetable/bell.json")),
            },
            WavetablePreset {
                name: "pad",
                data: parse_wavetable(include_str!("../assets/presets/wavetable/pad.json")),
            },
        ]
    })
}

/// Factory drum presets, parsed once on first call.
pub fn drum_presets() -> &'static [DrumPreset] {
    static PRESETS: OnceLock<Vec<DrumPreset>> = OnceLock::new();
    PRESETS.get_or_init(|| {
        vec![
            DrumPreset {
                name: "default",
                data: parse_drum(include_str!("../assets/presets/drum/default.json")),
            },
            DrumPreset {
                name: "acoustic",
                data: parse_drum(include_str!("../assets/presets/drum/acoustic.json")),
            },
            DrumPreset {
                name: "electronic",
                data: parse_drum(include_str!("../assets/presets/drum/electronic.json")),
            },
            DrumPreset {
                name: "808",
                data: parse_drum(include_str!("../assets/presets/drum/808.json")),
            },
            DrumPreset {
                name: "909",
                data: parse_drum(include_str!("../assets/presets/drum/909.json")),
            },
            DrumPreset {
                name: "lo-fi",
                data: parse_drum(include_str!("../assets/presets/drum/lo-fi.json")),
            },
        ]
    })
}

fn parse_wavetable(json: &str) -> WavetablePatchData {
    serde_json::from_str(json).expect("shipped wavetable preset must parse")
}

fn parse_drum(json: &str) -> DrumPatchData {
    serde_json::from_str(json).expect("shipped drum preset must parse")
}

/// Look up a wavetable preset by name. Returns `None` if no entry
/// matches — UI callers fall back to the "default" preset.
pub fn wavetable_preset_by_name(name: &str) -> Option<&'static WavetablePreset> {
    wavetable_presets().iter().find(|p| p.name == name)
}

/// Look up a drum preset by name.
pub fn drum_preset_by_name(name: &str) -> Option<&'static DrumPreset> {
    drum_presets().iter().find(|p| p.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_wavetable_preset_parses() {
        let presets = wavetable_presets();
        assert!(presets.len() >= 5, "expected at least the 5 starter presets");
        // The default preset must round-trip to WavetablePatchData::default()
        // — a regression here would mean the bundled JSON drifted away
        // from the runtime default the synth boots with.
        let default = wavetable_preset_by_name("default").expect("default present");
        assert_eq!(default.data, WavetablePatchData::default());
    }

    #[test]
    fn every_drum_preset_parses() {
        let presets = drum_presets();
        assert!(presets.len() >= 3, "expected at least the 3 starter kits");
        let default = drum_preset_by_name("default").expect("default present");
        assert_eq!(default.data, DrumPatchData::default());
    }

    #[test]
    fn wavetable_preset_names_are_unique() {
        let names: Vec<&str> = wavetable_presets().iter().map(|p| p.name).collect();
        let mut sorted = names.clone();
        sorted.sort();
        let mut deduped = sorted.clone();
        deduped.dedup();
        assert_eq!(sorted, deduped, "preset names must be unique");
    }

    #[test]
    fn drum_preset_names_are_unique() {
        let names: Vec<&str> = drum_presets().iter().map(|p| p.name).collect();
        let mut sorted = names.clone();
        sorted.sort();
        let mut deduped = sorted.clone();
        deduped.dedup();
        assert_eq!(sorted, deduped, "drum preset names must be unique");
    }

    #[test]
    fn wavetable_lookup_returns_none_for_unknown() {
        assert!(wavetable_preset_by_name("nope").is_none());
    }
}
