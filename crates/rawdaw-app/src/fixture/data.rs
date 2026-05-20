//! Round-1 fixture adapter: builds the UI's `Round1` view from
//! `rawdaw_model::fixtures::build_round1_project()` plus an inline
//! presentation overlay.
//!
//! Phase E2 step 2: the static UI tables are gone. The model is now
//! authoritative for the round-1 project's ids / names / structure
//! (tracks, patterns, chord-loop events, sections, arrangement). The
//! overlay below carries UI-only fields the model doesn't own —
//! per-pattern / -section / -chord-loop colors, the library's
//! `Pitched · N variants` meta strings, the round-2 cell realization
//! decorations (voicing / octave / humanization per cell), and the
//! transient project-meta fields the UI top bar surfaces (project
//! name, tempo, playhead) that don't yet live in the model.
//!
//! ## Mapping rules
//!
//! - `Track.id` is `format!("t_{}", model_track.name)` — preserves the
//!   round-1 fixture's `t_bass` / `t_lead` / `t_drums` / `t_pad` form
//!   so existing UI lookups by id continue to work.
//! - `Pattern.id` / `Section.id` / `ChordLoop.id`: model `name` strings
//!   (unique per kind in round 1).
//! - `Activation.state`: derived from `ActivationEntry.pattern_ref` —
//!   `Some(p)` ⇒ `Active`, `None` ⇒ `Silent`. A track that has no
//!   entry at all in the section's `activations` is **absent** from
//!   the UI section's activation list; the section editor renders the
//!   dashed `inherit` placeholder for those (round-2 decision 14).
//! - `Activation.pattern`: model `Pattern.name` resolved via
//!   `pattern_ref`. Empty string when `pattern_ref` is `None`.
//! - `ScheduleEntry.variant = None` when the model's variant id is the
//!   sentinel `__silent__` (sub-range silence), otherwise
//!   `Some(variant_id.as_str().to_string())`.

use rawdaw_model::activation::ActivationEntry as ModelActivation;
use rawdaw_model::chord::ChordSpec;
use rawdaw_model::fixtures::{build_round1_project, Round1Keys};
use rawdaw_model::id::TrackId;
use rawdaw_model::pattern::{Pattern as ModelPattern, PatternBody};
use rawdaw_model::project::Project as ModelProject;
use rawdaw_model::scale::Scale;
use rawdaw_model::section::{
    ActivationOverride as ModelActivationOverride, Section as ModelSection,
};
use rawdaw_model::track::{Role, Track as ModelTrack, TrackKind as ModelTrackKind};

use super::chord_naming::{absolute_label, pitch_class_name, quality_suffix, roman_label};
use super::{
    Activation, ActivationOverride, ActivationState, ChordEvent, ChordLoop, Pattern, Project,
    Round1, ScheduleEntry, Section, SectionRef, Track, TrackKind, Variant, VariantOverride,
};
use crate::initial_project::overlay::build_round1_overlay;
use crate::overlay::{CellOverlay, ProjectOverlay};
use crate::theme;

// ─── Entry point ──────────────────────────────────────────────────────────

pub(super) fn build_round1() -> Round1 {
    let (project, keys) = build_round1_project();
    let overlay = build_round1_overlay(&keys);
    Round1 {
        project: build_project_meta(),
        tracks: build_tracks(&project),
        patterns: build_patterns(&project, &overlay),
        chord_loops: build_chord_loops(&project, &overlay),
        sections: build_sections(&project, &keys, &overlay),
        arrangement: build_arrangement(&project),
        total_bars: arrangement_total_bars(&project),
    }
}

// Round-1 overlay construction lives in `crate::initial_project::overlay`
// (moved in C1b). `build_round1` calls it via the imported helper.

// ─── Project meta (top-bar fields the model doesn't carry yet) ───────────

