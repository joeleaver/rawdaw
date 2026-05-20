//! Realized strip — concrete pitches per chord event under the
//! current effective key.
//!
//! Sits beneath the chord-event timeline as a horizontal lane of
//! width-proportional cells. Each cell shows the chord's realized
//! pitch class + quality suffix (e.g. `G7` for V7 in C major) plus
//! `/PitchClass` when a bass override is set. The strip refreshes
//! reactively when the key, chord, or bass changes (project edits
//! flow through the C2 edit pump).
//!
//! ## Effective-key resolution (CL4 spec)
//!
//! 1. Chord event's `in_key` if set (handled inside
//!    [`resolve_chord_spec_root`]).
//! 2. The chord-loop's `key` if set.
//! 3. The project's `default_key` otherwise.
//!
//! Section-context scale override is a future hook (we'd need the
//! preview-section id passed in); v1 only opens the editor from
//! the library which doesn't carry a section context.

use rinch::prelude::*;

use rawdaw_model::chord::{BassSpec, ChordEvent, ChordSpec, ChordStep};
use rawdaw_model::id::ChordLoopId;
use rawdaw_model::pitch::PitchClass;
use rawdaw_model::realize::resolve::{
    resolve_chord_degree, resolve_chord_spec_root, resolve_scale_degree,
};
use rawdaw_model::scale::Scale;

use crate::chord_display::{pitch_class_name, quality_suffix};
use crate::state::AppState;
use crate::theme;

#[component]
pub(crate) fn RealizedStrip(id: ChordLoopId) -> NodeHandle {
    let strip_style = format!(
        "display: flex; align-items: stretch; \
         min-height: 28px; padding: 4px; gap: 2px; \
         background: {bg0}; border: 1px solid {line}; border-radius: 4px;",
        bg0 = theme::BG0,
        line = theme::LINE,
    );
    let label_style = "font-size: 10px; letter-spacing: 0.6px; text-transform: uppercase; \
         color: rgba(232,234,238,0.42); font-weight: 600; \
         display: flex; align-items: center; gap: 8px;"
        .to_string();

    rsx! {
        div { style: "display: flex; flex-direction: column; gap: 4px;",
            div { style: {label_style.clone()},
                span { "Realized" }
                span {
                    style: "font-weight: 400; letter-spacing: 0; text-transform: none; \
                            color: rgba(232,234,238,0.55);",
                    {move || effective_key_label(id)}
                }
            }
            div { style: {strip_style.clone()},
                for cell in build_realized_cells(id) {
                    div {
                        key: cell.idx.to_string(),
                        style: {cell_style(cell.width_frac)},
                        span { style: {cell_label_style()}, {cell.label.clone()} }
                    }
                }
            }
        }
    }
}

// ─── Cell building ──────────────────────────────────────────────────────

#[derive(Clone, PartialEq)]
struct RealizedCell {
    idx: usize,
    label: String,
    width_frac: f32,
}

fn build_realized_cells(id: ChordLoopId) -> Vec<RealizedCell> {
    let app = use_store::<AppState>();
    let project = app.project.get();
    let Some(loop_) = project.chord_loops.get(&id) else {
        return Vec::new();
    };
    let fallback_scale = loop_
        .key
        .clone()
        .unwrap_or_else(|| project.default_key.clone());
    let total_ticks = loop_.length.as_ticks().max(1) as f32;

    loop_
        .events
        .iter()
        .enumerate()
        .map(|(idx, ev)| RealizedCell {
            idx,
            label: format_realized(ev, &fallback_scale),
            width_frac: ev.duration.as_ticks() as f32 / total_ticks,
        })
        .collect()
}

fn format_realized(ev: &ChordEvent, fallback: &Scale) -> String {
    let effective = effective_scale_for_event(ev, fallback);
    let root = resolve_chord_spec_root(&ev.chord, fallback);
    let suffix = match &ev.chord {
        ChordSpec::Functional { suffix, .. } | ChordSpec::Absolute { suffix, .. } => suffix,
    };
    let mut out = format!(
        "{}{}",
        pitch_class_name(root),
        quality_suffix(&suffix.quality),
    );
    if let Some(bass_pc) = ev
        .bass
        .as_ref()
        .and_then(|b| resolve_bass(b, &ev.chord, &effective))
    {
        out.push('/');
        out.push_str(pitch_class_name(bass_pc));
    }
    out
}

/// The scale that drives `in_key`-aware resolution for a single
/// event. Mirrors `resolve_chord_spec_root`'s decision: functional
/// events with `in_key` use that scale; otherwise the fallback.
fn effective_scale_for_event(ev: &ChordEvent, fallback: &Scale) -> Scale {
    match &ev.chord {
        ChordSpec::Functional {
            in_key: Some(scale),
            ..
        } => scale.clone(),
        _ => fallback.clone(),
    }
}

fn resolve_bass(bass: &BassSpec, chord: &ChordSpec, scale: &Scale) -> Option<PitchClass> {
    match bass {
        BassSpec::Absolute(pc) => Some(*pc),
        BassSpec::Inversion(n) => {
            // 1st inversion → 3rd in bass, 2nd → 5th, 3rd → 7th.
            let step = match n {
                1 => ChordStep::Third,
                2 => ChordStep::Fifth,
                3 => ChordStep::Seventh,
                _ => return None,
            };
            let root = resolve_chord_spec_root(chord, scale);
            let suffix = match chord {
                ChordSpec::Functional { suffix, .. } | ChordSpec::Absolute { suffix, .. } => {
                    suffix
                }
            };
            resolve_chord_degree(
                rawdaw_model::chord::ChordDegree::new(step),
                root,
                suffix,
            )
        }
        BassSpec::ChordDegree(cd) => {
            let root = resolve_chord_spec_root(chord, scale);
            let suffix = match chord {
                ChordSpec::Functional { suffix, .. } | ChordSpec::Absolute { suffix, .. } => {
                    suffix
                }
            };
            resolve_chord_degree(*cd, root, suffix)
        }
        BassSpec::ScaleDegree(sd) => Some(resolve_scale_degree(*sd, scale)),
    }
}

fn effective_key_label(id: ChordLoopId) -> String {
    let app = use_store::<AppState>();
    let project = app.project.get();
    let Some(loop_) = project.chord_loops.get(&id) else {
        return String::new();
    };
    let (scale, source) = match &loop_.key {
        Some(s) => (s.clone(), "loop key"),
        None => (project.default_key.clone(), "project key"),
    };
    format!("{} {} · {source}", pitch_class_name(scale.tonic), mode_name(&scale))
}

fn mode_name(scale: &Scale) -> &'static str {
    use rawdaw_model::scale::Mode::*;
    match scale.mode {
        Ionian => "major",
        Aeolian => "minor",
        Dorian => "dorian",
        Phrygian => "phrygian",
        Lydian => "lydian",
        Mixolydian => "mixolydian",
        Locrian => "locrian",
        HarmonicMinor => "harmonic minor",
        MelodicMinor => "melodic minor",
        PhrygianDominant => "phrygian dominant",
        Lydian7 => "lydian dominant",
        Altered => "altered",
        MajorPentatonic => "major pentatonic",
        MinorPentatonic => "minor pentatonic",
        Blues => "blues",
        WholeTone => "whole tone",
        Chromatic => "chromatic",
        Custom { .. } => "custom",
    }
}

// ─── Styles ─────────────────────────────────────────────────────────────

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
        "font-size: 12px; font-variant-numeric: tabular-nums; \
         color: {text1}; \
         white-space: nowrap; overflow: hidden; text-overflow: ellipsis;",
        text1 = theme::TEXT1,
    )
}
