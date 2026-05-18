//! Round-1 fixture: the canonical project that drives `rawdaw-app`'s
//! initial UI and the engine-wiring milestone's playback.
//!
//! Mirrors the data shape of `rawdaw-app::fixture::round1()` exactly:
//! four tracks (bass / lead / drums / pad), four patterns
//! (bass-main, lead-main, drums-main with `main` + `fill` variants,
//! pad-bed), two chord loops (verse-progression `I V vi IV`,
//! chorus-progression `vi IV I V`), three sections (intro 4 bars,
//! verse 4 bars + `stripped` variant, chorus 8 bars), and a five-step
//! arrangement totaling 24 bars (intro · verse · verse@stripped ·
//! verse · chorus).
//!
//! Pattern bodies carry one-beat illustrative content per variant —
//! enough for the realization pass to emit real `TimedEvent`s without
//! introducing musical decisions that should belong to a real
//! composition. Phase E3 of the engine-wiring milestone consumes
//! these events; the UI consumes the structural shell.
//!
//! ## Stability
//!
//! Tests at the bottom of this file pin the shape against the round-1
//! invariants the UI port relies on (`pad`-absent-from-verse,
//! sub-range silences, the chorus drum `fill` entry, etc). Changing
//! the fixture without updating the tests will break either the UI
//! adapter or the engine event-count baseline.

use std::collections::BTreeMap;

use crate::activation::{ActivationEntry, RealizationParams};
use crate::chord::{
    ChordDegree, ChordEvent, ChordLoop, ChordQuality, ChordSpec, ChordStep, ChordSuffix,
    RomanDegree,
};
use crate::id::{
    ActivationEntryId, ChordLoopId, DrumKitId, InstrumentId, PatternId, SectionId, TrackId,
    VariantId,
};
use crate::pattern::{
    DrumEvent, DrumPatternBody, DrumPatternMetadata, DrumVoice, EventHumanization, OctaveSpec,
    Pattern, PatternBody, PitchSpec, PitchedEvent, PitchedPatternBody, PitchedPatternMetadata,
};
use crate::pitch::{Octave, PitchClass, U7};
use crate::project::{DrumKitStub, Project};
use crate::scale::{Scale, ScaleDegree};
use crate::section::{
    ActivationOverride, Arrangement, Section, SectionBody, SectionRef, SectionVariantOverride,
};
use crate::time::{BarRange, Duration, MusicalTime, PPQ};
use crate::track::{MixerPlacement, Role, Track, TrackKind};

/// Track id bundle the round-1 fixture exposes for callers that need
/// to route by id (e.g. `TrackRouting` in the engine).
#[derive(Debug, Clone, Copy)]
pub struct Round1TrackIds {
    pub bass: TrackId,
    pub lead: TrackId,
    pub drums: TrackId,
    pub pad: TrackId,
}

/// Pattern id bundle, exposed for the same reason as `Round1TrackIds`.
#[derive(Debug, Clone, Copy)]
pub struct Round1PatternIds {
    pub bass: PatternId,
    pub lead: PatternId,
    pub drums: PatternId,
    pub pad: PatternId,
}

/// Section id bundle.
#[derive(Debug, Clone, Copy)]
pub struct Round1SectionIds {
    pub intro: SectionId,
    pub verse: SectionId,
    pub chorus: SectionId,
}

/// Full id-key for the round-1 fixture. The constructor returns the
/// `Project` plus this so callers don't have to scan name lookups to
/// recover the ids of the entities they care about.
#[derive(Debug, Clone, Copy)]
pub struct Round1Keys {
    pub tracks: Round1TrackIds,
    pub patterns: Round1PatternIds,
    pub sections: Round1SectionIds,
    pub chord_loops: Round1ChordLoopIds,
    pub drum_kit: DrumKitId,
}

#[derive(Debug, Clone, Copy)]
pub struct Round1ChordLoopIds {
    pub verse: ChordLoopId,
    pub chorus: ChordLoopId,
}

