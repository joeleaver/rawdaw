//! Round-1 demo-project invariants pinned through the production
//! [`build_initial`] factory. These tests originated as `fixture/data.rs`
//! adapter-pin tests; after the fixture was dismantled in C1c they live
//! here so the boot path (model + overlay) keeps producing the same
//! shape the rest of the UI relies on.

use rawdaw_model::chord::ChordSpec;
use rawdaw_model::id::VariantId;
use rawdaw_model::section::ActivationOverride;

use crate::chord_display::{absolute_label, roman_label};
use crate::initial_project::build_initial;
use crate::overlay::{role_defaults, OctaveSpec, Voicing};

const SILENT_VARIANT_SENTINEL: &str = "__silent__";

#[test]
fn pad_has_no_verse_base_entry() {
    // Per round-2 decision 14: the pad track is deliberately absent
    // from verse's activation list so the section editor exercises the
    // dashed-border inherit placeholder path.
    let (project, _) = build_initial();
    let verse = project
        .sections
        .values()
        .find(|s| s.name == "verse")
        .expect("verse exists");
    let pad = project
        .tracks
        .iter()
        .find(|t| t.name == "pad")
        .expect("pad track")
        .id;
    assert!(
        !verse.base.activations.contains_key(&pad),
        "pad must NOT have an entry in verse.activations — round-2 \
         expects the dashed-border placeholder"
    );
}

#[test]
fn stripped_lead_schedule_has_only_silent_sub_range() {
    // Per round-2 decision 20: the only explicit entry on stripped lead
    // is the sub-range silence at bar 4. Bars 1–3 are an implicit-
    // default fill computed at render time — there must be NO phantom
    // "main" entry stored.
    let (project, _) = build_initial();
    let verse = project
        .sections
        .values()
        .find(|s| s.name == "verse")
        .expect("verse exists");
    let lead = project
        .tracks
        .iter()
        .find(|t| t.name == "lead")
        .expect("lead track")
        .id;
    let stripped = VariantId::from("stripped");
    let variant_override = verse
        .variants
        .get(&stripped)
        .expect("stripped variant exists");
    let ov = variant_override
        .activations
        .get(&lead)
        .expect("stripped variant overrides lead activation");
    let entry = match ov {
        ActivationOverride::Replace(e) => e,
        ActivationOverride::Silent => panic!("stripped lead is Replace, not Silent"),
    };
    assert_eq!(
        entry.variant_schedule.len(),
        1,
        "stripped lead schedule must hold exactly one entry"
    );
    let (range, vid) = &entry.variant_schedule[0];
    assert_eq!(range.start, 3);
    assert_eq!(range.end, 4);
    assert_eq!(
        vid.as_str(),
        SILENT_VARIANT_SENTINEL,
        "the one entry is the silenced sub-range sentinel"
    );
}

#[test]
fn chorus_drums_schedule_has_only_fill_entry() {
    // Same rule on the drum-fill case: the stored schedule is just the
    // non-default `fill` entry at bar 8. Bars 1–7 are the implicit
    // default.
    let (project, _) = build_initial();
    let chorus = project
        .sections
        .values()
        .find(|s| s.name == "chorus")
        .expect("chorus exists");
    let drums = project
        .tracks
        .iter()
        .find(|t| t.name == "drums")
        .expect("drums track")
        .id;
    let entry = chorus
        .base
        .activations
        .get(&drums)
        .expect("chorus drums activation");
    assert_eq!(
        entry.variant_schedule.len(),
        1,
        "chorus drums schedule must hold exactly one entry"
    );
    let (range, vid) = &entry.variant_schedule[0];
    assert_eq!(range.start, 7);
    assert_eq!(range.end, 8);
    assert_eq!(vid.as_str(), "fill");
}

#[test]
fn role_defaults_match_round_2_table() {
    let bass = role_defaults("bass").expect("bass role is defined");
    assert_eq!(bass.voicing, Voicing::Power);
    assert_eq!(bass.octave, OctaveSpec::Nearest);
    let pad = role_defaults("pad").expect("pad role is defined");
    assert_eq!(pad.voicing, Voicing::TriadOpen);
    assert_eq!(pad.octave, OctaveSpec::Anchored(3));
    assert!(
        role_defaults("—").is_none(),
        "drums (no role) → no defaults"
    );
}

#[test]
fn patterns_all_carry_default_variant() {
    let (project, _) = build_initial();
    for p in project.patterns.values() {
        assert!(
            !p.default_variant.as_str().is_empty(),
            "pattern {} is missing default_variant",
            p.name
        );
    }
}

#[test]
fn intro_has_pad_active_and_other_tracks_silent() {
    // The model carries pattern_ref:None silent entries for
    // bass/lead/drums in intro; the section editor maps those to
    // ActivationState::Silent so the inspector renders a `silent`
    // pill rather than the dashed `inherit` placeholder.
    let (project, _) = build_initial();
    let intro = project
        .sections
        .values()
        .find(|s| s.name == "intro")
        .expect("intro exists");
    let lookup = |name: &str| {
        project
            .tracks
            .iter()
            .find(|t| t.name == name)
            .map(|t| t.id)
            .expect("track")
    };
    let pad = lookup("pad");
    let bass = lookup("bass");
    let lead = lookup("lead");
    let drums = lookup("drums");
    assert!(intro.base.activations.contains_key(&pad));
    assert!(intro.base.activations.contains_key(&bass));
    assert!(intro.base.activations.contains_key(&lead));
    assert!(intro.base.activations.contains_key(&drums));
    assert!(
        intro.base.activations[&pad].pattern_ref.is_some(),
        "pad has a pattern in intro"
    );
    assert!(
        intro.base.activations[&bass].pattern_ref.is_none(),
        "bass is silent in intro"
    );
}

#[test]
fn chord_loop_events_resolve_in_c_major() {
    // verse-progression = I V vi IV in C major.
    let (project, _) = build_initial();
    let verse_loop = project
        .chord_loops
        .values()
        .find(|cl| cl.name == "verse-progression")
        .expect("verse-progression in initial project");
    let (romans, absolutes): (Vec<_>, Vec<_>) = verse_loop
        .events
        .iter()
        .filter_map(|ev| match &ev.chord {
            ChordSpec::Functional {
                roman,
                suffix,
                in_key,
            } => {
                let scale = in_key.as_ref().unwrap_or(&project.default_key);
                Some((
                    roman_label(*roman, &suffix.quality),
                    absolute_label(*roman, &suffix.quality, scale),
                ))
            }
            ChordSpec::Absolute { .. } => None,
        })
        .unzip();
    assert_eq!(romans, vec!["I", "V", "vi", "IV"]);
    assert_eq!(absolutes, vec!["C", "G", "Am", "F"]);
}

#[test]
fn arrangement_has_five_steps() {
    let (project, _) = build_initial();
    assert_eq!(project.arrangement.sections.len(), 5);
    let third = &project.arrangement.sections[2];
    assert_eq!(third.variant.as_str(), "stripped");
}
