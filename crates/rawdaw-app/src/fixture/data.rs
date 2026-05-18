//! Round-1 fixture builder + round-2 invariant tests.
//!
//! Round-1 mockup data, mirroring
//! `docs/design/mockups/round-1/components/data.js`, extended with
//! the round-2 additions from
//! `docs/design/mockups/round-2/components/data.js`. The split between
//! this file and `mod.rs` is purely size — types and helpers stay in
//! `mod.rs`; the bulky construction lives here so neither file
//! approaches the ~700-line cap.
//!
//! Phase E2 type-flip: every fixture field is now `String` / `Vec`. The
//! construction runs once via `OnceLock`-backed `round1()`; the tests
//! pin the round-2 invariants the section editor port relies on.

use super::{
    Activation, ActivationOverride, ActivationState, ChordEvent, ChordLoop, Humanization,
    OctaveSpec, Pattern, Project, Realization, Round1, ScheduleEntry, Section, SectionRef, Track,
    TrackKind, Variant, VariantOverride, Voicing,
};
use crate::theme;

// ─── Entry point ──────────────────────────────────────────────────────────

pub(super) fn build_round1() -> Round1 {
    Round1 {
        project: Project {
            name: "untitled-1".into(),
            key: "C major".into(),
            time_sig: "4/4".into(),
            tempo: 96,
            playhead_bar: 5,
            playhead_beat: 2,
        },
        tracks: build_tracks(),
        patterns: build_patterns(),
        chord_loops: build_chord_loops(),
        sections: build_sections(),
        arrangement: build_arrangement(),
        total_bars: 24,
    }
}

// ─── Tracks / patterns / chord events / chord loops ───────────────────────

fn build_tracks() -> Vec<Track> {
    vec![
        Track { id: "t_bass".into(),  name: "bass".into(),  kind: TrackKind::Pitched, role: "bass".into()    },
        Track { id: "t_lead".into(),  name: "lead".into(),  kind: TrackKind::Pitched, role: "melodic".into() },
        Track { id: "t_drums".into(), name: "drums".into(), kind: TrackKind::Drum,    role: "—".into()       },
        Track { id: "t_pad".into(),   name: "pad".into(),   kind: TrackKind::Pitched, role: "pad".into()     },
    ]
}

fn build_patterns() -> Vec<Pattern> {
    vec![
        Pattern {
            id: "p_bass".into(),
            name: "bass-main".into(),
            color: theme::PAL_TEAL.into(),
            kind: "Pitched".into(),
            variants: 2,
            default_variant: "main".into(),
            meta: "Pitched · 2 variants".into(),
        },
        Pattern {
            id: "p_lead".into(),
            name: "lead-main".into(),
            color: theme::PAL_PLUM.into(),
            kind: "Pitched".into(),
            variants: 1,
            default_variant: "main".into(),
            meta: "Pitched · 1 variant".into(),
        },
        Pattern {
            id: "p_drums".into(),
            name: "drums-main".into(),
            color: theme::PAL_SAGE.into(),
            kind: "Drum".into(),
            variants: 2,
            default_variant: "main".into(),
            meta: "Drum · 2 variants".into(),
        },
        Pattern {
            id: "p_pad".into(),
            name: "pad-bed".into(),
            color: theme::PAL_SLATE.into(),
            kind: "Pitched".into(),
            variants: 1,
            default_variant: "main".into(),
            meta: "Pitched · 1 variant".into(),
        },
    ]
}

fn ce(roman: &str, quality: &str, absolute: &str) -> ChordEvent {
    ChordEvent {
        roman: roman.into(),
        quality: quality.into(),
        absolute: absolute.into(),
    }
}

fn build_chord_loops() -> Vec<ChordLoop> {
    vec![
        ChordLoop {
            id: "cl_verse".into(),
            name: "verse-progression".into(),
            color: theme::PAL_TERRA.into(),
            length_bars: 4,
            events: vec![
                ce("I",  "", "C"),
                ce("V",  "", "G"),
                ce("vi", "", "Am"),
                ce("IV", "", "F"),
            ],
        },
        ChordLoop {
            id: "cl_chorus".into(),
            name: "chorus-progression".into(),
            color: theme::PAL_OLIVE.into(),
            length_bars: 4,
            events: vec![
                ce("vi", "", "Am"),
                ce("IV", "", "F"),
                ce("I",  "", "C"),
                ce("V",  "", "G"),
            ],
        },
    ]
}

// ─── Activation helpers ──────────────────────────────────────────────────