/// Build the round-1 project. Returns the `Project` plus a `Round1Keys`
/// bundle exposing every named id so callers can look up entities
/// without re-scanning.
pub fn build_round1_project() -> (Project, Round1Keys) {
    let mut project = Project::new(Scale::major(PitchClass::C));
    let alloc = &mut project.id_allocators;

    let chord_loops = insert_chord_loops(alloc, &mut project.chord_loops);
    let patterns = insert_patterns(alloc, &mut project.patterns);

    let drum_kit = DrumKitId::new(0);
    project.drum_kits.insert(
        drum_kit,
        DrumKitStub {
            id: drum_kit,
            name: "default-kit".into(),
        },
    );

    let tracks = insert_tracks(alloc, &mut project.tracks, drum_kit);

    let sections = insert_sections(alloc, &mut project.sections, &chord_loops, &patterns, &tracks);

    project.arrangement = build_arrangement(alloc, &sections);

    let keys = Round1Keys {
        tracks,
        patterns,
        sections,
        chord_loops,
        drum_kit,
    };
    (project, keys)
}

// ─── Chord loops ──────────────────────────────────────────────────────────

fn insert_chord_loops(
    alloc: &mut crate::project::IdAllocators,
    out: &mut BTreeMap<ChordLoopId, ChordLoop>,
) -> Round1ChordLoopIds {
    let verse = alloc.alloc_chord_loop();
    out.insert(
        verse,
        ChordLoop {
            id: verse,
            name: "verse-progression".into(),
            length: Duration::bars(4, 4),
            key: None,
            events: progression(&[
                (RomanDegree::I, ChordQuality::Major),
                (RomanDegree::V, ChordQuality::Major),
                (RomanDegree::VI, ChordQuality::Minor),
                (RomanDegree::IV, ChordQuality::Major),
            ]),
        },
    );
    let chorus = alloc.alloc_chord_loop();
    out.insert(
        chorus,
        ChordLoop {
            id: chorus,
            name: "chorus-progression".into(),
            length: Duration::bars(4, 4),
            key: None,
            events: progression(&[
                (RomanDegree::VI, ChordQuality::Minor),
                (RomanDegree::IV, ChordQuality::Major),
                (RomanDegree::I, ChordQuality::Major),
                (RomanDegree::V, ChordQuality::Major),
            ]),
        },
    );
    Round1ChordLoopIds { verse, chorus }
}

fn progression(degrees: &[(RomanDegree, ChordQuality)]) -> Vec<ChordEvent> {
    degrees
        .iter()
        .enumerate()
        .map(|(i, (roman, quality))| ChordEvent {
            time: MusicalTime::beats(i as i64 * 4),
            duration: Duration::beats(4),
            chord: ChordSpec::Functional {
                roman: *roman,
                suffix: ChordSuffix::new(quality.clone()),
                in_key: None,
            },
            bass: None,
            annotation: None,
        })
        .collect()
}

// ─── Patterns ─────────────────────────────────────────────────────────────

fn insert_patterns(
    alloc: &mut crate::project::IdAllocators,
    out: &mut BTreeMap<PatternId, Pattern>,
) -> Round1PatternIds {
    // Body construction allocates note ids, so each `*_body(alloc)`
    // call binds to a local before the `insert_pattern` call — using
    // `alloc` in two args of the same call is two simultaneous `&mut`
    // borrows.
    let body = bass_body(alloc);
    let bass = insert_pattern(alloc, out, "bass-main", body);
    let body = lead_body(alloc);
    let lead = insert_pattern(alloc, out, "lead-main", body);
    let body = drums_body(alloc);
    let drums = insert_pattern(alloc, out, "drums-main", body);
    let body = pad_body(alloc);
    let pad = insert_pattern(alloc, out, "pad-bed", body);
    Round1PatternIds {
        bass,
        lead,
        drums,
        pad,
    }
}

fn insert_pattern(
    alloc: &mut crate::project::IdAllocators,
    out: &mut BTreeMap<PatternId, Pattern>,
    name: &str,
    body: PatternBody,
) -> PatternId {
    let id = alloc.alloc_pattern();
    out.insert(
        id,
        Pattern {
            id,
            name: name.into(),
            default_variant: VariantId::main(),
            body,
        },
    );
    id
}

