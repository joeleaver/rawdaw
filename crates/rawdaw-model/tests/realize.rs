//! End-to-end realization tests against the smoke-test fixture.

mod common;

use rawdaw_model::*;

const SAMPLE_RATE: u32 = 48_000;

#[test]
fn realizing_tiny_project_produces_events() {
    let project = common::build_tiny_project();
    let events = realize(&project, SAMPLE_RATE);
    assert!(!events.is_empty(), "realize emitted no events");
}

#[test]
fn events_are_sorted_by_time() {
    let project = common::build_tiny_project();
    let events = realize(&project, SAMPLE_RATE);
    let mut prev = 0u64;
    for e in &events {
        let t = e.time.as_samples();
        assert!(t >= prev, "events out of order: {prev} -> {t}");
        prev = t;
    }
}

#[test]
fn every_note_on_has_a_matching_note_off() {
    let project = common::build_tiny_project();
    let events = realize(&project, SAMPLE_RATE);
    let mut note_ons = 0;
    let mut note_offs = 0;
    for e in &events {
        match e.message {
            Midi2Message::NoteOn { .. } => note_ons += 1,
            Midi2Message::NoteOff { .. } => note_offs += 1,
            // realize() does not synthesize CC or pitch-bend events
            // — those come from live MIDI input and pass through
            // translate_events untouched. Any in the realized
            // stream is a bug.
            Midi2Message::ControlChange { .. } | Midi2Message::PitchBend { .. } => {
                panic!("realize() emitted an unexpected control event: {:?}", e.message)
            }
        }
    }
    assert!(note_ons > 0);
    assert_eq!(note_ons, note_offs, "NoteOn/NoteOff counts must match");
}

#[test]
fn drum_events_emit_on_channel_9_with_gm_notes() {
    let project = common::build_tiny_project();
    let events = realize(&project, SAMPLE_RATE);

    // Find the drum track id by name.
    let drum_track = project
        .tracks
        .iter()
        .find(|t| t.name == "Drums")
        .expect("drum track exists")
        .id;

    let mut saw_kick = false;
    let mut saw_snare = false;
    let mut saw_hat = false;
    let mut saw_anything = false;

    for e in &events {
        if e.target != drum_track {
            continue;
        }
        saw_anything = true;
        if let Midi2Message::NoteOn {
            channel, note, ..
        } = &e.message
        {
            assert_eq!(
                channel.get(),
                9,
                "drum events must be on MIDI channel 9 (GM drums)"
            );
            match note.get() {
                36 => saw_kick = true,
                38 => saw_snare = true,
                42 => saw_hat = true,
                _ => {}
            }
        }
    }
    assert!(saw_anything, "no drum events realized");
    assert!(saw_kick, "no kick (MIDI 36) realized");
    assert!(saw_snare, "no snare (MIDI 38) realized");
    assert!(saw_hat, "no closed hat (MIDI 42) realized");
}

#[test]
fn bass_track_resolves_chord_roots_in_c_major() {
    // The bass pattern emits chord-degree Root at beat 0 and Fifth at beat 2.
    // Over a 4-bar I-V-vi-IV chord loop (in C major), the roots at bar
    // boundaries should be C, G, A, F.
    let project = common::build_tiny_project();
    let events = realize(&project, SAMPLE_RATE);

    let bass_track = project
        .tracks
        .iter()
        .find(|t| t.name == "Bass")
        .expect("bass track exists")
        .id;

    // Collect bass NoteOn pitch classes in time order.
    let bass_pcs: Vec<u8> = events
        .iter()
        .filter(|e| e.target == bass_track)
        .filter_map(|e| match &e.message {
            Midi2Message::NoteOn { note, .. } => Some(note.get() % 12),
            _ => None,
        })
        .collect();

    // First note (beat 0 of bar 0) is the root of I = C (pitch class 0).
    assert_eq!(bass_pcs[0], 0, "first bass note should be C (root of I)");

    // The bass pattern is 1 bar long and loops 4 times across the 4-bar verse.
    // The pattern emits Root then Fifth per bar. Roots per bar are I, V, vi, IV
    // → C(0), G(7), A(9), F(5). Fifths are G(7), D(2), E(4), C(0).
    // The pattern is: bar 0 [C, G], bar 1 [G, D], bar 2 [A, E], bar 3 [F, C].
    let expected_roots = [0, 7, 9, 5]; // C, G, A, F
    let expected_fifths = [7, 2, 4, 0]; // G, D, E, C
    for bar in 0..4 {
        let root_idx = bar * 2;
        let fifth_idx = bar * 2 + 1;
        assert_eq!(
            bass_pcs[root_idx], expected_roots[bar],
            "bar {bar} root mismatch"
        );
        assert_eq!(
            bass_pcs[fifth_idx], expected_fifths[bar],
            "bar {bar} fifth mismatch"
        );
    }
}

