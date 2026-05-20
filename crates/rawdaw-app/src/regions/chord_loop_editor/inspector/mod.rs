//! Per-event field editor pane.
//!
//! Reads `AppState::focused_chord_event_idx` and renders a stack of
//! editors for the focused event's harmony fields. Edits commit
//! through the C2 edit pump so realization re-runs and the audio
//! engine re-arms in lockstep with the host signal swap.
//!
//! ## File layout (split during CL2 — mod.rs originally cleared
//!   the 700-line cap, so the per-field code moved into submodules
//!   while the composition shell + shared helpers stayed here)
//!
//! - `mod.rs` (this file): `Inspector` + `FocusedFields` +
//!   `ChordSpecKind` decomposer + `fetch_event` / `mutate_event`
//!   + the `current_*_str` value-fn dispatchers.
//! - `roman_quality.rs`: RomanDegree + ChordQuality dropdowns +
//!   their encode/decode helpers + tests.
//! - `chips.rs`: Extension + Alteration multi-select chip rows +
//!   CSV encoding helpers + tests.
//! - `bass.rs`: BassSpec kind dropdown + value sub-editors
//!   (Inversion / Absolute fully supported; ChordDegree +
//!   ScaleDegree defer to CL3).
//! - `annotation.rs`: `in_key` borrowed-key dropdown + cadence
//!   tag dropdown + comment text input.

use rinch::prelude::*;

use rawdaw_model::chord::{ChordEvent, ChordQuality, ChordSpec, RomanDegree};
use rawdaw_model::id::ChordLoopId;
use rawdaw_model::scale::Scale;

use crate::state::AppState;
use crate::theme;

mod annotation;
mod bass;
mod chips;
mod roman_quality;
mod shorthand_input;

use annotation::{
    cadence_options, commit_cadence, commit_in_key, encode_cadence_opt, encode_scale,
    in_key_options, CommentInput,
};
use bass::BassEditor;
use chips::{AlterationChips, ExtensionChips};
use roman_quality::{
    commit_absolute_root, commit_mode, commit_quality, commit_roman, current_absolute_root_str,
    current_mode_str, encode_quality, encode_roman, mode_options, pitch_class_options,
    quality_options, roman_options,
};
use shorthand_input::ShorthandInput;

/// Inspector pane. Reads `AppState::focused_chord_event_idx`
/// reactively; the outer match remounts on focus change so the
/// inner editors always see a fresh `(id, idx)` pair.
#[component]
pub(crate) fn Inspector(id: ChordLoopId) -> NodeHandle {
    let pane_style = format!(
        "width: 320px; flex: 0 0 320px; \
         background: {bg1}; border-left: 1px solid {line}; \
         display: flex; flex-direction: column; \
         padding: 16px; gap: 12px; overflow-y: auto;",
        bg1 = theme::BG1,
        line = theme::LINE,
    );

    rsx! {
        aside { style: {pane_style.clone()},
            // `_focused` underscore prefix silences the
            // unused-variable lint — the rsx macro hides the
            // inner `idx: _focused` prop use from rustc's lint
            // pass, but `_`-prefixed names remain usable in Rust.
            if let Some(_focused) =
                use_store::<AppState>().focused_chord_event_idx.get()
            {
                FocusedFields { id: id, idx: _focused }
            } else {
                EmptyState { }
            }
        }
    }
}

#[component]
fn EmptyState() -> NodeHandle {
    let style = format!(
        "padding: 24px 12px; text-align: center; \
         color: {text2}; font-size: 12px; line-height: 1.5;",
        text2 = theme::TEXT2,
    );
    rsx! {
        div { style: {style.clone()},
            "No chord selected. Click a chord on the timeline or use + Chord to add one."
        }
    }
}

/// The per-event editor stack. Fetches the live event once at
/// render time; the surrounding `if let` in [`Inspector`]
/// remounts us whenever the focused index changes, so the rsx
/// tree always renders against an up-to-date snapshot.
#[component]
fn FocusedFields(id: ChordLoopId, idx: usize) -> NodeHandle {
    let Some(event) = fetch_event(id, idx) else {
        return rsx! { EmptyState { } };
    };

    let chord_kind = ChordSpecKind::from(&event.chord);
    let is_functional = chord_kind.is_functional;
    let comment_seed = event
        .annotation
        .as_ref()
        .and_then(|a| a.comment.clone())
        .unwrap_or_default();

    rsx! {
        div { style: "display: flex; flex-direction: column; gap: 14px;",
            div { style: {field_group_style()},
                span { style: {field_label_style()}, "Shorthand" }
                ShorthandInput { id: id, idx: idx }
            }
            div { style: {field_group_style()},
                span { style: {field_label_style()}, "Mode" }
                Select {
                    size: "sm",
                    value_fn: move || current_mode_str(id, idx),
                    data: mode_options(),
                    onchange: move |v: String| commit_mode(id, idx, v),
                }
            }
            div { style: {field_group_style()},
                span { style: {field_label_style()},
                    if is_functional { "Roman degree" } else { "Root pitch" }
                }
                if is_functional {
                    Select {
                        size: "sm",
                        value_fn: move || current_roman_str(id, idx),
                        data: roman_options(),
                        onchange: move |v: String| commit_roman(id, idx, v),
                    }
                } else {
                    Select {
                        size: "sm",
                        value_fn: move || current_absolute_root_str(id, idx),
                        data: pitch_class_options(),
                        onchange: move |v: String| commit_absolute_root(id, idx, v),
                    }
                }
            }
            div { style: {field_group_style()},
                span { style: {field_label_style()}, "Quality" }
                Select {
                    size: "sm",
                    value_fn: move || current_quality_str(id, idx),
                    data: quality_options(),
                    onchange: move |v: String| commit_quality(id, idx, v),
                }
            }
            div { style: {field_group_style()},
                span { style: {field_label_style()}, "Extensions" }
                ExtensionChips { id: id, idx: idx }
            }
            div { style: {field_group_style()},
                span { style: {field_label_style()}, "Alterations" }
                AlterationChips { id: id, idx: idx }
            }
            div { style: {field_group_style()},
                span { style: {field_label_style()}, "Bass" }
                BassEditor { id: id, idx: idx }
            }
            if is_functional {
                div { style: {field_group_style()},
                    span { style: {field_label_style()}, "Borrowed key (in_key)" }
                    Select {
                        size: "sm",
                        value_fn: move || current_in_key_str(id, idx),
                        data: in_key_options(),
                        onchange: move |v: String| commit_in_key(id, idx, v),
                    }
                }
            }
            div { style: {field_group_style()},
                span { style: {field_label_style()}, "Cadence" }
                Select {
                    size: "sm",
                    value_fn: move || current_cadence_str(id, idx),
                    data: cadence_options(),
                    onchange: move |v: String| commit_cadence(id, idx, v),
                }
            }
            div { style: {field_group_style()},
                span { style: {field_label_style()}, "Comment" }
                CommentInput { id: id, idx: idx, seed: comment_seed }
            }
        }
    }
}