fn build_project_meta() -> Project {
    // Tempo and playhead don't yet live on the model `Project`. The
    // round-1 mockup pins these; the engine-wiring milestone surfaces
    // real values in phases E4–E5.
    Project {
        name: "untitled-1".into(),
        key: "C major".into(),
        time_sig: "4/4".into(),
        tempo: 96,
        playhead_bar: 5,
        playhead_beat: 2,
    }
}

// ─── Tracks ───────────────────────────────────────────────────────────────

fn build_tracks(project: &ModelProject) -> Vec<Track> {
    project.tracks.iter().map(build_track).collect()
}

fn build_track(m: &ModelTrack) -> Track {
    let (kind, role) = match &m.kind {
        ModelTrackKind::Pitched { role } => (TrackKind::Pitched, role_name(*role).to_string()),
        ModelTrackKind::Drum { .. } => (TrackKind::Drum, "—".to_string()),
    };
    Track {
        id: track_ui_id(m),
        name: m.name.clone(),
        kind,
        role,
    }
}

fn track_ui_id(m: &ModelTrack) -> String {
    format!("t_{}", m.name)
}

fn role_name(role: Role) -> &'static str {
    match role {
        Role::Bass => "bass",
        Role::Voicing => "voicing",
        Role::Arp => "arp",
        Role::Melodic => "melodic",
        Role::Pad => "pad",
        Role::Countermelody => "countermel",
        Role::Other => "other",
    }
}

// ─── Patterns ─────────────────────────────────────────────────────────────

fn build_patterns(project: &ModelProject, overlay: &ProjectOverlay) -> Vec<Pattern> {
    project
        .patterns
        .values()
        .map(|p| build_pattern(p, overlay))
        .collect()
}

fn build_pattern(m: &ModelPattern, overlay: &ProjectOverlay) -> Pattern {
    let (kind_label, variants) = match &m.body {
        PatternBody::Pitched(b) => ("Pitched", b.variants.len() as u32),
        PatternBody::Drum(b) => ("Drum", b.variants.len() as u32),
    };
    let color = overlay
        .pattern_color
        .get(&m.id)
        .cloned()
        .unwrap_or_else(|| theme::TEXT2.to_string());
    let meta = overlay
        .pattern_meta
        .get(&m.id)
        .cloned()
        .unwrap_or_default();
    Pattern {
        id: m.name.clone(),
        name: m.name.clone(),
        color,
        kind: kind_label.into(),
        variants,
        default_variant: m.default_variant.as_str().to_string(),
        meta,
    }
}

// ─── Chord loops ──────────────────────────────────────────────────────────

fn build_chord_loops(project: &ModelProject, overlay: &ProjectOverlay) -> Vec<ChordLoop> {
    project
        .chord_loops
        .values()
        .map(|cl| {
            let scale = effective_scale(project, None);
            let events = cl.events.iter().map(|e| build_chord_event(e, &scale)).collect();
            let color = overlay
                .chord_loop_color
                .get(&cl.id)
                .cloned()
                .unwrap_or_else(|| theme::TEXT2.to_string());
            ChordLoop {
                id: cl.name.clone(),
                name: cl.name.clone(),
                color,
                length_bars: 4, // round-1 fixture: every loop is 4 bars (1 beat per chord × 4).
                events,
            }
        })
        .collect()
}

fn build_chord_event(e: &rawdaw_model::chord::ChordEvent, project_scale: &Scale) -> ChordEvent {
    match &e.chord {
        ChordSpec::Functional { roman, suffix, in_key } => {
            let scale = in_key.as_ref().unwrap_or(project_scale);
            ChordEvent {
                roman: roman_label(*roman, &suffix.quality),
                quality: "".into(), // round-1 fixture uses no extension/alteration markers
                absolute: absolute_label(*roman, &suffix.quality, scale),
            }
        }
        ChordSpec::Absolute { root, suffix } => ChordEvent {
            roman: "".into(),
            quality: "".into(),
            absolute: format!("{}{}", pitch_class_name(*root), quality_suffix(&suffix.quality)),
        },
    }
}