fn silent_no_realization(pattern: &str) -> Activation {
    Activation {
        pattern: pattern.into(),
        state: ActivationState::Silent,
        overridden: false,
        realization: None,
        variant_schedule: Vec::new(),
        per_note_overrides: 0,
    }
}

fn active_no_realization(pattern: &str) -> Activation {
    Activation {
        pattern: pattern.into(),
        state: ActivationState::Active,
        overridden: false,
        realization: None,
        variant_schedule: Vec::new(),
        per_note_overrides: 0,
    }
}

fn active_pitched(
    pattern: &str,
    voicing: Voicing,
    octave: OctaveSpec,
    humanization: Humanization,
    per_note_overrides: u32,
) -> Activation {
    Activation {
        pattern: pattern.into(),
        state: ActivationState::Active,
        overridden: false,
        realization: Some(Realization {
            voicing: Some(voicing),
            octave: Some(octave),
            humanization,
        }),
        variant_schedule: Vec::new(),
        per_note_overrides,
    }
}

fn active_drum(pattern: &str, humanization: Humanization, schedule: Vec<ScheduleEntry>) -> Activation {
    Activation {
        pattern: pattern.into(),
        state: ActivationState::Active,
        overridden: false,
        realization: Some(Realization {
            voicing: None,
            octave: None,
            humanization,
        }),
        variant_schedule: schedule,
        per_note_overrides: 0,
    }
}

// ─── Sections ─────────────────────────────────────────────────────────────
//
// Round-2 mockup data, mirroring
// `docs/design/mockups/round-2/components/data.js`. The intro section
// keeps its round-1 shape (no realization needed there yet — the section
// editor only renders verse + chorus). Verse and chorus add full
// realization blocks and variant schedules.

fn build_sections() -> Vec<Section> {
    vec![intro_section(), verse_section(), chorus_section()]
}

fn intro_section() -> Section {
    Section {
        id: "s_intro".into(),
        name: "intro".into(),
        color: theme::PAL_ROSE.into(),
        variants: vec![Variant { id: "base".into(), name: "base".into() }],
        default_variant: "base".into(),
        base_duration_bars: 4,
        chord_loops: vec!["verse-progression".into()],
        activations: vec![
            ("t_bass".into(),  silent_no_realization("bass-main")),
            ("t_lead".into(),  silent_no_realization("lead-main")),
            ("t_drums".into(), silent_no_realization("drums-main")),
            ("t_pad".into(),   active_no_realization("pad-bed")),
        ],
        variant_overrides: Vec::new(),
    }
}

fn verse_section() -> Section {
    // Verse / base. NOTE: pad is deliberately absent. Decision 14 in the
    // round-2 README says tracks without an entry in `activations` render
    // as a dashed-border inherit placeholder — that path is exercised here.
    let base_activations = vec![
        (
            "t_bass".into(),
            active_pitched(
                "bass-main",
                Voicing::Power,
                OctaveSpec::Nearest,
                Humanization { velocity: 0.04, timing: 4, swing: 0.0, seed: 1742 },
                0,
            ),
        ),
        (
            "t_lead".into(),
            active_pitched(
                "lead-main",
                Voicing::TriadClose,
                OctaveSpec::Anchored(4), // pinned, overrides role default `Nearest`
                Humanization { velocity: 0.06, timing: 5, swing: 0.0, seed: 913 },
                2,
            ),
        ),
        (
            "t_drums".into(),
            active_drum(
                "drums-main",
                Humanization { velocity: 0.10, timing: 7, swing: 0.05, seed: 8821 },
                Vec::new(),
            ),
        ),
    ];

    // Verse / stripped overrides. Bass + drums fully silenced; lead replaced
    // with an activation whose schedule drops out for bar 4 (a
    // `(BarRange, None)` sub-range silence — distinct from a full `Silent`,
    // per round-2 decision 20).
    let stripped_lead = Activation {
        pattern: "lead-main".into(),
        state: ActivationState::Active,
        overridden: true,
        realization: Some(Realization {
            voicing: Some(Voicing::TriadClose),
            octave: Some(OctaveSpec::Anchored(4)),
            humanization: Humanization { velocity: 0.05, timing: 4, swing: 0.0, seed: 913 },
        }),
        variant_schedule: vec![ScheduleEntry { start_bar: 3, end_bar: 4, variant: None }],
        per_note_overrides: 2,
    };

    let stripped: VariantOverride = vec![
        ("t_bass".into(),  ActivationOverride::Silent),
        ("t_drums".into(), ActivationOverride::Silent),
        ("t_lead".into(),  ActivationOverride::Replace(stripped_lead)),
    ];

    Section {
        id: "s_verse".into(),
        name: "verse".into(),
        color: theme::PAL_BLUE.into(),
        variants: vec![
            Variant { id: "base".into(),     name: "base".into()     },
            Variant { id: "stripped".into(), name: "stripped".into() },
        ],
        default_variant: "base".into(),
        base_duration_bars: 4,
        chord_loops: vec!["verse-progression".into()],
        activations: base_activations,
        variant_overrides: vec![("stripped".into(), stripped)],
    }
}

