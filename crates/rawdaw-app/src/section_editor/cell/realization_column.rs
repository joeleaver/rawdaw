//! Cell column 2 — realization parameters.
//!
//! Voicing + Octave dropdowns (pitched tracks only) followed by the
//! four-control Humanize row (velocity / timing / swing micro-sliders
//! plus a seed field). Drum tracks replace the voicing/octave rows
//! with "drums are pitch-symbolic — voicing & octave do not apply",
//! since drum patterns address parts by GM note and don't use
//! chord-derived voicing.
//!
//! ## Inheritance vs override (computed, not stored)
//!
//! Per the round-2 README port-time note (and `CLAUDE.md` rule 1), the
//! `↳ role default` inheritance tag and the `*` override mark are
//! computed at render time by comparing the activation's value
//! against `role_defaults[track.role]`. The fixture stores values
//! only; it does **not** carry `voicingFromRole` / `octaveFromRole`
//! flags (those were a display-only convenience in the JS mockup).
//! Equal → `InheritanceTag { source: RoleDefault }`. Different →
//! asterisk via the `overridden` arm of the dropdown row.

use rinch::prelude::*;

use crate::overlay::{
    self as fixture, Humanization, OctaveSpec, Realization, TrackKindTag as TrackKind, Voicing,
};
use crate::parts::{Icon, InheritanceSource, InheritanceTag};
use crate::theme;

#[component]
pub fn RealizationColumn(
    realization: Option<Realization>,
    track_kind: TrackKind,
    track_role: String,
) -> NodeHandle {
    let col_style = format!(
        "padding: 12px 14px; border-right: 1px solid {line}; \
         display: flex; flex-direction: column; gap: 8px;",
        line = theme::LINE,
    );

    // Resolve effective values + role defaults. The cell renders even
    // without an activation realization (Phase 1 fixture rows for the
    // intro section have `realization: None`) — in that case the
    // dropdowns show "—" and no inheritance tag fires.
    let role = fixture::role_defaults(track_role.as_str());
    let voicing_value = realization.and_then(|r| r.voicing);
    let octave_value = realization.and_then(|r| r.octave);
    let humanization_value = realization.map(|r| r.humanization);

    let is_drum = matches!(track_kind, TrackKind::Drum);

    rsx! {
        div { style: {col_style.clone()},
            SectionTitle { label: "Realization".to_string() }
            if !is_drum {
                VoicingRow {
                    value: voicing_value,
                    role_default: role.map(|r| r.voicing),
                }
            }
            if !is_drum {
                OctaveRow {
                    value: octave_value,
                    role_default: role.map(|r| r.octave),
                }
            }
            if is_drum {
                DrumNotice { }
            }
            HumanizeRow {
                humanization: humanization_value,
                role_default: role.map(|r| r.humanization),
            }
        }
    }
}

#[component]
fn SectionTitle(label: String) -> NodeHandle {
    let style = format!(
        "font-size: 10.5px; letter-spacing: 0.6px; text-transform: uppercase; \
         color: {text2}; font-weight: 600;",
        text2 = theme::TEXT2,
    );
    rsx! { div { style: {style.clone()}, {label.clone()} } }
}

#[component]
fn DrumNotice() -> NodeHandle {
    let style = format!(
        "font-size: 11px; color: {text3}; font-style: italic;",
        text3 = theme::TEXT3,
    );
    rsx! {
        div { style: {style.clone()},
            "drums are pitch-symbolic — voicing & octave do not apply"
        }
    }
}

// ─── Voicing & octave rows ────────────────────────────────────────────────

#[component]
fn VoicingRow(value: Option<Voicing>, role_default: Option<Voicing>) -> NodeHandle {
    let display = value
        .map(|v| v.label().to_string())
        .unwrap_or_else(|| "—".to_string());
    let matches_role = matches!((value, role_default), (Some(v), Some(d)) if v == d);
    let show_override = value.is_some() && role_default.is_some() && !matches_role;
    rsx! {
        DropdownRow {
            label: "Voicing".to_string(),
            value: display,
            inherited_from_role: matches_role,
            overridden: show_override,
        }
    }
}