fn effective_scale(project: &ModelProject, section_override: Option<&Scale>) -> Scale {
    section_override.cloned().unwrap_or_else(|| project.default_key.clone())
}

// ─── Sections ─────────────────────────────────────────────────────────────

fn build_sections(
    project: &ModelProject,
    keys: &Round1Keys,
    overlay: &ProjectOverlay,
) -> Vec<Section> {
    // Walk in the order intro / verse / chorus to match the round-1
    // fixture. The model's BTreeMap order is by SectionId, which lines
    // up because the fixture allocates intro first, then verse, then
    // chorus — but we anchor explicitly via `keys` to avoid coupling to
    // the BTreeMap's insertion-id order.
    [keys.sections.intro, keys.sections.verse, keys.sections.chorus]
        .into_iter()
        .map(|id| build_section(&project.sections[&id], project, overlay))
        .collect()
}

fn build_section(s: &ModelSection, project: &ModelProject, overlay: &ProjectOverlay) -> Section {
    let color = overlay
        .section_color
        .get(&s.id)
        .cloned()
        .unwrap_or_else(|| theme::TEXT2.to_string());
    let variants = section_variants(s);
    let chord_loops = section_chord_loops(s, project);
    let activations = section_activations(s, project, overlay);
    let variant_overrides = section_variant_overrides(s, project, overlay);
    Section {
        id: s.name.clone(),
        name: s.name.clone(),
        color,
        variants,
        default_variant: s.default_variant.as_str().to_string(),
        base_duration_bars: s.base.duration_bars,
        chord_loops,
        activations,
        variant_overrides,
    }
}

fn section_variants(s: &ModelSection) -> Vec<Variant> {
    // The UI's variant list always includes "base" (the default) plus
    // every named variant from the model's `variants` map. The model
    // doesn't store a `base` entry — it's implicit via
    // `default_variant`.
    let mut out = vec![Variant { id: "base".into(), name: "base".into() }];
    for v in s.variants.keys() {
        out.push(Variant {
            id: v.as_str().to_string(),
            name: v.as_str().to_string(),
        });
    }
    out
}

fn section_chord_loops(s: &ModelSection, project: &ModelProject) -> Vec<String> {
    s.base
        .chord_loops
        .iter()
        .map(|(_, id)| project.chord_loops[id].name.clone())
        .collect()
}

fn section_activations(
    s: &ModelSection,
    project: &ModelProject,
    overlay: &ProjectOverlay,
) -> Vec<(String, Activation)> {
    s.base
        .activations
        .iter()
        .map(|(track_id, entry)| {
            let track = project_track(project, *track_id);
            (
                track_ui_id(track),
                activation_from_model(entry, project, overlay.lookup_cell(s.id, "base", *track_id)),
            )
        })
        .collect()
}

fn section_variant_overrides(
    s: &ModelSection,
    project: &ModelProject,
    overlay: &ProjectOverlay,
) -> Vec<(String, VariantOverride)> {
    s.variants
        .iter()
        .map(|(vid, sov)| {
            let variant_str = vid.as_str().to_string();
            // The variant-name lookup against the overlay uses the
            // model's variant id verbatim. The overlay's keys are
            // `&'static str` literals — see `lookup_cell` for the
            // string-compare fallback path.
            let list: VariantOverride = sov
                .activations
                .iter()
                .map(|(tid, ov)| {
                    let track = project_track(project, *tid);
                    let cell = overlay.lookup_cell(s.id, vid.as_str(), *tid);
                    let ui_ov = match ov {
                        ModelActivationOverride::Silent => ActivationOverride::Silent,
                        ModelActivationOverride::Replace(entry) => ActivationOverride::Replace({
                            let mut act = activation_from_model(entry, project, cell);
                            act.overridden = true;
                            act
                        }),
                    };
                    (track_ui_id(track), ui_ov)
                })
                .collect();
            (variant_str, list)
        })
        .collect()
}

