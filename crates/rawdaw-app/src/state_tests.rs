use super::*;

#[test]
fn select_track_clears_section_selection() {
    let app = AppState::new();
    app.selected_idx.set(Some(2));
    assert_eq!(app.selected_idx.get(), Some(2));

    app.select_track(Some(0));
    assert_eq!(app.selected_track.get(), Some(0));
    assert_eq!(app.selected_idx.get(), None);
}

#[test]
fn set_selected_idx_clears_track_selection() {
    let app = AppState::new();
    app.select_track(Some(1));
    assert_eq!(app.selected_track.get(), Some(1));

    app.set_selected_idx(Some(3));
    assert_eq!(app.selected_idx.get(), Some(3));
    assert_eq!(app.selected_track.get(), None);
}

#[test]
fn clearing_selection_does_not_touch_other_axis() {
    // Setting either axis to `None` is a pure clear — it must
    // never disturb the other axis. The mutex only fires on
    // `Some(_)` selections.
    let app = AppState::new();
    app.select_track(Some(2));

    app.set_selected_idx(None);
    assert_eq!(app.selected_track.get(), Some(2));

    app.select_track(None);
    assert_eq!(app.selected_idx.get(), None);
}

#[test]
fn select_track_sets_midi_target_track() {
    // K3: selecting a track also updates the MIDI routing
    // target so live MIDI plays through that synth.
    let app = AppState::new();
    app.select_track(Some(2));
    assert_eq!(app.midi_target_track.get(), Some(2));

    app.select_track(Some(3));
    assert_eq!(app.midi_target_track.get(), Some(3));
}

#[test]
fn midi_target_track_is_sticky_across_section_selection() {
    // K3 sticky-routing contract: clicking a section block
    // clears `selected_track` but PRESERVES `midi_target_track`.
    // The user can audition a synth, click away to inspect a
    // section, and still play the audited synth.
    let app = AppState::new();
    app.select_track(Some(2));
    assert_eq!(app.midi_target_track.get(), Some(2));

    app.set_selected_idx(Some(5));
    assert_eq!(
        app.midi_target_track.get(),
        Some(2),
        "midi target must survive section-block selection",
    );
    assert_eq!(app.selected_track.get(), None);
}

#[test]
fn midi_target_track_is_sticky_across_clear() {
    // Calling select_track(None) explicitly also preserves
    // midi_target_track — the K3 contract.
    let app = AppState::new();
    app.select_track(Some(1));
    app.select_track(None);
    assert_eq!(
        app.midi_target_track.get(),
        Some(1),
        "midi target must survive an explicit track clear",
    );
    assert_eq!(app.selected_track.get(), None);
}

#[test]
fn select_chord_loop_clears_section_and_track_selection() {
    // CL1 selection mutex: picking a chord loop clears both
    // other axes so the inspector branches deterministically.
    let app = AppState::new();
    app.selected_idx.set(Some(2));
    app.selected_track.set(Some(1));

    let id = ChordLoopId::new(7);
    app.select_chord_loop(Some(id));

    assert_eq!(app.selected_chord_loop.get(), Some(id));
    assert_eq!(app.selected_idx.get(), None);
    assert_eq!(app.selected_track.get(), None);
}

#[test]
fn other_axes_clear_chord_loop_selection() {
    // Symmetric: setting section or track to Some(_) clears the
    // chord-loop selection, completing the three-way mutex.
    let app = AppState::new();
    app.select_chord_loop(Some(ChordLoopId::new(3)));

    app.set_selected_idx(Some(0));
    assert_eq!(app.selected_chord_loop.get(), None);

    app.select_chord_loop(Some(ChordLoopId::new(3)));
    app.select_track(Some(2));
    assert_eq!(app.selected_chord_loop.get(), None);
}

#[test]
fn select_chord_loop_does_not_touch_midi_target() {
    // Chord-loop selection isn't a synth-target switch, so the
    // K3 sticky MIDI target is untouched.
    let app = AppState::new();
    app.select_track(Some(2));
    assert_eq!(app.midi_target_track.get(), Some(2));

    app.select_chord_loop(Some(ChordLoopId::new(1)));
    assert_eq!(
        app.midi_target_track.get(),
        Some(2),
        "chord-loop selection must not redirect MIDI input",
    );
}