/// Bass-main: root + fifth on beats 1 and 3. One illustrative event per
/// variant is enough for the realization pass to produce events that
/// downstream tests can count. The fixture deliberately avoids picking
/// "real" composition choices — these are plumbing notes, not music.
fn bass_body(alloc: &mut crate::project::IdAllocators) -> PatternBody {
    let mut variants = BTreeMap::new();
    variants.insert(VariantId::main(), bass_variant(alloc));
    // Pattern carries 2 variants per the UI fixture's `variants: 2` —
    // the round-2 schedule UI doesn't exercise a non-main bass variant,
    // but the count is observable in the library.
    variants.insert("alt".into(), bass_variant(alloc));
    PatternBody::Pitched(PitchedPatternBody {
        metadata: PitchedPatternMetadata {
            length: Duration::bars(1, 4),
        },
        variants,
    })
}

fn bass_variant(alloc: &mut crate::project::IdAllocators) -> Vec<PitchedEvent> {
    vec![
        pitched(alloc, 0, 1, 100, PitchSpec::Chord {
            degree: ChordDegree::new(ChordStep::Root),
            octave: OctaveSpec::Nearest,
        }),
        pitched(alloc, 2, 1, 84, PitchSpec::Chord {
            degree: ChordDegree::new(ChordStep::Fifth),
            octave: OctaveSpec::Nearest,
        }),
    ]
}

/// Lead-main: a one-bar melodic figure — scale degree 3 → 1 → 5.
fn lead_body(alloc: &mut crate::project::IdAllocators) -> PatternBody {
    let mut variants = BTreeMap::new();
    let main_events = vec![
        pitched(alloc, 0, 1, 96, PitchSpec::Scale {
            degree: ScaleDegree::new(3),
            octave: OctaveSpec::Anchored(Octave(5)),
        }),
        pitched(alloc, 1, 1, 88, PitchSpec::Scale {
            degree: ScaleDegree::new(1),
            octave: OctaveSpec::Nearest,
        }),
        pitched(alloc, 2, 2, 100, PitchSpec::Scale {
            degree: ScaleDegree::new(5),
            octave: OctaveSpec::Nearest,
        }),
    ];
    variants.insert(VariantId::main(), main_events);
    PatternBody::Pitched(PitchedPatternBody {
        metadata: PitchedPatternMetadata {
            length: Duration::bars(1, 4),
        },
        variants,
    })
}

/// Drums-main: kick on 1+3, snare on 2+4, plus a `fill` variant with
/// a snare flourish on beat 4. Mirrors the round-2 fixture invariant
/// that drums carry both `main` and `fill` variants.
fn drums_body(alloc: &mut crate::project::IdAllocators) -> PatternBody {
    let mut variants = BTreeMap::new();
    let main = (0..4i64)
        .map(|beat| {
            let voice = if beat % 2 == 0 {
                DrumVoice::Kick
            } else {
                DrumVoice::Snare
            };
            drum_event(alloc, beat, 1, voice, 100)
        })
        .collect();
    variants.insert(VariantId::main(), main);
    let fill = vec![
        drum_event(alloc, 3, 1, DrumVoice::Snare, 110),
        drum_event(alloc, 3, 1, DrumVoice::Snare, 115),
    ];
    variants.insert("fill".into(), fill);
    PatternBody::Drum(DrumPatternBody {
        metadata: DrumPatternMetadata {
            length: Duration::bars(1, 4),
            voices: vec![DrumVoice::Kick, DrumVoice::Snare, DrumVoice::ClosedHat],
        },
        variants,
    })
}

/// Pad-bed: a sustained root chord across the whole bar.
fn pad_body(alloc: &mut crate::project::IdAllocators) -> PatternBody {
    let mut variants = BTreeMap::new();
    variants.insert(
        VariantId::main(),
        vec![pitched(alloc, 0, 16, 64, PitchSpec::Chord {
            degree: ChordDegree::new(ChordStep::Root),
            octave: OctaveSpec::Anchored(Octave(3)),
        })],
    );
    PatternBody::Pitched(PitchedPatternBody {
        metadata: PitchedPatternMetadata {
            length: Duration::bars(1, 4),
        },
        variants,
    })
}