/// Build the UI's `Activation` from a model `ActivationEntry`.
///
/// State derives from `pattern_ref`: `Some` ⇒ Active, `None` ⇒ Silent.
/// Realization decorations come from the overlay when one is registered
/// for the cell; otherwise the activation carries `realization: None`
/// (round-1-style; the round-2 cell still renders, just without the
/// `↳ role default` / `*` inheritance markers).
fn activation_from_model(
    entry: &ModelActivation,
    project: &ModelProject,
    cell: Option<CellOverlay>,
) -> Activation {
    let (pattern, state) = match entry.pattern_ref {
        Some(pid) => (project.patterns[&pid].name.clone(), ActivationState::Active),
        None => (String::new(), ActivationState::Silent),
    };
    let variant_schedule = entry
        .variant_schedule
        .iter()
        .map(|(range, vid)| ScheduleEntry {
            start_bar: range.start,
            end_bar: range.end,
            variant: if vid.as_str() == "__silent__" {
                None
            } else {
                Some(vid.as_str().to_string())
            },
        })
        .collect();
    let (realization, pinned) = cell
        .map(|c| (Some(c.realization), c.pinned))
        .unwrap_or((None, 0));
    Activation {
        pattern,
        state,
        overridden: false,
        realization,
        variant_schedule,
        per_note_overrides: pinned,
    }
}

fn project_track(project: &ModelProject, id: TrackId) -> &ModelTrack {
    project
        .tracks
        .iter()
        .find(|t| t.id == id)
        .unwrap_or_else(|| {
            panic!("track id {id:?} from section activation must exist in project.tracks")
        })
}

// ─── Arrangement ──────────────────────────────────────────────────────────

fn build_arrangement(project: &ModelProject) -> Vec<SectionRef> {
    project
        .arrangement
        .sections
        .iter()
        .enumerate()
        .map(|(idx, sr)| {
            let section = &project.sections[&sr.section];
            // 4/4 throughout the round-1 fixture: 4 beats per bar. The
            // UI's bar-aligned arrangement assumes integer bar starts.
            let start_bar = (sr.start.as_beats_f64() / 4.0) as u32;
            SectionRef {
                idx,
                section_key: section.name.clone(),
                variant: sr.variant.as_str().to_string(),
                start_bar,
                bars: section.base.duration_bars,
            }
        })
        .collect()
}

fn arrangement_total_bars(project: &ModelProject) -> u32 {
    let last = project
        .arrangement
        .sections
        .last()
        .expect("non-empty arrangement");
    let section = &project.sections[&last.section];
    let last_start_bar = (last.start.as_beats_f64() / 4.0) as u32;
    last_start_bar + section.base.duration_bars
}

// ─── Tests ────────────────────────────────────────────────────────────────
//
// Round-2 fixture invariants pinned to the model-driven adapter. These
// match the prior static-fixture tests but exercise the adapter
// alongside `build_round1_project()`.

#[cfg(test)]
mod tests {
    use crate::fixture::{
        base_activation, round1, variant_override, ActivationOverride, ActivationState, Section,
    };
    use crate::overlay::{role_defaults, OctaveSpec, Voicing};

