//! Tiny verse-style project covering every kind of object in the model.
//! Used by `rawdaw-model` tests (smoke / roundtrip) and downstream
//! crates that want a quick well-formed `Project` for integration tests.
//!
//! Lives under the `fixtures` Cargo feature — release builds of
//! `rawdaw-model` don't ship the constructor.

use std::collections::BTreeMap;

use crate::*;

/// Build a 4-bar verse-style project in C major with bass, melody, and drums.
/// The verse section has a `stripped` variant that silences drums, and the
/// arrangement plays `base → stripped → base`.
pub fn build_tiny_project() -> Project {
    let mut project = Project::new(Scale::major(PitchClass::C));
    let alloc = &mut project.id_allocators;

    // ---------- A chord loop: I - V - vi - IV in C major ----------
    let chord_loop_id = alloc.alloc_chord_loop();
    let chord_loop = ChordLoop {
        id: chord_loop_id,
        name: "verse-progression".into(),
        length: Duration::bars(4, 4),
        key: None,
        events: vec![
            ChordEvent {
                time: MusicalTime::beats(0),
                duration: Duration::beats(4),
                chord: ChordSpec::Functional {
                    roman: RomanDegree::I,
                    suffix: ChordSuffix::new(ChordQuality::Major),
                    in_key: None,
                },
                bass: None,
                annotation: None,
            },
            ChordEvent {
                time: MusicalTime::beats(4),
                duration: Duration::beats(4),
                chord: ChordSpec::Functional {
                    roman: RomanDegree::V,
                    suffix: ChordSuffix::new(ChordQuality::Major),
                    in_key: None,
                },
                bass: None,
                annotation: None,
            },
            ChordEvent {
                time: MusicalTime::beats(8),
                duration: Duration::beats(4),
                chord: ChordSpec::Functional {
                    roman: RomanDegree::VI,
                    suffix: ChordSuffix::new(ChordQuality::Minor),
                    in_key: None,
                },
                bass: None,
                annotation: None,
            },
            ChordEvent {
                time: MusicalTime::beats(12),
                duration: Duration::beats(4),
                chord: ChordSpec::Functional {
                    roman: RomanDegree::IV,
                    suffix: ChordSuffix::new(ChordQuality::Major),
                    in_key: None,
                },
                bass: None,
                annotation: Some(Annotation {
                    cadence: Some(CadenceTag::Plagal),
                    comment: None,
                }),
            },
        ],
    };
    project.chord_loops.insert(chord_loop_id, chord_loop);

    let main_variant: VariantId = "main".into();

    // ---------- Bass pattern: roots on downbeats ----------
    let bass_pattern_id = alloc.alloc_pattern();
    let mut bass_variants = BTreeMap::new();
    bass_variants.insert(
        main_variant.clone(),
        vec![
            PitchedEvent {
                note_id: alloc.alloc_note(),
                time: MusicalTime::beats(0),
                duration: Duration::beats(1),
                velocity: U7::clamp(96),
                articulation: None,
                humanization: EventHumanization::default(),
                spec: PitchSpec::Chord {
                    degree: ChordDegree::new(ChordStep::Root),
                    octave: OctaveSpec::RelativeToRole,
                },
            },
            PitchedEvent {
                note_id: alloc.alloc_note(),
                time: MusicalTime::beats(2),
                duration: Duration::beats(1),
                velocity: U7::clamp(80),
                articulation: None,
                humanization: EventHumanization::default(),
                spec: PitchSpec::Chord {
                    degree: ChordDegree::new(ChordStep::Fifth),
                    octave: OctaveSpec::Nearest,
                },
            },
        ],
    );
    project.patterns.insert(
        bass_pattern_id,
        Pattern {
            id: bass_pattern_id,
            name: "bass-main".into(),
            default_variant: main_variant.clone(),
            body: PatternBody::Pitched(PitchedPatternBody {
                metadata: PitchedPatternMetadata {
                    length: Duration::bars(1, 4),
                },
                variants: bass_variants,
            }),
        },
    );

    // ---------- Melody pattern: scale degree + chromatic ornament + resolution ----------
    let melody_pattern_id = alloc.alloc_pattern();
    let mut melody_variants = BTreeMap::new();
    melody_variants.insert(
        main_variant.clone(),
        vec![
            PitchedEvent {
                note_id: alloc.alloc_note(),
                time: MusicalTime::beats(0),
                duration: Duration::beats(1),
                velocity: U7::clamp(100),
                articulation: None,
                humanization: EventHumanization::default(),
                spec: PitchSpec::Scale {
                    degree: ScaleDegree::new(3),
                    octave: OctaveSpec::Anchored(Octave(5)),
                },
            },
            PitchedEvent {
                note_id: alloc.alloc_note(),
                time: MusicalTime::beats(1),
                duration: Duration::ticks(PPQ / 2),
                velocity: U7::clamp(70),
                articulation: Some(ArticulationTag::new("staccato")),
                humanization: EventHumanization::default(),
                spec: PitchSpec::Chromatic {
                    semitones_from_prev: -1,
                },
            },
            PitchedEvent {
                note_id: alloc.alloc_note(),
                time: MusicalTime::beats(2),
                duration: Duration::beats(2),
                velocity: U7::clamp(96),
                articulation: None,
                humanization: EventHumanization::default(),
                spec: PitchSpec::Scale {
                    degree: ScaleDegree::new(1),
                    octave: OctaveSpec::Nearest,
                },
            },
        ],
    );
    project.patterns.insert(
        melody_pattern_id,
        Pattern {
            id: melody_pattern_id,
            name: "verse-melody".into(),
            default_variant: main_variant.clone(),
            body: PatternBody::Pitched(PitchedPatternBody {
                metadata: PitchedPatternMetadata {
                    length: Duration::bars(1, 4),
                },
                variants: melody_variants,
            }),
        },
    );

    // ---------- Drum pattern: kick on 1+3, snare on 2+4, hat 16ths ----------
    let drums_pattern_id = alloc.alloc_pattern();
    let fill_variant: VariantId = "fill".into();
    let mut drum_variants = BTreeMap::new();
    let mut main_drums = Vec::new();
    for beat in 0..4i64 {
        for sub in 0..4 {
            main_drums.push(DrumEvent {
                note_id: alloc.alloc_note(),
                time: MusicalTime::beats(beat) + MusicalTime::ticks(sub * (PPQ / 4)),
                duration: Duration::ticks(PPQ / 4),
                voice: DrumVoice::ClosedHat,
                velocity: U7::clamp(if sub == 0 { 96 } else { 70 }),
                articulation: None,
                humanization: EventHumanization::default(),
            });
        }
        let on_beat_voice = if beat % 2 == 0 {
            DrumVoice::Kick
        } else {
            DrumVoice::Snare
        };
        main_drums.push(DrumEvent {
            note_id: alloc.alloc_note(),
            time: MusicalTime::beats(beat),
            duration: Duration::beats(1),
            voice: on_beat_voice,
            velocity: U7::clamp(if beat % 2 == 0 { 110 } else { 105 }),
            articulation: None,
            humanization: EventHumanization::default(),
        });
    }
    drum_variants.insert(main_variant.clone(), main_drums);
    drum_variants.insert(
        fill_variant.clone(),
        vec![DrumEvent {
            note_id: alloc.alloc_note(),
            time: MusicalTime::beats(3),
            duration: Duration::beats(1),
            voice: DrumVoice::Snare,
            velocity: U7::clamp(120),
            articulation: Some(ArticulationTag::new("roll")),
            humanization: EventHumanization::default(),
        }],
    );
    project.patterns.insert(
        drums_pattern_id,
        Pattern {
            id: drums_pattern_id,
            name: "verse-drums".into(),
            default_variant: main_variant.clone(),
            body: PatternBody::Drum(DrumPatternBody {
                metadata: DrumPatternMetadata {
                    length: Duration::bars(1, 4),
                    voices: vec![DrumVoice::Kick, DrumVoice::Snare, DrumVoice::ClosedHat],
                },
                variants: drum_variants,
            }),
        },
    );

    // ---------- Drum kit + tracks ----------
    let drum_kit_id = DrumKitId::new(0);
    project.drum_kits.insert(
        drum_kit_id,
        DrumKitStub {
            id: drum_kit_id,
            name: "default-kit".into(),
        },
    );

    let bass_track_id = alloc.alloc_track();
    let melody_track_id = alloc.alloc_track();
    let drum_track_id = alloc.alloc_track();
    project.tracks.push(Track::new(
        bass_track_id,
        "Bass".into(),
        TrackKind::Pitched { role: Role::Bass },
        InstrumentId::new(0),
        MixerPlacement { display_index: 0 },
    ));
    project.tracks.push(Track::new(
        melody_track_id,
        "Lead".into(),
        TrackKind::Pitched {
            role: Role::Melodic,
        },
        InstrumentId::new(1),
        MixerPlacement { display_index: 1 },
    ));
    project.tracks.push(Track::new(
        drum_track_id,
        "Drums".into(),
        TrackKind::Drum { kit: drum_kit_id },
        InstrumentId::new(2),
        MixerPlacement { display_index: 2 },
    ));

    // ---------- Verse section with all three tracks activated ----------
    let verse_section_id = alloc.alloc_section();
    let mut verse_activations = BTreeMap::new();
    verse_activations.insert(
        bass_track_id,
        ActivationEntry {
            id: ActivationEntryId::new(0),
            pattern_ref: Some(bass_pattern_id),
            variant_schedule: Vec::new(),
            realization: RealizationParams::default(),
            per_note_overrides: Vec::new(),
        },
    );
    verse_activations.insert(
        melody_track_id,
        ActivationEntry {
            id: ActivationEntryId::new(1),
            pattern_ref: Some(melody_pattern_id),
            variant_schedule: Vec::new(),
            realization: RealizationParams::default(),
            per_note_overrides: Vec::new(),
        },
    );
    verse_activations.insert(
        drum_track_id,
        ActivationEntry {
            id: ActivationEntryId::new(2),
            pattern_ref: Some(drums_pattern_id),
            variant_schedule: vec![(BarRange::new(3, 4), fill_variant.clone())],
            realization: RealizationParams {
                humanization: Humanization {
                    velocity_jitter: 4,
                    timing_jitter_ticks: 6,
                    swing: 0.0,
                },
                seed: 0xDEAD_BEEF,
                ..Default::default()
            },
            per_note_overrides: Vec::new(),
        },
    );

    let mut variants_map = BTreeMap::new();
    let mut stripped_acts = BTreeMap::new();
    stripped_acts.insert(drum_track_id, ActivationOverride::Silent);
    variants_map.insert(
        VariantId::from("stripped"),
        SectionVariantOverride {
            activations: stripped_acts,
            ..Default::default()
        },
    );

    project.sections.insert(
        verse_section_id,
        Section {
            id: verse_section_id,
            name: "verse".into(),
            base: SectionBody {
                duration_bars: 4,
                scale_override: None,
                chord_loops: vec![(BarRange::new(0, 4), chord_loop_id)],
                activations: verse_activations,
            },
            variants: variants_map,
            default_variant: VariantId::base(),
        },
    );

    // ---------- Arrangement: base, stripped, base ----------
    project.arrangement.sections.push(SectionRef {
        id: alloc.alloc_section_ref(),
        section: verse_section_id,
        variant: VariantId::base(),
        start: MusicalTime::ZERO,
    });
    project.arrangement.sections.push(SectionRef {
        id: alloc.alloc_section_ref(),
        section: verse_section_id,
        variant: VariantId::from("stripped"),
        start: MusicalTime::bars(4, 4),
    });
    project.arrangement.sections.push(SectionRef {
        id: alloc.alloc_section_ref(),
        section: verse_section_id,
        variant: VariantId::base(),
        start: MusicalTime::bars(8, 4),
    });

    project
}
