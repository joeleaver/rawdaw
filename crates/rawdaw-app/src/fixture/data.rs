//! Static fixture tables + round-2 invariant tests.
//!
//! Round-1 mockup data, mirroring
//! `docs/design/mockups/round-1/components/data.js`, extended with
//! the round-2 additions from
//! `docs/design/mockups/round-2/components/data.js`. The split between
//! this file and `mod.rs` is purely size — types and helpers stay in
//! `mod.rs`; the bulky `static` tables live here so neither file
//! approaches the ~700-line cap.

use super::{
    Activation, ActivationOverride, ActivationState, ChordEvent, ChordLoop, Humanization,
    OctaveSpec, Pattern, Project, Realization, Round1, ScheduleEntry, Section, SectionRef, Track,
    TrackKind, Variant, VariantOverride, Voicing,
};
use crate::theme;

// ─── Tracks / patterns / chord events / chord loops ───────────────────────

static TRACKS: &[Track] = &[
    Track { id: "t_bass",  name: "bass",  kind: TrackKind::Pitched, role: "bass"    },
    Track { id: "t_lead",  name: "lead",  kind: TrackKind::Pitched, role: "melodic" },
    Track { id: "t_drums", name: "drums", kind: TrackKind::Drum,    role: "—"       },
    Track { id: "t_pad",   name: "pad",   kind: TrackKind::Pitched, role: "pad"     },
];

static PATTERNS: &[Pattern] = &[
    Pattern { id: "p_bass",  name: "bass-main",  color: theme::PAL_TEAL,
              kind: "Pitched", variants: 2, default_variant: "main",
              meta: "Pitched · 2 variants" },
    Pattern { id: "p_lead",  name: "lead-main",  color: theme::PAL_PLUM,
              kind: "Pitched", variants: 1, default_variant: "main",
              meta: "Pitched · 1 variant"  },
    Pattern { id: "p_drums", name: "drums-main", color: theme::PAL_SAGE,
              kind: "Drum",    variants: 2, default_variant: "main",
              meta: "Drum · 2 variants"    },
    Pattern { id: "p_pad",   name: "pad-bed",    color: theme::PAL_SLATE,
              kind: "Pitched", variants: 1, default_variant: "main",
              meta: "Pitched · 1 variant"  },
];

static VERSE_PROGRESSION_EVENTS: &[ChordEvent] = &[
    ChordEvent { roman: "I",  quality: "", absolute: "C"  },
    ChordEvent { roman: "V",  quality: "", absolute: "G"  },
    ChordEvent { roman: "vi", quality: "", absolute: "Am" },
    ChordEvent { roman: "IV", quality: "", absolute: "F"  },
];
static CHORUS_PROGRESSION_EVENTS: &[ChordEvent] = &[
    ChordEvent { roman: "vi", quality: "", absolute: "Am" },
    ChordEvent { roman: "IV", quality: "", absolute: "F"  },
    ChordEvent { roman: "I",  quality: "", absolute: "C"  },
    ChordEvent { roman: "V",  quality: "", absolute: "G"  },
];

static CHORD_LOOPS: &[ChordLoop] = &[
    ChordLoop { id: "cl_verse",  name: "verse-progression",
                color: theme::PAL_TERRA, length_bars: 4,
                events: VERSE_PROGRESSION_EVENTS },
    ChordLoop { id: "cl_chorus", name: "chorus-progression",
                color: theme::PAL_OLIVE, length_bars: 4,
                events: CHORUS_PROGRESSION_EVENTS },
];

// ─── Section activations ──────────────────────────────────────────────────
//
// Round-2 mockup data, mirroring
// `docs/design/mockups/round-2/components/data.js`. The intro section
// keeps its round-1 shape (no realization needed there yet — the section
// editor only renders verse + chorus). Verse and chorus add full
// realization blocks and variant schedules.

static NO_SCHEDULE: &[ScheduleEntry] = &[];

// Implicit-default schedule entry helpers are NOT defined; the scheduler
// computes default-variant fills at render time. Only non-default and
// silenced sub-ranges are stored.

static INTRO_ACTIVATIONS: &[(&str, Activation)] = &[
    ("t_bass",  Activation {
        pattern: "bass-main",  state: ActivationState::Silent, overridden: false,
        realization: None, variant_schedule: NO_SCHEDULE, per_note_overrides: 0,
    }),
    ("t_lead",  Activation {
        pattern: "lead-main",  state: ActivationState::Silent, overridden: false,
        realization: None, variant_schedule: NO_SCHEDULE, per_note_overrides: 0,
    }),
    ("t_drums", Activation {
        pattern: "drums-main", state: ActivationState::Silent, overridden: false,
        realization: None, variant_schedule: NO_SCHEDULE, per_note_overrides: 0,
    }),
    ("t_pad",   Activation {
        pattern: "pad-bed",    state: ActivationState::Active, overridden: false,
        realization: None, variant_schedule: NO_SCHEDULE, per_note_overrides: 0,
    }),
];