fn pitched(
    alloc: &mut crate::project::IdAllocators,
    beat: i64,
    sixteenths: i64,
    velocity: u8,
    spec: PitchSpec,
) -> PitchedEvent {
    let _ = alloc; // alloc.alloc_note() below
    PitchedEvent {
        note_id: alloc.alloc_note(),
        time: MusicalTime::beats(beat),
        duration: Duration::ticks(sixteenths * (PPQ / 4)),
        velocity: U7::clamp(velocity),
        articulation: None,
        humanization: EventHumanization::default(),
        spec,
    }
}

fn drum_event(
    alloc: &mut crate::project::IdAllocators,
    beat: i64,
    _sixteenths: i64,
    voice: DrumVoice,
    velocity: u8,
) -> DrumEvent {
    DrumEvent {
        note_id: alloc.alloc_note(),
        time: MusicalTime::beats(beat),
        duration: Duration::beats(1),
        voice,
        velocity: U7::clamp(velocity),
        articulation: None,
        humanization: EventHumanization::default(),
    }
}

// ─── Tracks ───────────────────────────────────────────────────────────────

fn insert_tracks(
    alloc: &mut crate::project::IdAllocators,
    out: &mut Vec<Track>,
    drum_kit: DrumKitId,
) -> Round1TrackIds {
    let bass = push_track(alloc, out, "bass", TrackKind::Pitched { role: Role::Bass });
    let lead = push_track(
        alloc,
        out,
        "lead",
        TrackKind::Pitched {
            role: Role::Melodic,
        },
    );
    let drums = push_track(alloc, out, "drums", TrackKind::Drum { kit: drum_kit });
    let pad = push_track(alloc, out, "pad", TrackKind::Pitched { role: Role::Pad });
    Round1TrackIds {
        bass,
        lead,
        drums,
        pad,
    }
}

fn push_track(
    alloc: &mut crate::project::IdAllocators,
    out: &mut Vec<Track>,
    name: &str,
    kind: TrackKind,
) -> TrackId {
    let id = alloc.alloc_track();
    let display_index = out.len() as u32;
    let instrument = InstrumentId::new(display_index as u64);
    out.push(Track {
        id,
        name: name.into(),
        kind,
        instrument,
        mixer: MixerPlacement { display_index },
    });
    id
}

// ─── Sections ─────────────────────────────────────────────────────────────

fn insert_sections(
    alloc: &mut crate::project::IdAllocators,
    out: &mut BTreeMap<SectionId, Section>,
    cl: &Round1ChordLoopIds,
    pat: &Round1PatternIds,
    tr: &Round1TrackIds,
) -> Round1SectionIds {
    let intro = alloc.alloc_section();
    out.insert(intro, build_intro(alloc, intro, cl, pat, tr));

    let verse = alloc.alloc_section();
    out.insert(verse, build_verse(alloc, verse, cl, pat, tr));

    let chorus = alloc.alloc_section();
    out.insert(chorus, build_chorus(alloc, chorus, cl, pat, tr));

    Round1SectionIds {
        intro,
        verse,
        chorus,
    }
}

/// Intro: pad-only; bass / lead / drums are silent. Mirrors the round-1
/// fixture's intro shape.
fn build_intro(
    alloc: &mut crate::project::IdAllocators,
    id: SectionId,
    cl: &Round1ChordLoopIds,
    pat: &Round1PatternIds,
    tr: &Round1TrackIds,
) -> Section {
    let mut activations = BTreeMap::new();
    activations.insert(tr.pad, simple_activation(alloc, pat.pad));
    Section {
        id,
        name: "intro".into(),
        base: SectionBody {
            duration_bars: 4,
            scale_override: None,
            chord_loops: vec![(BarRange::new(0, 4), cl.verse)],
            activations,
        },
        variants: BTreeMap::new(),
        default_variant: VariantId::base(),
    }
}