#[test]
fn provenance_traces_back_to_pattern_event() {
    let project = common::build_tiny_project();
    let events = realize(&project, SAMPLE_RATE);

    // Every realized event must carry a NoteId that exists somewhere in the
    // project's patterns.
    let mut known_note_ids: std::collections::HashSet<NoteId> =
        std::collections::HashSet::new();
    for pattern in project.patterns.values() {
        match &pattern.body {
            PatternBody::Pitched(b) => {
                for events in b.variants.values() {
                    for e in events {
                        known_note_ids.insert(e.note_id);
                    }
                }
            }
            PatternBody::Drum(b) => {
                for events in b.variants.values() {
                    for e in events {
                        known_note_ids.insert(e.note_id);
                    }
                }
            }
        }
    }

    for ev in &events {
        assert!(
            known_note_ids.contains(&ev.provenance.event_note_id),
            "event provenance references unknown NoteId {:?}",
            ev.provenance.event_note_id
        );
    }
}

#[test]
fn stripped_section_silences_drums() {
    // The arrangement is base → stripped → base. In the stripped variant,
    // drums are silent. So drum events should appear only in the first and
    // third sections.
    let project = common::build_tiny_project();
    let events = realize(&project, SAMPLE_RATE);

    let drum_track = project
        .tracks
        .iter()
        .find(|t| t.name == "Drums")
        .unwrap()
        .id;

    // Section refs are at bars 0, 4, 8. With 4/4 at 120 BPM, each bar is
    // 2 seconds = 96000 samples. So:
    //   stripped section starts at sample 4 * 96000 = 384000
    //   stripped section ends at sample 8 * 96000 = 768000
    // Drum events whose start time falls in [384000, 768000) should not exist.
    let stripped_start = 384_000u64;
    let stripped_end = 768_000u64;

    let drum_events_in_stripped: Vec<_> = events
        .iter()
        .filter(|e| e.target == drum_track)
        .filter(|e| matches!(e.message, Midi2Message::NoteOn { .. }))
        .filter(|e| {
            let t = e.time.as_samples();
            t >= stripped_start && t < stripped_end
        })
        .collect();

    assert!(
        drum_events_in_stripped.is_empty(),
        "expected no drum NoteOns in the stripped section, got {} of them",
        drum_events_in_stripped.len()
    );

    // Sanity: drums should still play in the base sections.
    let drum_events_in_base: Vec<_> = events
        .iter()
        .filter(|e| e.target == drum_track)
        .filter(|e| matches!(e.message, Midi2Message::NoteOn { .. }))
        .filter(|e| {
            let t = e.time.as_samples();
            t < stripped_start || t >= stripped_end
        })
        .collect();
    assert!(
        !drum_events_in_base.is_empty(),
        "drums should play in the non-stripped sections"
    );
}

// ---------- Variant duration override ----------

