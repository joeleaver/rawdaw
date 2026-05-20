//! Quick-entry shorthand text input for the focused chord event.
//!
//! Bidirectional binding with the dropdown set:
//!
//! - **Up** (typing → model): user types in the input; on `Enter`
//!   the buffer is passed through [`crate::chord_shorthand::parse`]
//!   and committed through `mutate_event` if it parses. Parse
//!   failures leave the buffer alone so the user can fix the
//!   typo — the dropdowns still reflect the previous (valid) state.
//! - **Down** (model → typing): an [`Effect`] subscribes to the
//!   store, re-formats the chord via [`crate::chord_shorthand::format`],
//!   and writes back to the buffer when it diverges from the
//!   canonical form. The Effect reads the buffer through
//!   [`untracked`] so user typing isn't re-firing the Effect
//!   in a feedback loop — only model-side changes (dropdown
//!   edits, undo, file load) push down.
//!
//! Established by C4's TopBar name/BPM controls; same shape here.

use rinch::core::reactive::{untracked, Effect};
use rinch::prelude::*;

use rawdaw_model::id::ChordLoopId;

use crate::chord_shorthand::{format, parse};
use crate::state::AppState;

use super::{fetch_event, mutate_event};

#[component]
pub(super) fn ShorthandInput(id: ChordLoopId, idx: usize) -> NodeHandle {
    let buffer = Signal::new(current_canonical(id, idx));

    let _ = Effect::new(move || {
        let canonical = current_canonical(id, idx);
        let current = untracked(|| buffer.get());
        if current == canonical {
            return;
        }
        buffer.set(canonical);
    });

    rsx! {
        TextInput {
            size: "sm",
            placeholder: "shorthand…",
            value_fn: move || buffer.get(),
            oninput: move |v: String| buffer.set(v),
            onsubmit: move || commit_shorthand(id, idx, buffer.get()),
        }
    }
}

/// Read the focused chord + bass and render through the formatter.
/// Returns an empty string if the event has gone away (focus race).
fn current_canonical(id: ChordLoopId, idx: usize) -> String {
    let key = use_store::<AppState>().project.get().default_key.clone();
    fetch_event(id, idx)
        .map(|ev| format(&ev.chord, ev.bass.as_ref(), &key))
        .unwrap_or_default()
}

/// Parse the typed shorthand and commit if valid. On parse failure
/// the input keeps its current contents so the user can fix the
/// typo; dropdowns continue to reflect the prior (valid) state.
fn commit_shorthand(id: ChordLoopId, idx: usize, raw: String) {
    let key = use_store::<AppState>().project.get().default_key.clone();
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return;
    }
    let parsed = match parse(trimmed, &key) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("chord shorthand parse failed: {} (input: `{trimmed}`)", e.message);
            return;
        }
    };
    mutate_event(id, idx, move |ev| {
        ev.chord = parsed.chord.clone();
        ev.bass = parsed.bass.clone();
    });
}