fn field_group_style() -> String {
    "display: flex; flex-direction: column; gap: 4px;".into()
}

fn field_label_style() -> String {
    "font-size: 10px; letter-spacing: 0.6px; text-transform: uppercase; \
     color: rgba(232,234,238,0.42); font-weight: 600;"
        .into()
}

// ─── Reactive value-fn helpers ───────────────────────────────────────────

// Each fetches the live event and returns the canonical String the
// corresponding `Select` shows. Defined as free functions so the rsx
// `move ||` closures capture only the `Copy` (id, idx) pair rather
// than an outer `String` (which the macro's Fn-closure capture rules
// would force a clone-per-call otherwise).

fn current_roman_str(id: ChordLoopId, idx: usize) -> String {
    fetch_event(id, idx)
        .map(|ev| ChordSpecKind::from(&ev.chord).roman_string())
        .unwrap_or_else(|| "I".into())
}

fn current_quality_str(id: ChordLoopId, idx: usize) -> String {
    fetch_event(id, idx)
        .map(|ev| ChordSpecKind::from(&ev.chord).quality_string())
        .unwrap_or_else(|| "Major".into())
}

fn current_in_key_str(id: ChordLoopId, idx: usize) -> String {
    fetch_event(id, idx)
        .map(|ev| ChordSpecKind::from(&ev.chord).in_key_string())
        .unwrap_or_else(|| "none".into())
}

fn current_cadence_str(id: ChordLoopId, idx: usize) -> String {
    encode_cadence_opt(
        fetch_event(id, idx)
            .and_then(|ev| ev.annotation.as_ref().and_then(|a| a.cadence)),
    )
}

// ─── Event fetch + mutation ──────────────────────────────────────────────

pub(super) fn fetch_event(id: ChordLoopId, idx: usize) -> Option<ChordEvent> {
    let app = use_store::<AppState>();
    let project = app.project.get();
    let loop_ = project.chord_loops.get(&id)?;
    loop_.events.get(idx).cloned()
}

/// Mutate the focused chord event through the C2 edit pump.
pub(super) fn mutate_event<F>(id: ChordLoopId, idx: usize, f: F)
where
    F: FnOnce(&mut ChordEvent) + 'static,
{
    let app = use_store::<AppState>();
    if let Err(e) = app.apply_project_edit(move |p| {
        if let Some(loop_) = p.chord_loops.get_mut(&id)
            && let Some(event) = loop_.events.get_mut(idx)
        {
            f(event);
        }
    }) {
        eprintln!("chord_loop_editor: event mutation failed: {e}");
    }
}

// ─── Chord-spec decomposer ───────────────────────────────────────────────

/// Pull each editable field out of a `ChordSpec` so the editor
/// surface can render against owned values regardless of which
/// `ChordSpec` variant the event carries. Absolute events fall
/// back to "I" for the (unused) Roman degree and `None` for
/// `in_key` — the Roman + in_key dropdowns are hidden when
/// `is_functional` is false anyway.
#[derive(Clone)]
struct ChordSpecKind {
    is_functional: bool,
    roman: Option<RomanDegree>,
    quality: ChordQuality,
    in_key: Option<Scale>,
}

impl ChordSpecKind {
    fn from(spec: &ChordSpec) -> Self {
        match spec {
            ChordSpec::Functional { roman, suffix, in_key } => Self {
                is_functional: true,
                roman: Some(*roman),
                quality: suffix.quality.clone(),
                in_key: in_key.clone(),
            },
            ChordSpec::Absolute { suffix, .. } => Self {
                is_functional: false,
                roman: None,
                quality: suffix.quality.clone(),
                in_key: None,
            },
        }
    }

    fn roman_string(&self) -> String {
        self.roman.map(encode_roman).unwrap_or_else(|| "I".into())
    }

    fn quality_string(&self) -> String {
        encode_quality(&self.quality)
    }

    fn in_key_string(&self) -> String {
        match &self.in_key {
            None => "none".into(),
            Some(s) => encode_scale(s),
        }
    }
}