// Verse / base. NOTE: pad is deliberately absent. Decision 14 in the
// round-2 README says tracks without an entry in `activations` render
// as a dashed-border inherit placeholder — that path is exercised here.
static VERSE_ACTIVATIONS: &[(&str, Activation)] = &[
    ("t_bass",  Activation {
        pattern: "bass-main", state: ActivationState::Active, overridden: false,
        realization: Some(Realization {
            voicing: Some(Voicing::Power),
            octave: Some(OctaveSpec::Nearest),
            humanization: Humanization { velocity: 0.04, timing: 4, swing: 0.0, seed: 1742 },
        }),
        variant_schedule: NO_SCHEDULE,
        per_note_overrides: 0,
    }),
    ("t_lead",  Activation {
        pattern: "lead-main", state: ActivationState::Active, overridden: false,
        realization: Some(Realization {
            voicing: Some(Voicing::TriadClose),
            octave: Some(OctaveSpec::Anchored(4)),  // pinned, overrides role default `Nearest`
            humanization: Humanization { velocity: 0.06, timing: 5, swing: 0.0, seed: 913 },
        }),
        variant_schedule: NO_SCHEDULE,
        per_note_overrides: 2,
    }),
    ("t_drums", Activation {
        pattern: "drums-main", state: ActivationState::Active, overridden: false,
        realization: Some(Realization {
            voicing: None, octave: None,  // drums are pitch-symbolic
            humanization: Humanization { velocity: 0.10, timing: 7, swing: 0.05, seed: 8821 },
        }),
        variant_schedule: NO_SCHEDULE,
        per_note_overrides: 0,
    }),
];

// Verse / stripped overrides. Bass + drums fully silenced; lead replaced
// with an activation whose schedule drops out for bar 4
// (a `(BarRange, None)` sub-range silence — distinct from a full
// `Silent`, per round-2 decision 20).
static STRIPPED_LEAD_SCHEDULE: &[ScheduleEntry] = &[
    ScheduleEntry { start_bar: 3, end_bar: 4, variant: None },
];

static STRIPPED_LEAD_ACTIVATION: Activation = Activation {
    pattern: "lead-main",
    state: ActivationState::Active,
    overridden: true,
    realization: Some(Realization {
        voicing: Some(Voicing::TriadClose),
        octave: Some(OctaveSpec::Anchored(4)),
        humanization: Humanization { velocity: 0.05, timing: 4, swing: 0.0, seed: 913 },
    }),
    variant_schedule: STRIPPED_LEAD_SCHEDULE,
    per_note_overrides: 2,
};

static VERSE_STRIPPED_OVERRIDE: &[(&str, ActivationOverride)] = &[
    ("t_bass",  ActivationOverride::Silent),
    ("t_drums", ActivationOverride::Silent),
    ("t_lead",  ActivationOverride::Replace(STRIPPED_LEAD_ACTIVATION)),
];

static VERSE_VARIANT_OVERRIDES: &[(&str, VariantOverride)] = &[
    ("stripped", VERSE_STRIPPED_OVERRIDE),
];

// Chorus / base. Drums carry a sparse variant schedule: only the `fill`
// entry at bar 8 is stored. Bars 1–7 play the pattern's default variant
// (`main`), computed at render time — never persisted as a phantom entry.
static CHORUS_DRUMS_SCHEDULE: &[ScheduleEntry] = &[
    ScheduleEntry { start_bar: 7, end_bar: 8, variant: Some("fill") },
];

