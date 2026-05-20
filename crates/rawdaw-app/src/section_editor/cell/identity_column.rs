//! Cell column 1 — identity + pattern + footer.
//!
//! Renders three rows stacked vertically:
//!
//! 1. **Track row** — kind icon, track name, kind tag, optional
//!    `role: X` pill, and the state pill at the right edge.
//! 2. **Pattern card** — color swatch + pattern name + kind hint +
//!    `Open in pattern editor (round 3)` chevron button.
//! 3. **Footer** — pinned-overrides chip ("`N pinned`" or italic
//!    "no pinned notes") on the left; optional `silenced in this
//!    variant` / `replaced in this variant` tag aligned right.
//!
//! Per round-2 README decision 23, the pinned chip is the only place
//! the warm-accent color appears on a cell — plumbed through
//! `theme::ACCENT`.

use rinch::prelude::*;

use crate::overlay::{ActivationState, TrackKindTag as TrackKind};
use crate::parts::{rgba, Icon, StatePill};
use crate::theme;

#[component]
pub fn IdentityColumn(
    track_name: String,
    track_kind: TrackKind,
    track_role: String,
    pattern_name: String,
    pattern_color: String,
    pattern_kind: String,
    state: ActivationState,
    overridden_by_variant: bool,
    source_label: String,
) -> NodeHandle {
    let has_pattern = !pattern_name.is_empty();
    let col_bg = if has_pattern {
        rgba(pattern_color.as_str(), 0.04)
    } else {
        "transparent".to_string()
    };
    // Pinned-overrides count. Threaded into IdentityColumn as a fixed
    // 0 today; Phase 5/6 will plumb `Activation.per_note_overrides`
    // through the resolved cell slot. Local `let` binding sidesteps
    // the rsx macro's auto-Option-wrap behavior on bare integer
    // literals (`pinned: 0u32` resolves as `Option<u32>` instead of
    // `u32` — see also `crate::parts::Icon` call sites for the same
    // workaround).
    let pinned_count: u32 = 0;
    let col_style = format!(
        "padding: 12px 14px; border-right: 1px solid {line}; \
         display: flex; flex-direction: column; gap: 10px; \
         background: {bg};",
        line = theme::LINE,
        bg = col_bg,
    );

    rsx! {
        div { style: {col_style.clone()},
            TrackRow {
                track_name: track_name,
                track_kind: track_kind,
                track_role: track_role,
                state: state,
                overridden_by_variant: overridden_by_variant,
            }
            if has_pattern {
                PatternCard {
                    pattern_name: pattern_name.clone(),
                    pattern_color: pattern_color.clone(),
                    pattern_kind: pattern_kind.clone(),
                }
            }
            CellFooter {
                pinned: pinned_count,
                source_label: source_label,
            }
        }
    }
}

#[component]
fn TrackRow(
    track_name: String,
    track_kind: TrackKind,
    track_role: String,
    state: ActivationState,
    overridden_by_variant: bool,
) -> NodeHandle {
    let row_style = "display: flex; align-items: center; gap: 8px; flex-wrap: wrap;";
    let name_style = format!(
        "font-size: 13.5px; font-weight: 600; color: {text0}; letter-spacing: -0.1px;",
        text0 = theme::TEXT0,
    );
    let kind_label = match track_kind {
        TrackKind::Pitched => "PITCHED",
        TrackKind::Drum => "DRUM",
    };
    let kind_style = format!(
        "font-size: 10px; color: {text3}; letter-spacing: 0.4px;",
        text3 = theme::TEXT3,
    );
    let glyph = match track_kind {
        TrackKind::Drum => "pattern".to_string(),
        TrackKind::Pitched => "section".to_string(),
    };
    let icon_stroke = theme::TEXT1.to_string();
    let icon_size = 14.0_f32;
    let icon_w = 1.6_f32;
    let kind_label_owned = kind_label.to_string();
    let track_name_owned = track_name.clone();
    let show_role = !track_role.is_empty() && track_role != "—";
    let role_label = format!("role: {track_role}");

    rsx! {
        div { style: {row_style.to_string()},
            Icon { glyph: glyph, size: icon_size, stroke: {icon_stroke.clone()}, stroke_width: icon_w }
            span { style: {name_style.clone()}, {track_name_owned.clone()} }
            span { style: {kind_style.clone()}, {kind_label_owned.clone()} }
            if show_role {
                RolePill { label: role_label.clone() }
            }
            span { style: "flex: 1;" }
            StatePill { state: state, overridden: overridden_by_variant }
        }
    }
}

#[component]
fn RolePill(label: String) -> NodeHandle {
    let style = format!(
        "display: inline-flex; align-items: center; height: 16px; \
         padding: 0 5px; border-radius: 3px; \
         font-size: 10px; color: {text2}; \
         background: {bg0}; border: 1px solid {line};",
        text2 = theme::TEXT2,
        bg0 = theme::BG0,
        line = theme::LINE,
    );
    rsx! { span { style: {style.clone()}, {label.clone()} } }
}

