//! Three-column activation cell shell.
//!
//! Wraps `StripePaper` to get the pattern-color left stripe + neutral
//! body, then composes:
//!
//! - col 1 (320px): `IdentityColumn` — track row + pattern card + footer.
//! - col 2 (1fr):   `realization_placeholder` — Phase 5 lands the real
//!   voicing / octave / humanize controls.
//! - col 3 (520px): `schedule_placeholder` — Phase 6 lands the real
//!   `schedule_timeline` widget.
//!
//! The Silent state dims the whole cell to 78 % opacity (matches the
//! mockup). The outer wrapper carries the opacity rather than
//! `StripePaper` itself — keeping the primitive style-purist.

use rinch::prelude::*;

use crate::fixture::{ActivationState, Realization, TrackKind};
use crate::parts::StripePaper;
use crate::section_editor::cell::identity_column::IdentityColumn;
use crate::section_editor::cell::realization_column::RealizationColumn;
use crate::theme;

#[component]
pub fn ActivationCell(
    track_name: String,
    track_kind: TrackKind,
    track_role: String,
    pattern_name: String,
    pattern_color: String,
    pattern_kind: String,
    state: ActivationState,
    overridden_by_variant: bool,
    /// Empty string when no source label applies; non-empty produces
    /// the right-aligned `silenced in this variant` /
    /// `replaced in this variant` tag in the cell footer.
    source_label: String,
    /// `None` when the activation has no realization block (round-1
    /// fixture rows still go through this component for now). Phases
    /// 5 reads this in `RealizationColumn`.
    realization: Option<Realization>,
) -> NodeHandle {
    let opacity = if matches!(state, ActivationState::Silent) {
        "0.78"
    } else {
        "1"
    };
    let wrap_style = format!("opacity: {opacity};");
    let grid_style = "display: grid; grid-template-columns: 320px 1fr 520px; \
         align-items: stretch; min-height: 96px;"
        .to_string();
    let bg = theme::BG1.to_string();
    let radius = 6.0_f32;
    let stripe_color_for_identity = pattern_color.clone();
    let realization_track_role = track_role.clone();

    rsx! {
        div { style: {wrap_style.clone()},
            StripePaper {
                stripe_color: pattern_color,
                background: bg,
                padding: "0".to_string(),
                radius: radius,
                div { style: {grid_style.clone()},
                    IdentityColumn {
                        track_name: track_name,
                        track_kind: track_kind,
                        track_role: track_role,
                        pattern_name: pattern_name,
                        pattern_color: stripe_color_for_identity,
                        pattern_kind: pattern_kind,
                        state: state,
                        overridden_by_variant: overridden_by_variant,
                        source_label: source_label,
                    }
                    RealizationColumn {
                        realization: realization,
                        track_kind: track_kind,
                        track_role: realization_track_role,
                    }
                    SchedulePlaceholder { }
                }
            }
        }
    }
}

#[component]
fn SchedulePlaceholder() -> NodeHandle {
    let style = format!(
        "padding: 12px 14px; \
         display: flex; flex-direction: column; gap: 6px; \
         font-size: 11px; color: {text3}; font-style: italic;",
        text3 = theme::TEXT3,
    );
    rsx! {
        div { style: {style.clone()},
            div {
                style: "font-size: 10.5px; letter-spacing: 0.6px; \
                        text-transform: uppercase; color: rgba(232,234,238,0.42); \
                        font-weight: 600; font-style: normal;",
                "Variant schedule"
            }
            div { "per-bar variant timeline + legend" }
            div { "(filled in phase 6)" }
        }
    }
}