#[test]
fn section_variant_can_override_duration_bars() {
    let mut project = common::build_tiny_project();

    // Add a "short" variant that runs only 2 bars instead of 4.
    let verse_id = *project.sections.keys().next().unwrap();
    project
        .sections
        .get_mut(&verse_id)
        .unwrap()
        .variants
        .insert(
            VariantId::from("short"),
            SectionVariantOverride {
                duration_bars: Some(2),
                ..Default::default()
            },
        );

    // Append a SectionRef using the "short" variant *after* the existing 3
    // section refs (which together span bars 0..12).
    let short_start = MusicalTime::bars(12, 4);
    let id = project.id_allocators.alloc_section_ref();
    project.arrangement.sections.push(SectionRef {
        id,
        section: verse_id,
        variant: VariantId::from("short"),
        start: short_start,
    });

    let events = realize(&project, SAMPLE_RATE);

    let bass_track = project
        .tracks
        .iter()
        .find(|t| t.name == "Bass")
        .unwrap()
        .id;

    let short_start_sample = project.tempo_map.musical_to_sample(short_start, SAMPLE_RATE);

    let bass_notes_in_short: Vec<_> = events
        .iter()
        .filter(|e| e.target == bass_track)
        .filter(|e| matches!(e.message, Midi2Message::NoteOn { .. }))
        .filter(|e| e.time >= short_start_sample)
        .collect();

    // The bass pattern is 1 bar long, 2 NoteOns per bar (Root + Fifth). A
    // 2-bar variant should produce 4 NoteOns, not the 8 a 4-bar section would.
    assert_eq!(
        bass_notes_in_short.len(),
        4,
        "short variant (2 bars) should produce 4 bass NoteOns, got {}",
        bass_notes_in_short.len()
    );
}

// ---------- Per-note overrides ----------

#[test]
fn pin_pitch_override_replaces_resolved_note() {
    let mut project = common::build_tiny_project();

    // Find the bass pattern's first event NoteId.
    let bass_pattern_id = project
        .patterns
        .values()
        .find(|p| p.name == "bass-main")
        .unwrap()
        .id;
    let first_note_id = match &project.patterns[&bass_pattern_id].body {
        PatternBody::Pitched(b) => b.variants[&VariantId::from("main")][0].note_id,
        _ => unreachable!(),
    };

    let bass_track_id = project
        .tracks
        .iter()
        .find(|t| t.name == "Bass")
        .unwrap()
        .id;
    let verse_id = *project.sections.keys().next().unwrap();

    // Pin the first bass event to MIDI 12 (C0) — far from any natural resolution.
    let override_id = project.id_allocators.alloc_note_override();
    project
        .sections
        .get_mut(&verse_id)
        .unwrap()
        .base
        .activations
        .get_mut(&bass_track_id)
        .unwrap()
        .per_note_overrides
        .push(NoteOverride {
            id: override_id,
            target: first_note_id,
            transform: OverrideTransform::PinPitch(MidiNote::new(12).unwrap()),
        });

    let events = realize(&project, SAMPLE_RATE);
    let first_bass_on = events
        .iter()
        .find(|e| {
            e.target == bass_track_id && matches!(e.message, Midi2Message::NoteOn { .. })
        })
        .expect("at least one bass NoteOn");

    match first_bass_on.message {
        Midi2Message::NoteOn { note, .. } => {
            assert_eq!(note.get(), 12, "PinPitch override should force MIDI 12");
        }
        _ => unreachable!(),
    }
    assert_eq!(first_bass_on.provenance.override_applied, Some(override_id));
}

#[test]
fn pin_velocity_override_replaces_velocity() {
    let mut project = common::build_tiny_project();

    let bass_pattern_id = project
        .patterns
        .values()
        .find(|p| p.name == "bass-main")
        .unwrap()
        .id;
    let first_note_id = match &project.patterns[&bass_pattern_id].body {
        PatternBody::Pitched(b) => b.variants[&VariantId::from("main")][0].note_id,
        _ => unreachable!(),
    };
    let bass_track_id = project
        .tracks
        .iter()
        .find(|t| t.name == "Bass")
        .unwrap()
        .id;
    let verse_id = *project.sections.keys().next().unwrap();

    project
        .sections
        .get_mut(&verse_id)
        .unwrap()
        .base
        .activations
        .get_mut(&bass_track_id)
        .unwrap()
        .per_note_overrides
        .push(NoteOverride {
            id: project.id_allocators.alloc_note_override(),
            target: first_note_id,
            transform: OverrideTransform::PinVelocity(U7::clamp(1)),
        });

    let events = realize(&project, SAMPLE_RATE);
    let first_bass_on = events
        .iter()
        .find(|e| {
            e.target == bass_track_id && matches!(e.message, Midi2Message::NoteOn { .. })
        })
        .unwrap();
    let v = match first_bass_on.message {
        Midi2Message::NoteOn { velocity, .. } => velocity,
        _ => unreachable!(),
    };
    // U7::clamp(1) widened to 16-bit should be ~516 (1 * 516 + 0).
    assert!(v.get() < 1000, "pinned velocity should be tiny, got {}", v.get());
}