#[component]
fn OctaveRow(value: Option<OctaveSpec>, role_default: Option<OctaveSpec>) -> NodeHandle {
    let display = value
        .map(|o| o.label().to_string())
        .unwrap_or_else(|| "—".to_string());
    let matches_role = matches!((value, role_default), (Some(v), Some(d)) if v == d);
    let show_override = value.is_some() && role_default.is_some() && !matches_role;
    rsx! {
        DropdownRow {
            label: "Octave".to_string(),
            value: display,
            inherited_from_role: matches_role,
            overridden: show_override,
        }
    }
}

#[component]
fn DropdownRow(
    label: String,
    value: String,
    inherited_from_role: bool,
    overridden: bool,
) -> NodeHandle {
    let row_style = "display: grid; grid-template-columns: 64px 1fr auto; \
         gap: 8px; align-items: center;"
        .to_string();
    let label_style = format!(
        "font-size: 11.5px; color: {text2};",
        text2 = theme::TEXT2,
    );
    let select_style = format!(
        "display: flex; align-items: center; gap: 6px; \
         padding: 4px 8px; border-radius: 4px; \
         background: {bg0}; border: 1px solid {line}; \
         font-size: 12px; color: {text0}; overflow: hidden;",
        bg0 = theme::BG0,
        line = theme::LINE,
        text0 = theme::TEXT0,
    );
    let value_style = "flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;";
    let chevron_stroke = theme::TEXT2.to_string();
    let trailing_style =
        "min-width: 92px; font-size: 10.5px; display: flex; justify-content: flex-end;";
    let inheritance_source = InheritanceSource::RoleDefault;
    let label_owned = label.clone();
    let value_owned = value.clone();

    rsx! {
        div { style: {row_style.clone()},
            span { style: {label_style.clone()}, {label_owned.clone()} }
            div { style: {select_style.clone()},
                span { style: {value_style.to_string()}, {value_owned.clone()} }
                Icon { glyph: "chevron-d", size: 11.0,
                       stroke: {chevron_stroke.clone()}, stroke_width: 1.6 }
            }
            div { style: {trailing_style.to_string()},
                if inherited_from_role {
                    InheritanceTag { source: inheritance_source }
                }
                if overridden {
                    OverrideMark { }
                }
            }
        }
    }
}

#[component]
fn OverrideMark() -> NodeHandle {
    let style = format!(
        "color: {text2}; cursor: help; padding: 0 4px; \
         border: 1px solid {line_soft}; border-radius: 2px; \
         font-size: 10.5px; letter-spacing: 0.2px;",
        text2 = theme::TEXT2,
        line_soft = theme::LINE_SOFT,
    );
    rsx! {
        span { style: {style.clone()},
            title: "overrides the track role's default", "*"
        }
    }
}

// ─── Humanize row ─────────────────────────────────────────────────────────

#[component]
fn HumanizeRow(
    humanization: Option<Humanization>,
    role_default: Option<Humanization>,
) -> NodeHandle {
    let row_style = "display: grid; grid-template-columns: 64px 1fr; gap: 8px; \
         align-items: start; margin-top: 4px;"
        .to_string();
    let label_style = format!(
        "font-size: 11.5px; color: {text2}; padding-top: 5px;",
        text2 = theme::TEXT2,
    );
    let controls_style =
        "display: flex; flex-wrap: wrap; gap: 10px; align-items: center;";

    // Role-default is currently informational only — phases 5+ surface
    // it inline if a humanize control diverges. For now we just keep
    // the param threaded through so future inheritance logic doesn't
    // re-walk the role table.
    let _ = role_default;

    let (vel_text, vel_fill, tim_text, tim_fill, swing_text, swing_fill, seed_text) =
        match humanization {
            Some(h) => humanize_displays(h),
            None => (
                "—".to_string(),
                0.0_f32,
                "—".to_string(),
                0.0_f32,
                "—".to_string(),
                0.0_f32,
                String::new(),
            ),
        };

    rsx! {
        div { style: {row_style.clone()},
            span { style: {label_style.clone()}, "Humanize" }
            div { style: {controls_style.to_string()},
                MicroSlider { label: "vel".to_string(),
                              value: vel_text, fill: vel_fill }
                MicroSlider { label: "tim".to_string(),
                              value: tim_text, fill: tim_fill }
                MicroSlider { label: "swing".to_string(),
                              value: swing_text, fill: swing_fill }
                SeedField { seed_text: seed_text }
            }
        }
    }
}

