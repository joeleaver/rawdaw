//! Realized strip — concrete pitches per pattern event.
//!
//! Sits beneath the piano-roll as a horizontal lane of width-
//! proportional cells. Each cell shows the absolute pitch class +
//! octave the event resolves to under the project's default key
//! (and, for `PitchSpec::Chord` events, against the chord-context
//! preview chord — `I` in the project key by default).
//!
//! The chord-context preview is a UI-local concept per P2 design
//! decision 4: it does not serialize. v1 ships a fixed `I` preview;
//! the picker UI lands in a later polish pass.
//!
//! Parallels `regions/chord_loop_editor/realized_strip.rs`.

use rinch::prelude::*;

use rawdaw_model::chord::{ChordDegree, ChordQuality, ChordSuffix};
use rawdaw_model::id::PatternId;
use rawdaw_model::pattern::{OctaveSpec, PatternBody, PitchSpec, PitchedEvent};
use rawdaw_model::pitch::{MidiNote, Octave, PitchClass};
use rawdaw_model::realize::resolve::{
    nearest_to, resolve_chord_degree, resolve_scale_degree,
};
use rawdaw_model::scale::Scale;

use crate::chord_display::pitch_class_name;
use crate::regions::pattern_editor::pitched::helpers::DEFAULT_ANCHOR_OCTAVE;
use crate::state::AppState;
use crate::theme;

#[component]
pub fn RealizedStrip(id: PatternId) -> NodeHandle {
    let label_style = "font-size: 10px; letter-spacing: 0.6px; text-transform: uppercase; \
         color: rgba(232,234,238,0.42); font-weight: 600;"
        .to_string();
    let strip_style = format!(
        "display: flex; align-items: stretch; \
         min-height: 28px; padding: 4px; gap: 2px; \
         background: {bg0}; border: 1px solid {line}; border-radius: 4px;",
        bg0 = theme::BG0,
        line = theme::LINE,
    );

    rsx! {
        div { style: "display: flex; flex-direction: column; gap: 4px;",
            div { style: {label_style.clone()}, "Realized" }
            div { style: {strip_style.clone()},
                for cell in build_realized_cells(id) {
                    div {
                        key: cell.note_id_key.clone(),
                        style: {cell_style(cell.width_frac)},
                        span { style: {cell_label_style()}, {cell.label.clone()} }
                    }
                }
            }
        }
    }
}

// ─── Cell building ────────────────────────────────────────────────────────

#[derive(Clone, PartialEq)]
struct RealizedCell {
    note_id_key: String,
    label: String,
    width_frac: f32,
}

fn build_realized_cells(id: PatternId) -> Vec<RealizedCell> {
    let app = use_store::<AppState>();
    let project = app.project.get();
    let Some(pattern) = project.patterns.get(&id) else {
        return Vec::new();
    };
    let body = match &pattern.body {
        PatternBody::Pitched(b) => b,
        PatternBody::Drum(_) => return Vec::new(),
    };

    let variant = app
        .focused_variant
        .get()
        .filter(|v| body.variants.contains_key(v))
        .unwrap_or_else(|| pattern.default_variant.clone());
    let events = body.variants.get(&variant).cloned().unwrap_or_default();

    let scale = project.default_key.clone();
    // Preview chord defaults to `I` in the project's default key.
    // P2 design decision 4: editor-local; picker UI lands later.
    let preview_chord = preview_chord_for(&scale);

    let total_ticks = body.metadata.length.as_ticks().max(1) as f32;

    let mut sorted: Vec<&PitchedEvent> = events.iter().collect();
    sorted.sort_by_key(|e| e.time.as_ticks());

    sorted
        .into_iter()
        .map(|ev| {
            let label = realize_event_label(ev, &scale, &preview_chord);
            let width = ev.duration.as_ticks() as f32 / total_ticks;
            RealizedCell {
                note_id_key: format!("n{}", ev.note_id.get()),
                label,
                width_frac: width.clamp(0.0, 1.0),
            }
        })
        .collect()
}

