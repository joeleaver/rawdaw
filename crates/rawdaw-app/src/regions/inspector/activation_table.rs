//! Activation table inside the inspector. One row per global track,
//! showing which pattern is bound and the effective state (active /
//! silent / inherit) under the currently-selected variant.
//!
//! Extracted from the parent `inspector.rs` so the inspector module
//! stays under the ~700-line cap. The split is by concern — the
//! activation table has its own per-track render logic, the rest of the
//! inspector is header/tabs/form/footer plumbing.

use rinch::prelude::*;

use crate::fixture::{self, ActivationOverride, ActivationState};
use crate::parts::StatePill;
use crate::theme;

#[component]
pub fn ActivationTable(section_name_key: String, variant_id: String) -> NodeHandle {
    let r = fixture::round1();
    let outer_style = format!(
        "border-radius: 4px; overflow: hidden; \
         border: 1px solid {line}; background: {bg0};",
        line = theme::LINE, bg0 = theme::BG0,
    );
    let head_style = format!(
        "display: grid; grid-template-columns: 64px 1fr 60px; \
         padding: 5px 8px; gap: 8px; \
         font-size: 10px; color: rgba(232,234,238,0.42); \
         letter-spacing: 0.5px; text-transform: uppercase; \
         border-bottom: 1px solid {line}; background: {bg1};",
        line = theme::LINE, bg1 = theme::BG1,
    );

    rsx! {
        div { style: {outer_style.clone()},
            div { style: {head_style.clone()},
                div { "Track" }
                div { "Pattern" }
                div { style: "text-align: right;", "State" }
            }
            for track in r.tracks.iter().cloned() {
                ActivationRow {
                    key: track.id,
                    section_name_key: section_name_key.clone(),
                    variant_id: variant_id.clone(),
                    track_id: track.id.to_string(),
                    track_name: track.name.to_string(),
                    track_is_drum: track.kind == fixture::TrackKind::Drum,
                }
            }
        }
    }
}

/// Effective state for `track_id` in the named section under `variant_id`.
/// Returns `(pattern_name, state, overridden_from_base)`.
///
/// Variant overrides are walked first so a `Silent` or `Replace` entry
/// wins over the base; absence of an override means "inherit base"; a
/// missing base entry means "track has no activation in this section"
/// and renders as `Inherit` (the round-1 inspector treats this as a
/// quiet third pill).
fn effective_state(
    section_name_key: &str,
    variant_id: &str,
    track_id: &str,
) -> (Option<&'static str>, ActivationState, bool) {
    let r = fixture::round1();
    let Some(section) = fixture::section_by_key(&r, section_name_key) else {
        return (None, ActivationState::Inherit, false);
    };

    // Variant override takes precedence over base. The override list is
    // sparse — absence means "inherit base."
    for (vid, ov) in section.variant_overrides.iter() {
        if *vid != variant_id {
            continue;
        }
        for (tid, entry) in ov.iter() {
            if *tid != track_id {
                continue;
            }
            return match entry {
                ActivationOverride::Silent => {
                    let pat = section
                        .activations
                        .iter()
                        .find(|(t, _)| *t == track_id)
                        .map(|(_, a)| a.pattern);
                    (pat, ActivationState::Silent, true)
                }
                ActivationOverride::Replace(act) => (Some(act.pattern), act.state, true),
            };
        }
    }

    let Some(base) = section
        .activations
        .iter()
        .find(|(tid, _)| *tid == track_id)
        .map(|(_, a)| a)
    else {
        return (None, ActivationState::Inherit, false);
    };
    (Some(base.pattern), base.state, false)
}

#[component]
fn ActivationRow(
    section_name_key: String,
    variant_id: String,
    track_id: String,
    track_name: String,
    track_is_drum: bool,
) -> NodeHandle {
    let row_style = format!(
        "display: grid; grid-template-columns: 64px 1fr 60px; \
         padding: 6px 8px; gap: 8px; align-items: center; \
         border-bottom: 1px solid {line_soft}; font-size: 12px;",
        line_soft = theme::LINE_SOFT,
    );
    let track_dot_color = if track_is_drum {
        "rgba(232,234,238,0.42)"
    } else {
        "rgba(232,234,238,0.62)"
    };
    let track_dot_style = format!(
        "width: 6px; height: 6px; border-radius: 50%; \
         background: {track_dot_color}; opacity: 0.6;",
    );
    let track_cell_style = "color: rgba(232,234,238,0.96); \
         display: flex; align-items: center; gap: 5px;";

    let (pat_name_opt, state, overridden) =
        effective_state(section_name_key.as_str(), variant_id.as_str(), track_id.as_str());

    let r = fixture::round1();
    let pat_swatch_color = pat_name_opt
        .and_then(|n| fixture::pattern_by_name(&r, n))
        .map(|p| p.color.to_string())
        .unwrap_or_default();
    let has_swatch = !pat_swatch_color.is_empty();
    let pat_label = pat_name_opt.unwrap_or("—").to_string();
    let pat_cell_color = if pat_name_opt.is_some() {
        "rgba(232,234,238,0.62)"
    } else {
        "rgba(232,234,238,0.28)"
    };
    let pat_cell_style = format!(
        "color: {pat_cell_color}; display: flex; align-items: center; gap: 5px;",
    );
    // Render the swatch unconditionally; collapse visually when there's no
    // pattern so we don't have to thread an `Option` through rsx control flow
    // (which moves captured non-Copy bindings).
    let pat_swatch_style = if has_swatch {
        format!(
            "width: 7px; height: 7px; border-radius: 1.5px; \
             background: {pat_swatch_color}; flex: 0 0 auto;",
        )
    } else {
        "display: none;".to_string()
    };

    rsx! {
        div { style: {row_style.clone()},
            div { style: {track_cell_style.to_string()},
                span { style: {track_dot_style.clone()} }
                span { {track_name.clone()} }
            }
            div { style: {pat_cell_style.clone()},
                span { style: {pat_swatch_style.clone()} }
                span {
                    style: "overflow: hidden; text-overflow: ellipsis; white-space: nowrap;",
                    {pat_label.clone()}
                }
            }
            div { style: "text-align: right;",
                StatePill { state: state, overridden: overridden }
            }
        }
    }
}