/// Verse: bass / lead / drums active; pad deliberately absent (the
/// dashed-border inherit placeholder in the section editor).
/// `stripped` variant silences bass + drums and replaces lead with a
/// `(3..4, None)` silent sub-range.
fn build_verse(
    alloc: &mut crate::project::IdAllocators,
    id: SectionId,
    cl: &Round1ChordLoopIds,
    pat: &Round1PatternIds,
    tr: &Round1TrackIds,
) -> Section {
    let mut activations = BTreeMap::new();
    activations.insert(tr.bass, simple_activation(alloc, pat.bass));
    activations.insert(tr.lead, simple_activation(alloc, pat.lead));
    activations.insert(tr.drums, simple_activation(alloc, pat.drums));

    let mut stripped_acts = BTreeMap::new();
    stripped_acts.insert(tr.bass, ActivationOverride::Silent);
    stripped_acts.insert(tr.drums, ActivationOverride::Silent);
    stripped_acts.insert(
        tr.lead,
        ActivationOverride::Replace(ActivationEntry {
            id: ActivationEntryId::new(alloc_activation_entry(alloc)),
            pattern_ref: Some(pat.lead),
            // Sparse: one entry for the silenced bar 4. Bars 1-3 are an
            // implicit-default fill computed at render time. No phantom
            // `main` entry, per round-2 README decision 24.
            variant_schedule: vec![(BarRange::new(3, 4), VariantId::new("__silent__"))],
            realization: RealizationParams::default(),
            per_note_overrides: Vec::new(),
        }),
    );

    let mut variants = BTreeMap::new();
    variants.insert(
        VariantId::new("stripped"),
        SectionVariantOverride {
            activations: stripped_acts,
            ..Default::default()
        },
    );

    Section {
        id,
        name: "verse".into(),
        base: SectionBody {
            duration_bars: 4,
            scale_override: None,
            chord_loops: vec![(BarRange::new(0, 4), cl.verse)],
            activations,
        },
        variants,
        default_variant: VariantId::base(),
    }
}

/// Chorus: all four tracks active. Drums carry a sparse variant schedule
/// — one `fill` entry at bar 8 (bars 1–7 are implicit default).
fn build_chorus(
    alloc: &mut crate::project::IdAllocators,
    id: SectionId,
    cl: &Round1ChordLoopIds,
    pat: &Round1PatternIds,
    tr: &Round1TrackIds,
) -> Section {
    let mut activations = BTreeMap::new();
    activations.insert(tr.bass, simple_activation(alloc, pat.bass));
    activations.insert(tr.lead, simple_activation(alloc, pat.lead));
    activations.insert(tr.pad, simple_activation(alloc, pat.pad));
    activations.insert(
        tr.drums,
        ActivationEntry {
            id: ActivationEntryId::new(alloc_activation_entry(alloc)),
            pattern_ref: Some(pat.drums),
            // Sparse: `fill` at bar 8 only.
            variant_schedule: vec![(BarRange::new(7, 8), VariantId::new("fill"))],
            realization: RealizationParams::default(),
            per_note_overrides: Vec::new(),
        },
    );

    Section {
        id,
        name: "chorus".into(),
        base: SectionBody {
            duration_bars: 8,
            scale_override: None,
            chord_loops: vec![(BarRange::new(0, 8), cl.chorus)],
            activations,
        },
        variants: BTreeMap::new(),
        default_variant: VariantId::base(),
    }
}

fn simple_activation(
    alloc: &mut crate::project::IdAllocators,
    pattern: PatternId,
) -> ActivationEntry {
    ActivationEntry {
        id: ActivationEntryId::new(alloc_activation_entry(alloc)),
        pattern_ref: Some(pattern),
        variant_schedule: Vec::new(),
        realization: RealizationParams::default(),
        per_note_overrides: Vec::new(),
    }
}

fn alloc_activation_entry(alloc: &mut crate::project::IdAllocators) -> u64 {
    let id = alloc.next_activation_entry_id;
    alloc.next_activation_entry_id += 1;
    id
}

// ─── Arrangement ──────────────────────────────────────────────────────────

fn build_arrangement(
    alloc: &mut crate::project::IdAllocators,
    s: &Round1SectionIds,
) -> Arrangement {
    let mut sections = Vec::new();
    let mut push = |section: SectionId, variant: VariantId, start_bars: i64| {
        sections.push(SectionRef {
            id: alloc.alloc_section_ref(),
            section,
            variant,
            start: MusicalTime::bars(start_bars, 4),
        });
    };
    push(s.intro, VariantId::base(), 0);
    push(s.verse, VariantId::base(), 4);
    push(s.verse, VariantId::new("stripped"), 8);
    push(s.verse, VariantId::base(), 12);
    push(s.chorus, VariantId::base(), 16);
    Arrangement { sections }
}

