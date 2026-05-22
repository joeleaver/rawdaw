//! Inline pattern picker shared by `IdentityColumn` and `CellInherit`.
//!
//! Renders a `Select` whose options are the project's patterns plus a
//! `(no pattern)` sentinel. The currently-bound pattern is the initial
//! value (or `(no pattern)` for an unbound / inherit row). Selecting a
//! pattern routes through `pattern_actions::set_activation_pattern`,
//! which creates the activation entry if it doesn't exist yet.
//!
//! Used by both the Active cell's identity column (replaces the static
//! PatternCard's name span) and the CellInherit placeholder (replaces
//! the "+ Add activation" button). One picker primitive keeps the
//! bind/unbind semantics unified.

use rinch::prelude::*;

use rawdaw_model::id::{PatternId, SectionId, TrackId};

use crate::pattern_actions::set_activation_pattern;
use crate::state::{AppState, EditorMode};

const NO_PATTERN_VALUE: &str = "__none__";

/// Sentinel u64 for "no pattern bound." Picked as `u64::MAX` so we
/// don't collide with `PatternId(0)`, which is a valid first-allocated
/// pattern in fresh projects.
pub(super) const NO_PATTERN_SENTINEL: u64 = u64::MAX;

#[component]
pub(super) fn PatternSelect(
    section_id: SectionId,
    track_id: TrackId,
    /// Currently-bound pattern's id, or [`NO_PATTERN_SENTINEL`] for
    /// "(no pattern)". The `value` prop is fed through every render so
    /// a project edit from elsewhere reflects here.
    current_pattern_value: u64,
) -> NodeHandle {
    rsx! {
        for opts in pattern_options(current_pattern_value) {
            Select {
                key: opts.key.clone(),
                size: "sm",
                value: {opts.current_value.clone()},
                data: opts.options.clone(),
                onchange: move |v: String| {
                    commit_pattern_change(section_id, track_id, v);
                },
            }
        }
    }
}

#[derive(Clone, PartialEq, Default)]
struct PatternOptions {
    key: String,
    current_value: String,
    options: Vec<SelectOption>,
}

fn pattern_options(current_pattern_value: u64) -> Vec<PatternOptions> {
    let app = use_store::<AppState>();
    let project = app.project.get();
    let mut options = vec![SelectOption::new(
        NO_PATTERN_VALUE.to_string(),
        "(no pattern)".to_string(),
    )];
    // Sort patterns by name for stable dropdown order.
    let mut patterns: Vec<_> = project.patterns.values().collect();
    patterns.sort_by_key(|p| p.name.clone());
    for p in &patterns {
        options.push(SelectOption::new(p.id.get().to_string(), p.name.clone()));
    }
    let current_value = if current_pattern_value == NO_PATTERN_SENTINEL {
        NO_PATTERN_VALUE.to_string()
    } else {
        current_pattern_value.to_string()
    };
    // Key encodes the project's pattern set (count + ids) so adding /
    // removing patterns elsewhere remounts the Select with the fresh
    // option list.
    let patterns_key: String = patterns
        .iter()
        .map(|p| p.id.get().to_string())
        .collect::<Vec<_>>()
        .join("-");
    let key = format!("ps{}-{patterns_key}", current_value);
    vec![PatternOptions {
        key,
        current_value,
        options,
    }]
}

fn commit_pattern_change(section_id: SectionId, track_id: TrackId, value: String) {
    let app = use_store::<AppState>();
    let new_pattern: Option<PatternId> = if value == NO_PATTERN_VALUE {
        None
    } else {
        match value.parse::<u64>() {
            Ok(n) => Some(PatternId::new(n)),
            Err(_) => return,
        }
    };
    // Read the active variant from EditorMode at click time. Editing
    // the base tab routes the mutation through `section.base.activations`;
    // any other tab routes it through `section.variants[v].activations`
    // (see `pattern_actions::set_activation_pattern`).
    let variant_id = match app.editor_mode.get() {
        EditorMode::SectionEditor { variant, .. } => variant,
        EditorMode::Arrangement => return,
    };
    if let Err(e) = app.apply_project_edit(move |p| {
        set_activation_pattern(p, section_id, track_id, &variant_id, new_pattern);
    }) {
        eprintln!("section_editor: set activation pattern failed: {e}");
    }
}