static CHORUS_ACTIVATIONS: &[(&str, Activation)] = &[
    ("t_bass",  Activation {
        pattern: "bass-main", state: ActivationState::Active, overridden: false,
        realization: Some(Realization {
            voicing: Some(Voicing::Power),
            octave: Some(OctaveSpec::Nearest),
            humanization: Humanization { velocity: 0.04, timing: 4, swing: 0.0, seed: 1742 },
        }),
        variant_schedule: NO_SCHEDULE,
        per_note_overrides: 0,
    }),
    ("t_lead",  Activation {
        pattern: "lead-main", state: ActivationState::Active, overridden: false,
        realization: Some(Realization {
            voicing: Some(Voicing::TriadClose),
            octave: Some(OctaveSpec::UpFromPrev),  // hook leap, overrides role default
            humanization: Humanization { velocity: 0.07, timing: 5, swing: 0.0, seed: 913 },
        }),
        variant_schedule: NO_SCHEDULE,
        per_note_overrides: 3,
    }),
    ("t_drums", Activation {
        pattern: "drums-main", state: ActivationState::Active, overridden: false,
        realization: Some(Realization {
            voicing: None, octave: None,
            humanization: Humanization { velocity: 0.12, timing: 8, swing: 0.05, seed: 8821 },
        }),
        variant_schedule: CHORUS_DRUMS_SCHEDULE,
        per_note_overrides: 0,
    }),
    ("t_pad",   Activation {
        pattern: "pad-bed",    state: ActivationState::Active, overridden: false,
        realization: Some(Realization {
            // drop2 overrides role:pad's default triad-open
            voicing: Some(Voicing::Drop2),
            // Nearest overrides role:pad's default Anchored(3)
            octave: Some(OctaveSpec::Nearest),
            humanization: Humanization { velocity: 0.02, timing: 2, swing: 0.0, seed: 3104 },
        }),
        variant_schedule: NO_SCHEDULE,
        per_note_overrides: 0,
    }),
];

// ─── Sections ─────────────────────────────────────────────────────────────

static INTRO_VARIANTS: &[Variant] = &[Variant { id: "base", name: "base" }];
static VERSE_VARIANTS: &[Variant] = &[
    Variant { id: "base",     name: "base"     },
    Variant { id: "stripped", name: "stripped" },
];
static CHORUS_VARIANTS: &[Variant] = &[Variant { id: "base", name: "base" }];

static EMPTY_OVERRIDES: &[(&str, VariantOverride)] = &[];

static SECTIONS: &[Section] = &[
    Section {
        id: "s_intro", name: "intro", color: theme::PAL_ROSE,
        variants: INTRO_VARIANTS, default_variant: "base",
        base_duration_bars: 4,
        chord_loops: &["verse-progression"],
        activations: INTRO_ACTIVATIONS,
        variant_overrides: EMPTY_OVERRIDES,
    },
    Section {
        id: "s_verse", name: "verse", color: theme::PAL_BLUE,
        variants: VERSE_VARIANTS, default_variant: "base",
        base_duration_bars: 4,
        chord_loops: &["verse-progression"],
        activations: VERSE_ACTIVATIONS,
        variant_overrides: VERSE_VARIANT_OVERRIDES,
    },
    Section {
        id: "s_chorus", name: "chorus", color: theme::PAL_SAND,
        variants: CHORUS_VARIANTS, default_variant: "base",
        base_duration_bars: 8,
        chord_loops: &["chorus-progression"],
        activations: CHORUS_ACTIVATIONS,
        variant_overrides: EMPTY_OVERRIDES,
    },
];

static ARRANGEMENT: &[SectionRef] = &[
    SectionRef { idx: 0, section_key: "intro",  variant: "base",     start_bar: 0,  bars: 4 },
    SectionRef { idx: 1, section_key: "verse",  variant: "base",     start_bar: 4,  bars: 4 },
    SectionRef { idx: 2, section_key: "verse",  variant: "stripped", start_bar: 8,  bars: 4 },
    SectionRef { idx: 3, section_key: "verse",  variant: "base",     start_bar: 12, bars: 4 },
    SectionRef { idx: 4, section_key: "chorus", variant: "base",     start_bar: 16, bars: 8 },
];

pub fn round1() -> Round1 {
    Round1 {
        project: Project {
            name: "untitled-1",
            key: "C major",
            time_sig: "4/4",
            tempo: 96,
            playhead_bar: 5,
            playhead_beat: 2,
        },
        tracks: TRACKS,
        patterns: PATTERNS,
        chord_loops: CHORD_LOOPS,
        sections: SECTIONS,
        arrangement: ARRANGEMENT,
        total_bars: 24,
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────
//
// Round-2 fixture invariants. These pin the data shape the section
// editor port relies on — especially the "no phantom default entry"
// rule from the round-2 README.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture::{
        base_activation, role_defaults, variant_override, OctaveSpec, Voicing,
    };

    fn section(name: &str) -> &'static Section {
        SECTIONS
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
        let e = act.variant_schedule[0];
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
        let e = drums.variant_schedule[0];
        assert_eq!(e.start_bar, 7);
        assert_eq!(e.end_bar, 8);
        assert_eq!(e.variant, Some("fill"));
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
        for p in PATTERNS.iter() {
            assert!(
                !p.default_variant.is_empty(),
                "pattern {} is missing default_variant",
                p.name
            );
        }
    }
}