// ─── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arrangement_is_five_steps_24_bars() {
        let (p, _) = build_round1_project();
        assert_eq!(p.arrangement.sections.len(), 5);
        // 24 bars total: intro(4) + verse(4) + verse-stripped(4) +
        // verse(4) + chorus(8).
        let last = p.arrangement.sections.last().expect("non-empty arrangement");
        let last_section = &p.sections[&last.section];
        // 4/4 throughout the round-1 fixture: 4 beats per bar.
        let last_start_bars = (last.start.as_beats_f64() / 4.0) as u32;
        let last_end_bars = last_start_bars + last_section.base.duration_bars;
        assert_eq!(last_end_bars, 24);
    }

    #[test]
    fn pad_absent_from_verse_base() {
        let (p, keys) = build_round1_project();
        let verse = &p.sections[&keys.sections.verse];
        assert!(
            !verse.base.activations.contains_key(&keys.tracks.pad),
            "pad must not have a verse-base activation — the section \
             editor expects to render an inherit placeholder"
        );
    }

    #[test]
    fn verse_has_stripped_variant_with_three_overrides() {
        let (p, keys) = build_round1_project();
        let verse = &p.sections[&keys.sections.verse];
        let stripped = verse
            .variants
            .get(&VariantId::new("stripped"))
            .expect("stripped variant exists");
        assert_eq!(stripped.activations.len(), 3, "bass + lead + drums");
        assert!(matches!(
            stripped.activations[&keys.tracks.bass],
            ActivationOverride::Silent
        ));
        assert!(matches!(
            stripped.activations[&keys.tracks.drums],
            ActivationOverride::Silent
        ));
        assert!(matches!(
            stripped.activations[&keys.tracks.lead],
            ActivationOverride::Replace(_)
        ));
    }

    #[test]
    fn chorus_drums_has_single_fill_schedule_entry() {
        let (p, keys) = build_round1_project();
        let chorus = &p.sections[&keys.sections.chorus];
        let drums = chorus
            .base
            .activations
            .get(&keys.tracks.drums)
            .expect("chorus drums activation");
        assert_eq!(
            drums.variant_schedule.len(),
            1,
            "round-1 chorus drums schedule has exactly one entry"
        );
        let (range, variant) = &drums.variant_schedule[0];
        assert_eq!(range.start, 7);
        assert_eq!(range.end, 8);
        assert_eq!(variant.as_str(), "fill");
    }

    #[test]
    fn chorus_has_all_four_tracks_active() {
        let (p, keys) = build_round1_project();
        let chorus = &p.sections[&keys.sections.chorus];
        for tr in [keys.tracks.bass, keys.tracks.lead, keys.tracks.drums, keys.tracks.pad] {
            assert!(
                chorus.base.activations.contains_key(&tr),
                "chorus is the section where pad lands"
            );
        }
    }

    #[test]
    fn drums_pattern_carries_main_and_fill_variants() {
        let (p, keys) = build_round1_project();
        let drums = &p.patterns[&keys.patterns.drums];
        let variants: Vec<_> = match &drums.body {
            PatternBody::Drum(body) => body.variants.keys().map(|v| v.as_str().to_string()).collect(),
            _ => panic!("drums-main should have a drum body"),
        };
        assert!(variants.contains(&"main".to_string()));
        assert!(variants.contains(&"fill".to_string()));
    }

    #[test]
    fn all_patterns_carry_default_main_variant() {
        let (p, _) = build_round1_project();
        for pat in p.patterns.values() {
            assert_eq!(
                pat.default_variant,
                VariantId::main(),
                "round-1 patterns all default to `main`",
            );
        }
    }

    #[test]
    fn ids_match_lookup_paths() {
        // The Round1Keys bundle must agree with the by-name lookups —
        // callers may use either path.
        let (p, keys) = build_round1_project();
        let bass_by_name = p
            .patterns
            .values()
            .find(|pat| pat.name == "bass-main")
            .expect("bass-main exists");
        assert_eq!(bass_by_name.id, keys.patterns.bass);

        let verse_by_name = p
            .sections
            .values()
            .find(|s| s.name == "verse")
            .expect("verse exists");
        assert_eq!(verse_by_name.id, keys.sections.verse);
    }
}