    fn section(name: &str) -> &'static Section {
        round1()
            .sections
            .iter()
            .find(|s| s.name == name)
            .unwrap_or_else(|| panic!("section {name} is part of the fixture"))
    }

    #[test]
    fn pad_has_no_verse_base_entry() {
        // Per round-2 decision 14: the pad track is deliberately absent
        // from verse's activation list so the section editor exercises
        // the dashed-border inherit placeholder path.
        assert!(
            base_activation(section("verse"), "t_pad").is_none(),
            "pad must NOT have an entry in verse.activations — round-2 \
             expects the dashed-border placeholder"
        );
    }

    #[test]
    fn stripped_lead_schedule_has_only_silent_sub_range() {
        // Per round-2 decision 20: the only explicit entry on stripped
        // lead is the sub-range silence at bar 4. Bars 1–3 are an
        // implicit-default fill computed at render time — there must be
        // NO phantom "main" entry stored.
        let ov = variant_override(section("verse"), "stripped", "t_lead")
            .expect("stripped variant overrides the lead activation");
        let act = match ov {
            ActivationOverride::Replace(a) => a,
            ActivationOverride::Silent => panic!("stripped lead is Replace, not Silent"),
        };
        assert_eq!(
            act.variant_schedule.len(),
            1,
            "stripped lead schedule must hold exactly one entry"
        );
        let e = &act.variant_schedule[0];
        assert_eq!(e.start_bar, 3);
        assert_eq!(e.end_bar, 4);
        assert_eq!(e.variant, None, "the one entry is a silenced sub-range");
    }

    #[test]
    fn chorus_drums_schedule_has_only_fill_entry() {
        // Same rule on the drum-fill case: the stored schedule is just
        // the non-default `fill` entry at bar 8. Bars 1–7 are the
        // implicit default.
        let drums =
            base_activation(section("chorus"), "t_drums").expect("chorus drums activation");
        assert_eq!(
            drums.variant_schedule.len(),
            1,
            "chorus drums schedule must hold exactly one entry"
        );
        let e = &drums.variant_schedule[0];
        assert_eq!(e.start_bar, 7);
        assert_eq!(e.end_bar, 8);
        assert_eq!(e.variant.as_deref(), Some("fill"));
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
        for p in round1().patterns.iter() {
            assert!(
                !p.default_variant.is_empty(),
                "pattern {} is missing default_variant",
                p.name
            );
        }
    }

    #[test]
    fn intro_has_pad_active_and_other_tracks_silent() {
        // The model now carries pattern_ref:None silent entries for
        // bass/lead/drums in intro; the adapter maps those to
        // ActivationState::Silent (with an empty pattern string) so
        // the round-1 inspector renders a `silent` pill rather than
        // the dashed `inherit` placeholder for those tracks.
        let intro = section("intro");
        let names: Vec<&str> = intro
            .activations
            .iter()
            .map(|(tid, _)| tid.as_str())
            .collect();
        assert!(names.contains(&"t_pad"));
        assert!(names.contains(&"t_bass"));
        assert!(names.contains(&"t_lead"));
        assert!(names.contains(&"t_drums"));
        let pad_state = base_activation(intro, "t_pad").map(|a| a.state);
        let bass_state = base_activation(intro, "t_bass").map(|a| a.state);
        assert_eq!(pad_state, Some(ActivationState::Active));
        assert_eq!(bass_state, Some(ActivationState::Silent));
    }

    #[test]
    fn chord_loop_events_resolve_in_c_major() {
        // verse-progression = I V vi IV in C major.
        let r = round1();
        let verse_loop = r
            .chord_loops
            .iter()
            .find(|c| c.name == "verse-progression")
            .expect("verse-progression in fixture");
        let romans: Vec<&str> = verse_loop.events.iter().map(|e| e.roman.as_str()).collect();
        assert_eq!(romans, vec!["I", "V", "vi", "IV"]);
        let abs: Vec<&str> = verse_loop.events.iter().map(|e| e.absolute.as_str()).collect();
        assert_eq!(abs, vec!["C", "G", "Am", "F"]);
    }

    #[test]
    fn arrangement_has_five_steps_24_bars() {
        let r = round1();
        assert_eq!(r.arrangement.len(), 5);
        assert_eq!(r.total_bars, 24);
        assert_eq!(r.arrangement[0].section_key, "intro");
        assert_eq!(r.arrangement[2].variant, "stripped");
        assert_eq!(r.arrangement[4].section_key, "chorus");
        assert_eq!(r.arrangement[4].bars, 8);
    }
}