#[test]
fn select_chord_loop_none_does_not_touch_other_axes() {
    // Clearing chord-loop selection is a pure clear; it must not
    // disturb the section or track selection.
    let app = AppState::new();
    app.set_selected_idx(Some(4));

    app.select_chord_loop(None);
    assert_eq!(app.selected_idx.get(), Some(4));
    assert_eq!(app.selected_chord_loop.get(), None);
}

#[test]
fn select_pattern_clears_other_three_axes() {
    // P1 selection mutex: picking a pattern clears section,
    // track, and chord-loop selection so the inspector branches
    // deterministically on a single axis.
    let app = AppState::new();
    app.selected_idx.set(Some(2));
    app.selected_track.set(Some(1));
    app.selected_chord_loop.set(Some(ChordLoopId::new(5)));

    let id = PatternId::new(11);
    app.select_pattern(Some(id));

    assert_eq!(app.selected_pattern.get(), Some(id));
    assert_eq!(app.selected_idx.get(), None);
    assert_eq!(app.selected_track.get(), None);
    assert_eq!(app.selected_chord_loop.get(), None);
}

#[test]
fn other_axes_clear_pattern_selection() {
    // Symmetric: setting any other axis to Some(_) clears the
    // pattern selection, completing the four-way mutex.
    let app = AppState::new();
    let pid = PatternId::new(7);

    app.select_pattern(Some(pid));
    app.set_selected_idx(Some(0));
    assert_eq!(app.selected_pattern.get(), None);

    app.select_pattern(Some(pid));
    app.select_track(Some(1));
    assert_eq!(app.selected_pattern.get(), None);

    app.select_pattern(Some(pid));
    app.select_chord_loop(Some(ChordLoopId::new(3)));
    assert_eq!(app.selected_pattern.get(), None);
}

#[test]
fn select_pattern_does_not_touch_midi_target() {
    // Pattern selection isn't a synth-target switch, so the K3
    // sticky MIDI target is untouched (mirrors chord-loop's
    // behavior).
    let app = AppState::new();
    app.select_track(Some(2));
    assert_eq!(app.midi_target_track.get(), Some(2));

    app.select_pattern(Some(PatternId::new(4)));
    assert_eq!(
        app.midi_target_track.get(),
        Some(2),
        "pattern selection must not redirect MIDI input",
    );
}

#[test]
fn select_pattern_none_does_not_touch_other_axes() {
    // Clearing pattern selection is a pure clear.
    let app = AppState::new();
    app.set_selected_idx(Some(3));

    app.select_pattern(None);
    assert_eq!(app.selected_idx.get(), Some(3));
    assert_eq!(app.selected_pattern.get(), None);
}

#[test]
fn select_pattern_resets_focused_note_and_variant() {
    // P2 contract: switching to a fresh pattern clears the
    // focused-note signal (idx into a different pattern's events
    // would be stale anyway, but the symmetric clear keeps the
    // inspector branching deterministically) and seeds
    // `focused_variant` from the pattern's `default_variant`.
    use rawdaw_model::pattern::{
        Pattern, PatternBody, PitchedPatternBody, PitchedPatternMetadata,
    };
    use rawdaw_model::time::Duration;
    use std::collections::BTreeMap;

    let app = AppState::new();
    // Manually inject a pattern so `select_pattern` can read its
    // default_variant. The empty default project AppState seeds
    // has no patterns, so the lookup would otherwise yield None.
    let pid = PatternId::new(42);
    let default_variant = VariantId::new("verse-line");
    let pattern = Pattern {
        id: pid,
        name: "test".into(),
        default_variant: default_variant.clone(),
        body: PatternBody::Pitched(PitchedPatternBody {
            metadata: PitchedPatternMetadata {
                length: Duration::beats(16),
            },
            variants: BTreeMap::new(),
        }),
    };
    let mut project = Project::new(Scale::major(PitchClass::C));
    project.patterns.insert(pid, pattern);
    app.project.set(Rc::new(project));

    // Pre-seed a stale focused-note + a different focused-variant
    // to prove `select_pattern` clears / replaces them.
    app.focused_pattern_note.set(Some(NoteId::new(7)));
    app.focused_variant.set(Some(VariantId::new("other")));

    app.select_pattern(Some(pid));
    assert_eq!(app.selected_pattern.get(), Some(pid));
    assert_eq!(app.focused_pattern_note.get(), None);
    assert_eq!(app.focused_variant.get(), Some(default_variant));
}