#[component]
fn PatternCard(pattern_name: String, pattern_color: String, pattern_kind: String) -> NodeHandle {
    let card_style = format!(
        "display: flex; align-items: center; gap: 8px; \
         padding: 6px 8px; border-radius: 4px; \
         background: {bg0}; border: 1px solid {line};",
        bg0 = theme::BG0,
        line = theme::LINE,
    );
    let swatch_style = format!(
        "width: 9px; height: 9px; border-radius: 2px; \
         background: {col}; border: 1px solid {border_col}; flex: 0 0 auto;",
        col = pattern_color,
        border_col = rgba(pattern_color.as_str(), 0.60),
    );
    let name_style = format!(
        "font-size: 12.5px; font-weight: 500; color: {text0};",
        text0 = theme::TEXT0,
    );
    let kind_style = format!("font-size: 11px; color: {text3};", text3 = theme::TEXT3);
    let kind_hint = format!("· {pattern_kind}");
    let btn_style = format!(
        "width: 20px; height: 20px; padding: 0; \
         background: transparent; border: 1px solid {line}; \
         border-radius: 3px; cursor: pointer; \
         display: inline-flex; align-items: center; justify-content: center; \
         font-family: inherit; color: {text2};",
        line = theme::LINE,
        text2 = theme::TEXT2,
    );
    let chevron_stroke = theme::TEXT2.to_string();
    let chevron_size = 13.0_f32;
    let chevron_w = 1.6_f32;

    rsx! {
        div { style: {card_style.clone()},
            span { style: {swatch_style.clone()} }
            span { style: {name_style.clone()}, {pattern_name.clone()} }
            span { style: {kind_style.clone()}, {kind_hint.clone()} }
            span { style: "flex: 1;" }
            button {
                r#type: "button", style: {btn_style.clone()},
                title: "Open in pattern editor (round 3)",
                Icon { glyph: "chevron-r", size: chevron_size,
                       stroke: {chevron_stroke.clone()}, stroke_width: chevron_w }
            }
        }
    }
}

#[component]
fn CellFooter(pinned: u32, source_label: String) -> NodeHandle {
    let row_style = format!(
        "display: flex; align-items: center; gap: 10px; \
         margin-top: auto; font-size: 11px; color: {text2};",
        text2 = theme::TEXT2,
    );
    let has_label = !source_label.is_empty();
    let has_pinned = pinned > 0;

    rsx! {
        div { style: {row_style.clone()},
            if has_pinned {
                PinnedChip { count: pinned }
            }
            if !has_pinned {
                NoPinnedHint { }
            }
            if has_label {
                SourceLabelTag { label: source_label.clone() }
            }
        }
    }
}

#[component]
fn PinnedChip(count: u32) -> NodeHandle {
    // The warm-accent color (theme::ACCENT) is the ONLY accent color
    // used on a cell per round-2 README decision 23. Don't add other
    // accent colors here.
    let style = format!(
        "display: inline-flex; align-items: center; gap: 5px; \
         padding: 3px 8px; border-radius: 11px; \
         background: {bg}; border: 1px solid {border}; \
         color: {acc}; cursor: pointer; \
         font-size: 11px; font-weight: 500; font-family: inherit;",
        bg = rgba(theme::ACCENT, 0.10),
        border = rgba(theme::ACCENT, 0.35),
        acc = theme::ACCENT,
    );
    let dot_stroke = theme::ACCENT.to_string();
    let chev_stroke = theme::ACCENT.to_string();
    let dot_size = 7.0_f32;
    let chev_size = 11.0_f32;
    let sw = 1.6_f32;
    let label = format!("{count} pinned");

    rsx! {
        button {
            r#type: "button", style: {style.clone()},
            title: "Open piano roll on pinned notes",
            Icon { glyph: "dot", size: dot_size, stroke: {dot_stroke.clone()}, stroke_width: sw }
            span { {label.clone()} }
            Icon { glyph: "chevron-r", size: chev_size, stroke: {chev_stroke.clone()}, stroke_width: sw }
        }
    }
}

#[component]
fn NoPinnedHint() -> NodeHandle {
    let style = format!(
        "color: {text3}; font-style: italic; font-size: 11px;",
        text3 = theme::TEXT3,
    );
    rsx! {
        span { style: {style.clone()}, "no pinned notes" }
    }
}

#[component]
fn SourceLabelTag(label: String) -> NodeHandle {
    let style = format!(
        "margin-left: auto; font-size: 10.5px; color: {text2}; \
         padding: 1px 6px; border-radius: 2px; \
         border: 1px solid {line_soft}; letter-spacing: 0.2px;",
        text2 = theme::TEXT2,
        line_soft = theme::LINE_SOFT,
    );
    rsx! {
        span { style: {style.clone()},
            title: "see Variants tab",
            {label.clone()}
        }
    }
}