/// `I` chord in the project's default key. Major triad in major
/// modes, minor triad in minor — picked by inspecting the mode.
fn preview_chord_for(scale: &Scale) -> PreviewChord {
    use rawdaw_model::scale::Mode::*;
    let quality = match scale.mode {
        Ionian | Lydian | Mixolydian | MajorPentatonic | WholeTone => ChordQuality::Major,
        Aeolian | Dorian | Phrygian | Locrian | HarmonicMinor | MelodicMinor
        | PhrygianDominant | Altered | MinorPentatonic | Blues => ChordQuality::Minor,
        Lydian7 => ChordQuality::Dominant7,
        Chromatic => ChordQuality::Major,
        Custom { .. } => ChordQuality::Major,
    };
    PreviewChord {
        root: scale.tonic,
        suffix: ChordSuffix::new(quality),
    }
}

struct PreviewChord {
    root: PitchClass,
    suffix: ChordSuffix,
}

fn realize_event_label(event: &PitchedEvent, scale: &Scale, preview: &PreviewChord) -> String {
    match &event.spec {
        PitchSpec::Rest => "—".to_string(),
        PitchSpec::Absolute { pitch_class, octave } => {
            format!("{}{}", pitch_class_name(*pitch_class), octave.0)
        }
        PitchSpec::Scale { degree, octave } => {
            let pc = resolve_scale_degree(*degree, scale);
            let oct = pick_preview_octave(pc, *octave);
            format!("{}{}", pitch_class_name(pc), oct.0)
        }
        PitchSpec::Chord { degree, octave } => {
            match preview_chord_degree(*degree, preview) {
                Some(pc) => {
                    let oct = pick_preview_octave(pc, *octave);
                    format!("{}{}", pitch_class_name(pc), oct.0)
                }
                None => "—".to_string(),
            }
        }
        PitchSpec::Chromatic { semitones_from_prev } => {
            // v1 doesn't track prev-pitch state in the strip — show
            // the raw delta so the user can see the intent.
            format!("Δ{semitones_from_prev:+}")
        }
    }
}

fn preview_chord_degree(degree: ChordDegree, preview: &PreviewChord) -> Option<PitchClass> {
    resolve_chord_degree(degree, preview.root, &preview.suffix)
}

/// Translate the event's `OctaveSpec` into a concrete preview
/// `Octave`. Voice-leading specs (Nearest / UpFromPrev /
/// DownFromPrev) bias around the anchor octave since the strip
/// doesn't track previous-pitch state. `Anchored` honors the spec.
fn pick_preview_octave(pc: PitchClass, spec: OctaveSpec) -> Octave {
    match spec {
        OctaveSpec::Anchored(o) => o,
        OctaveSpec::Nearest | OctaveSpec::RelativeToRole => {
            let anchor =
                MidiNote::from_pitch_octave(pc, DEFAULT_ANCHOR_OCTAVE).unwrap_or(MidiNote::MIDDLE_C);
            nearest_to(pc, anchor).octave()
        }
        OctaveSpec::UpFromPrev => Octave(DEFAULT_ANCHOR_OCTAVE.0 + 1),
        OctaveSpec::DownFromPrev => Octave(DEFAULT_ANCHOR_OCTAVE.0 - 1),
    }
}

// ─── Styles ───────────────────────────────────────────────────────────────

fn cell_style(width_frac: f32) -> String {
    let pct = (width_frac * 100.0).clamp(0.0, 100.0);
    format!(
        "flex: 0 0 calc({pct:.4}% - 2px); \
         display: flex; align-items: center; justify-content: center; \
         min-width: 0; padding: 0 4px; \
         border-radius: 3px; background: {bg1};",
        bg1 = theme::BG1,
    )
}

fn cell_label_style() -> String {
    format!(
        "font-size: 11px; font-variant-numeric: tabular-nums; \
         color: {text1}; \
         white-space: nowrap; overflow: hidden; text-overflow: ellipsis;",
        text1 = theme::TEXT1,
    )
}