/// Format the four humanize-control displays. Mirrors the JS mockup's
/// formulae:
/// - velocity: `{round(vel * 100)}%`, fill = `vel / 0.20`
/// - timing:   `{timing}t`,            fill = `timing / 16`
/// - swing:    `straight` if 0 else `{round(swing*100)}%`, fill = `swing / 0.5`
/// - seed:     decimal string of the `u64` seed
///
/// `fill` is clamped to `[0.02, 1.0]` so the bar is always visible.
fn humanize_displays(h: Humanization) -> (String, f32, String, f32, String, f32, String) {
    let vel_text = format!("{}%", (h.velocity * 100.0).round() as i32);
    let vel_fill = clamp_fill(h.velocity / 0.20);
    let tim_text = format!("{}t", h.timing);
    let tim_fill = clamp_fill(h.timing as f32 / 16.0);
    let swing_text = if h.swing == 0.0 {
        "straight".to_string()
    } else {
        format!("{}%", (h.swing * 100.0).round() as i32)
    };
    let swing_fill = clamp_fill(h.swing / 0.5);
    let seed_text = h.seed.to_string();
    (vel_text, vel_fill, tim_text, tim_fill, swing_text, swing_fill, seed_text)
}

fn clamp_fill(f: f32) -> f32 {
    f.clamp(0.02, 1.0)
}

#[component]
fn MicroSlider(label: String, value: String, fill: f32) -> NodeHandle {
    let w_px = 64;
    let outer_style = "display: inline-flex; flex-direction: column; gap: 2px;";
    let header_style = format!(
        "display: flex; align-items: baseline; justify-content: space-between; \
         font-size: 10px; color: {text2}; gap: 6px; line-height: 1;",
        text2 = theme::TEXT2,
    );
    let label_style = "letter-spacing: 0.3px;";
    let value_style = format!(
        "color: {text0}; font-feature-settings: \"tnum\" 1; \
         font-variant-numeric: tabular-nums;",
        text0 = theme::TEXT0,
    );
    let track_style = format!(
        "width: {w}px; height: 4px; border-radius: 2px; \
         background: {bg0}; border: 1px solid {line}; \
         position: relative; overflow: hidden;",
        w = w_px,
        bg0 = theme::BG0,
        line = theme::LINE,
    );
    let fill_pct = (fill * 100.0).clamp(2.0, 100.0);
    let fill_style = format!(
        "position: absolute; inset: 0; width: {pct}%; \
         background: rgba(213,200,166,0.55);",
        pct = fill_pct,
    );
    let label_owned = label.clone();
    let value_owned = value.clone();

    rsx! {
        div { style: {outer_style.to_string()},
            div { style: {header_style.clone()},
                span { style: {label_style.to_string()}, {label_owned.clone()} }
                span { style: {value_style.clone()}, {value_owned.clone()} }
            }
            div { style: {track_style.clone()},
                div { style: {fill_style.clone()} }
            }
        }
    }
}