fn chorus_section() -> Section {
    // Chorus / base. Drums carry a sparse variant schedule: only the `fill`
    // entry at bar 8 is stored. Bars 1–7 play the pattern's default variant
    // (`main`), computed at render time — never persisted as a phantom entry.
    let activations = vec![
        (
            "t_bass".into(),
            active_pitched(
                "bass-main",
                Voicing::Power,
                OctaveSpec::Nearest,
                Humanization { velocity: 0.04, timing: 4, swing: 0.0, seed: 1742 },
                0,
            ),
        ),
        (
            "t_lead".into(),
            active_pitched(
                "lead-main",
                Voicing::TriadClose,
                OctaveSpec::UpFromPrev, // hook leap, overrides role default
                Humanization { velocity: 0.07, timing: 5, swing: 0.0, seed: 913 },
                3,
            ),
        ),
        (
            "t_drums".into(),
            active_drum(
                "drums-main",
                Humanization { velocity: 0.12, timing: 8, swing: 0.05, seed: 8821 },
                vec![ScheduleEntry {
                    start_bar: 7,
                    end_bar: 8,
                    variant: Some("fill".into()),
                }],
            ),
        ),
        (
            "t_pad".into(),
            // drop2 overrides role:pad's default triad-open; Nearest overrides
            // role:pad's default Anchored(3).
            active_pitched(
                "pad-bed",
                Voicing::Drop2,
                OctaveSpec::Nearest,
                Humanization { velocity: 0.02, timing: 2, swing: 0.0, seed: 3104 },
                0,
            ),
        ),
    ];

    Section {
        id: "s_chorus".into(),
        name: "chorus".into(),
        color: theme::PAL_SAND.into(),
        variants: vec![Variant { id: "base".into(), name: "base".into() }],
        default_variant: "base".into(),
        base_duration_bars: 8,
        chord_loops: vec!["chorus-progression".into()],
        activations,
        variant_overrides: Vec::new(),
    }
}

// ─── Arrangement ──────────────────────────────────────────────────────────

fn build_arrangement() -> Vec<SectionRef> {
    vec![
        SectionRef { idx: 0, section_key: "intro".into(),  variant: "base".into(),     start_bar: 0,  bars: 4 },
        SectionRef { idx: 1, section_key: "verse".into(),  variant: "base".into(),     start_bar: 4,  bars: 4 },
        SectionRef { idx: 2, section_key: "verse".into(),  variant: "stripped".into(), start_bar: 8,  bars: 4 },
        SectionRef { idx: 3, section_key: "verse".into(),  variant: "base".into(),     start_bar: 12, bars: 4 },
        SectionRef { idx: 4, section_key: "chorus".into(), variant: "base".into(),     start_bar: 16, bars: 8 },
    ]
}

// ─── Tests ────────────────────────────────────────────────────────────────
//
// Round-2 fixture invariants. These pin the data shape the section
// editor port relies on — especially the "no phantom default entry"
// rule from the round-2 README.

#[cfg(test)]
mod tests {
    use crate::fixture::{
        self, base_activation, role_defaults, round1, variant_override, ActivationOverride,
        OctaveSpec, Section, Voicing,
    };

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
        // Per round-2 decision 20 + the fixture-fix pass: the only
        // explicit entry on stripped lead is the sub-range silence at
        // bar 4. Bars 1–3 are an implicit-default fill computed at
        // render time — there must be NO phantom "main" entry stored.
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
        // Spot-check the role table against round-2's `data.js` so the
        // inheritance comparison in the section editor matches the
        // mockup. Drums have no role → no defaults.
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
        // Every Pattern must declare its default_variant — the schedule
        // builder uses it to compute implicit-default fills, and a
        // missing value would silently mis-render.
        for p in fixture::round1().patterns.iter() {
            assert!(
                !p.default_variant.is_empty(),
                "pattern {} is missing default_variant",
                p.name
            );
        }
    }
}