#[test]
fn mute_override_skips_event_emission() {
    let mut project = common::build_tiny_project();

    let bass_pattern_id = project
        .patterns
        .values()
        .find(|p| p.name == "bass-main")
        .unwrap()
        .id;
    let first_note_id = match &project.patterns[&bass_pattern_id].body {
        PatternBody::Pitched(b) => b.variants[&VariantId::from("main")][0].note_id,
        _ => unreachable!(),
    };
    let bass_track_id = project
        .tracks
        .iter()
        .find(|t| t.name == "Bass")
        .unwrap()
        .id;
    let verse_id = *project.sections.keys().next().unwrap();

    // Count bass NoteOns before adding the mute.
    let events_before = realize(&project, SAMPLE_RATE);
    let bass_count_before = events_before
        .iter()
        .filter(|e| e.target == bass_track_id)
        .filter(|e| matches!(e.message, Midi2Message::NoteOn { .. }))
        .count();

    // Mute the first event.
    project
        .sections
        .get_mut(&verse_id)
        .unwrap()
        .base
        .activations
        .get_mut(&bass_track_id)
        .unwrap()
        .per_note_overrides
        .push(NoteOverride {
            id: project.id_allocators.alloc_note_override(),
            target: first_note_id,
            transform: OverrideTransform::Mute,
        });

    let events_after = realize(&project, SAMPLE_RATE);
    let bass_count_after = events_after
        .iter()
        .filter(|e| e.target == bass_track_id)
        .filter(|e| matches!(e.message, Midi2Message::NoteOn { .. }))
        .count();

    // The bass plays in all three section refs (only *drums* are silenced in
    // the stripped variant). The pattern is 1 bar long, looped 4 times per
    // 4-bar section. So muting the first event removes 3 × 4 = 12 NoteOns.
    assert_eq!(
        bass_count_after,
        bass_count_before - 12,
        "muting first bass event should remove 12 NoteOns (3 sections × 4 iterations); got {} → {}",
        bass_count_before,
        bass_count_after
    );
}

#[test]
fn melody_chromatic_event_resolves_a_half_step_below_previous() {
    // The melody pattern: ScaleDegree(3, octave=Anchored(5)) → E5 (MIDI 76),
    // then Chromatic(-1) → 75 (D#5 / E♭5), then ScaleDegree(1, Nearest) → C5.
    let project = common::build_tiny_project();
    let events = realize(&project, SAMPLE_RATE);

    let melody_track = project
        .tracks
        .iter()
        .find(|t| t.name == "Lead")
        .unwrap()
        .id;

    let melody_notes: Vec<u8> = events
        .iter()
        .filter(|e| e.target == melody_track)
        .filter_map(|e| match &e.message {
            Midi2Message::NoteOn { note, .. } => Some(note.get()),
            _ => None,
        })
        .collect();

    // First three are the first iteration of the melody pattern.
    assert_eq!(melody_notes[0], 76, "scale degree 3 anchored to octave 5 = E5");
    assert_eq!(
        melody_notes[1], 75,
        "chromatic -1 from E5 should be D#5/E♭5 (MIDI 75)"
    );
    // Resolution to scale degree 1 (C) nearest to D#5 — C5 (60) and C6 (84)
    // are equidistant in semitones (15 vs 9). Nearest is C5 (closer at 9).
    assert_eq!(
        melody_notes[2], 72,
        "scale degree 1 nearest to D#5 should be C5 (MIDI 72)"
    );
}