#[component]
fn SeedField(seed_text: String) -> NodeHandle {
    let outer_style = "display: inline-flex; flex-direction: column; gap: 2px;";
    let header_style = format!(
        "font-size: 10px; color: {text2}; letter-spacing: 0.3px; line-height: 1;",
        text2 = theme::TEXT2,
    );
    let body_style = format!(
        "display: inline-flex; align-items: center; gap: 4px; \
         padding: 2px 4px 2px 6px; border-radius: 3px; \
         background: {bg0}; border: 1px solid {line}; \
         font-size: 11px; color: {text0}; \
         font-feature-settings: \"tnum\" 1; \
         font-variant-numeric: tabular-nums;",
        bg0 = theme::BG0,
        line = theme::LINE,
        text0 = theme::TEXT0,
    );
    // The mockup includes a "re-roll seed" affordance — port-time we
    // ship a placeholder gear icon; click handler arrives with the
    // model-bind milestone (round-3 or later).
    let seed_owned = seed_text.clone();
    let display = if seed_owned.is_empty() {
        "—".to_string()
    } else {
        seed_owned
    };

    rsx! {
        div { style: {outer_style.to_string()},
            div { style: {header_style.clone()}, "seed" }
            div { style: {body_style.clone()},
                span { {display.clone()} }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn realization(voicing: Voicing, octave: OctaveSpec) -> Realization {
        Realization {
            voicing: Some(voicing),
            octave: Some(octave),
            humanization: Humanization::default(),
        }
    }

    #[test]
    fn melodic_role_triad_close_matches_default() {
        let role = fixture::role_defaults("melodic").expect("melodic role exists");
        assert_eq!(role.voicing, Voicing::TriadClose);
        let r = realization(Voicing::TriadClose, OctaveSpec::Nearest);
        let matches = matches!((r.voicing, Some(role.voicing)), (Some(a), Some(b)) if a == b);
        assert!(matches, "lead@verse@base should match role default voicing");
    }

    #[test]
    fn pad_role_drop2_overrides_default() {
        // The role default is triad-open; the chorus fixture overrides
        // the pad's voicing to drop2 (override → `*` mark).
        let role = fixture::role_defaults("pad").expect("pad role exists");
        assert_eq!(role.voicing, Voicing::TriadOpen);
        let r = realization(Voicing::Drop2, OctaveSpec::Anchored(3));
        let matches = matches!((r.voicing, Some(role.voicing)), (Some(a), Some(b)) if a == b);
        assert!(!matches, "pad@chorus should NOT match the role default");
    }

    #[test]
    fn melodic_role_anchored_4_overrides_default_octave() {
        // The role default octave is Nearest; lead@verse@base sets it
        // to Anchored(4) — should render with `*`.
        let role = fixture::role_defaults("melodic").expect("melodic role exists");
        assert_eq!(role.octave, OctaveSpec::Nearest);
        let r = realization(Voicing::TriadClose, OctaveSpec::Anchored(4));
        let matches = matches!((r.octave, Some(role.octave)), (Some(a), Some(b)) if a == b);
        assert!(!matches, "lead@verse@base octave should NOT match role default");
    }

    #[test]
    fn humanize_displays_format_velocity_and_swing() {
        let h = Humanization { velocity: 0.06, timing: 5, swing: 0.0, seed: 1742 };
        let (vel, _vf, tim, _tf, swing, _sf, seed) = humanize_displays(h);
        assert_eq!(vel, "6%");
        assert_eq!(tim, "5t");
        assert_eq!(swing, "straight");
        assert_eq!(seed, "1742");
    }

    #[test]
    fn humanize_displays_round_velocity_to_integer_percent() {
        let h = Humanization { velocity: 0.045, timing: 0, swing: 0.25, seed: 0 };
        let (vel, _vf, _tim, _tf, swing, _sf, _seed) = humanize_displays(h);
        assert_eq!(vel, "5%"); // 0.045 * 100 = 4.5 → rounds to 5
        assert_eq!(swing, "25%");
    }

    #[test]
    fn fill_is_clamped_to_visible_range() {
        // Zero fill clamps up so the bar is always visible.
        assert_eq!(clamp_fill(0.0), 0.02);
        // Over-1 fill clamps down.
        assert_eq!(clamp_fill(1.5), 1.0);
        // In-range values pass through.
        assert_eq!(clamp_fill(0.3), 0.3);
    }

}