#[test]
fn select_pattern_none_clears_focused_note_and_variant() {
    let app = AppState::new();
    app.focused_pattern_note.set(Some(NoteId::new(3)));
    app.focused_variant.set(Some(VariantId::new("main")));

    app.select_pattern(None);
    assert_eq!(app.focused_pattern_note.get(), None);
    assert_eq!(app.focused_variant.get(), None);
}

#[test]
fn select_pattern_resets_editor_mode_to_arrangement() {
    // Mirrors select_chord_loop: opening a pattern editor mounts
    // the new region inside the arrangement surface even if the
    // user was in section-editor mode.
    let app = AppState::new();
    app.open_section_editor("verse", "base");
    assert!(matches!(
        app.editor_mode.get(),
        EditorMode::SectionEditor { .. },
    ));

    app.select_pattern(Some(PatternId::new(9)));
    assert_eq!(app.editor_mode.get(), EditorMode::Arrangement);
}

// ---------- selected_section (S2 selection axis) ----------

#[test]
fn select_section_clears_other_four_axes() {
    // Symmetric: selecting a section template clears the other
    // four axes, completing the five-way mutex S2 establishes.
    let app = AppState::new();
    let sid = SectionId::new(11);

    app.set_selected_idx(Some(0));
    app.select_section(Some(sid));
    assert_eq!(app.selected_section.get(), Some(sid));
    assert_eq!(app.selected_idx.get(), None);

    app.select_track(Some(2));
    app.select_section(Some(sid));
    assert_eq!(app.selected_section.get(), Some(sid));
    assert_eq!(app.selected_track.get(), None);

    app.select_chord_loop(Some(ChordLoopId::new(3)));
    app.select_section(Some(sid));
    assert_eq!(app.selected_section.get(), Some(sid));
    assert_eq!(app.selected_chord_loop.get(), None);

    app.select_pattern(Some(PatternId::new(4)));
    app.select_section(Some(sid));
    assert_eq!(app.selected_section.get(), Some(sid));
    assert_eq!(app.selected_pattern.get(), None);
}

#[test]
fn other_four_axes_clear_selected_section() {
    // Symmetric inverse: setting any other axis to Some(_) clears
    // selected_section.
    let app = AppState::new();
    let sid = SectionId::new(11);

    app.select_section(Some(sid));
    app.set_selected_idx(Some(0));
    assert_eq!(app.selected_section.get(), None);

    app.select_section(Some(sid));
    app.select_track(Some(2));
    assert_eq!(app.selected_section.get(), None);

    app.select_section(Some(sid));
    app.select_chord_loop(Some(ChordLoopId::new(3)));
    assert_eq!(app.selected_section.get(), None);

    app.select_section(Some(sid));
    app.select_pattern(Some(PatternId::new(4)));
    assert_eq!(app.selected_section.get(), None);
}

#[test]
fn select_section_does_not_touch_midi_target() {
    // Section selection isn't a synth-target switch.
    let app = AppState::new();
    app.select_track(Some(2));
    assert_eq!(app.midi_target_track.get(), Some(2));

    app.select_section(Some(SectionId::new(11)));
    assert_eq!(
        app.midi_target_track.get(),
        Some(2),
        "section selection must not redirect MIDI input",
    );
}

#[test]
fn select_section_none_does_not_touch_other_axes() {
    // Clearing section selection is a pure clear.
    let app = AppState::new();
    app.set_selected_idx(Some(3));

    app.select_section(None);
    assert_eq!(app.selected_idx.get(), Some(3));
    assert_eq!(app.selected_section.get(), None);
}

#[test]
fn select_section_does_not_change_editor_mode() {
    // Unlike select_chord_loop / select_pattern, select_section
    // leaves editor_mode untouched. S3 wires the editor mount
    // off `selected_section` rather than forcing a mode switch
    // at selection time.
    let app = AppState::new();
    app.open_section_editor("verse", "base");
    let before = app.editor_mode.get();

    app.select_section(Some(SectionId::new(11)));
    assert_eq!(app.editor_mode.get(), before);
}
