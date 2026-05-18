//! Shared visual primitives across regions.
//!
//! Two kinds of helpers live here:
//!
//! - Pure functions (no `rsx!`) like [`rgba`] that any module can call.
//! - `#[component]` PascalCase functions like [`Icon`] callable from
//!   `rsx!` as `Icon { glyph: "...", ... }`.
//!
//! Plain `fn foo() -> NodeHandle { rsx! { ... } }` does NOT work in
//! Rinch — `rsx!` expects a `__scope` binding that only `#[component]`
//! injects. Use `#[component]` for any rsx-producing helper.

#![allow(dead_code)] // glyph set + helpers accrete; not every one is consumed yet

use rinch::prelude::*;

use crate::fixture::ActivationState;

/// Convert a `#RRGGBB` literal to an `rgba(r,g,b,a)` CSS string.
///
/// Panics in debug if `hex` is not a 7-char `#RRGGBB` value. The design
/// tokens are all 6-char hex by convention; we don't accept `#RGB` or
/// `#RRGGBBAA`.
pub fn rgba(hex: &str, a: f32) -> String {
    let bytes = hex.as_bytes();
    debug_assert!(
        bytes.len() == 7 && bytes[0] == b'#',
        "expected #RRGGBB, got {hex}",
    );
    let r = u8::from_str_radix(&hex[1..3], 16).unwrap_or(0);
    let g = u8::from_str_radix(&hex[3..5], 16).unwrap_or(0);
    let b = u8::from_str_radix(&hex[5..7], 16).unwrap_or(0);
    format!("rgba({r},{g},{b},{a})")
}

/// Tabler-style line glyph set the round-1 mockup uses.
///
/// Rinch's SVG paint path (`rinch-dom/src/paint/svg.rs`) reads `fill`,
/// `stroke`, `stroke-width`, `stroke-linecap`, `stroke-linejoin` as
/// **SVG attributes on the element**, not CSS style properties. The
/// outer box dimensions still come from `style: width/height`.
///
/// The glyph set is intentionally closed and named after consumer needs;
/// add new glyphs here as their consumer regions get ported. `record`
/// and `dot` are filled circles; everything else is a stroked path.
#[component]
pub fn Icon(glyph: String, size: f32, stroke: String, stroke_width: f32) -> NodeHandle {
    let name = glyph.as_str();
    let box_style = format!(
        "width: {size}px; height: {size}px; flex: 0 0 auto; display: block;",
    );

    // Filled-circle branch.
    if name == "record" || name == "dot" {
        let r_attr = if name == "dot" { "3" } else { "6" }.to_string();
        let fill_attr = stroke.clone();
        return rsx! {
            svg {
                viewBox: "0 0 24 24",
                fill: {fill_attr.clone()},
                style: {box_style.clone()},
                circle { cx: "12", cy: "12", r: {r_attr.clone()} }
            }
        };
    }

    let path_d = match name {
        "play" => "M7 4v16l13 -8z",
        "pause" => "M8 5v14M16 5v14",
        "stop" => "M6 6h12v12H6z",
        "rewind" => "M21 5v14l-10 -7zM4 5v14",
        "search" => "M10 16a6 6 0 1 1 0 -12a6 6 0 0 1 0 12zM21 21l-6 -6",
        "chevron-d" => "M6 9l6 6l6 -6",
        "chevron-r" => "M9 6l6 6l-6 6",
        "chevron-u" => "M6 15l6 -6l6 6",
        "plus" => "M12 5v14M5 12h14",
        "minus" => "M5 12h14",
        "zoom-out" => "M10 16a6 6 0 1 1 0 -12a6 6 0 0 1 0 12zM7 10h6M21 21l-6 -6",
        "zoom-in" => "M10 16a6 6 0 1 1 0 -12a6 6 0 0 1 0 12zM7 10h6M10 7v6M21 21l-6 -6",
        "pattern" => "M3 6h4v4H3zM10 6h4v4h-4zM17 6h4v4h-4zM3 14h4v4H3zM10 14h4v4h-4zM17 14h4v4h-4z",
        "chord" => "M5 4v12.5a3 3 0 1 1 -2 -2.83V7l12 -3v10.5a3 3 0 1 1 -2 -2.83V4z",
        "section" => "M3 8h6v8H3zM11 8h4v8h-4zM17 8h4v8h-4z",
        "gear" => "M19.4 15a1.65 1.65 0 0 0 .33 1.82a2 2 0 1 1 -2.83 2.83a1.65 1.65 0 0 0 -1.82 -.33a1.65 1.65 0 0 0 -1 1.51a2 2 0 1 1 -4 0a1.65 1.65 0 0 0 -1.08 -1.51a1.65 1.65 0 0 0 -1.82 .33a2 2 0 1 1 -2.83 -2.83a1.65 1.65 0 0 0 .33 -1.82a1.65 1.65 0 0 0 -1.51 -1a2 2 0 1 1 0 -4a1.65 1.65 0 0 0 1.51 -1.08a1.65 1.65 0 0 0 -.33 -1.82a2 2 0 1 1 2.83 -2.83a1.65 1.65 0 0 0 1.82 .33a1.65 1.65 0 0 0 1 -1.51a2 2 0 1 1 4 0a1.65 1.65 0 0 0 1 1.51a1.65 1.65 0 0 0 1.82 -.33a2 2 0 1 1 2.83 2.83a1.65 1.65 0 0 0 -.33 1.82a1.65 1.65 0 0 0 1.51 1a2 2 0 1 1 0 4z",
        _ => "",
    };
    let d_attr = path_d.to_string();
    let sw_attr = stroke_width.to_string();
    rsx! {
        svg {
            viewBox: "0 0 24 24",
            fill: "none",
            stroke: {stroke.clone()},
            stroke-width: {sw_attr.clone()},
            stroke-linecap: "round",
            stroke-linejoin: "round",
            style: {box_style.clone()},
            path { d: {d_attr.clone()} }
        }
    }
}

/// Paper with a colored left-edge stripe.
///
/// Generalizes the bespoke "border-left: Npx solid color + tinted body"
/// pattern that round-1's inspector header and round-2's activation cell
/// both want. The 3px stripe-width and `theme::LINE` outer border are
/// baked in because every use site shares them; add props later if a
/// caller needs to vary either.
///
/// Props:
/// - `stripe_color`: hex string for the left stripe.
/// - `background`: body background — typically `theme::BG1` for a
///   neutral card or `rgba(stripe_color, 0.06)` for a tinted header.
/// - `padding`: CSS padding shorthand, e.g. `"12px 14px"`.
/// - `radius`: border-radius in px. `0.0` for full-width headers, `6.0`
///   for free-floating cells.
/// - `children`: rsx children rendered inside the padded body.
#[component]
pub fn StripePaper(
    stripe_color: String,
    background: String,
    padding: String,
    radius: f32,
    children: &[NodeHandle],
) -> NodeHandle {
    let style = format!(
        "background: {bg}; border: 1px solid {bc}; \
         border-left: 3px solid {col}; \
         border-radius: {r}px; padding: {pad}; \
         box-sizing: border-box; overflow: hidden;",
        bg = background,
        bc = crate::theme::LINE,
        col = stripe_color,
        r = radius,
        pad = padding,
    );
    // Children are NOT auto-appended by `#[component]` — the macro just
    // exposes them as a `&[NodeHandle]` parameter and the body has to
    // wire them in. Capture the rsx root and append explicitly.
    let root = rsx! {
        div { style: {style.clone()} }
    };
    for child in children {
        root.append_child(child);
    }
    root
}

// ─── Schedule timeline primitive ─────────────────────────────────────────
//
// One activation cell's variant-schedule strip. Round-2 mockup defines a
// per-cell timeline scoped to the section's duration; round-3 will reuse
// the same primitive for sub-range editing. Built as `#[component]` here
// rather than under `components/` per Phase 4's convention — `parts.rs`
// is rawdaw-app's shared-primitives bucket until it crosses the 700-line
// cap.

/// Visual mode for one schedule segment. Drives fill, border, and label.
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum ScheduleSegmentStyle {
    /// Default-variant fill — flat low-alpha block in the pattern's
    /// identity color. The implicit-fill segments produced by the
    /// schedule builder use this mode.
    #[default]
    DefaultFill,
    /// Non-default named variant — higher alpha + diagonal hatch.
    NonDefault,
    /// Silenced sub-range — dashed border + slashed pattern + italic
    /// "silent" label. Surfaces when an entry's `variant` is `None`.
    Silent,
}

/// One segment in the schedule timeline. `start_bar` is inclusive,
/// `end_bar` exclusive (matching the engine-side `BarRange`
/// convention). `label` shows on the segment when present and the
/// segment is wide enough to display it (`NonDefault` variant ids,
/// the literal "silent" label).
#[derive(Clone, PartialEq, Eq, Default, Debug)]
pub struct ScheduleSegment {
    pub start_bar: u32,
    pub end_bar: u32,
    pub style: ScheduleSegmentStyle,
    pub label: String,
}

/// One activation's variant-schedule strip. Stateless render —
/// segments are pre-computed by the caller via a schedule builder
/// that gap-fills implicit-default ranges. The primitive itself
/// knows nothing about the data model.
///
/// Layout: `H=36`. Segment band occupies `y=3..23` (h=20); tick lane
/// `y=23..36` (h=13). Per-bar grid lines run vertically through the
/// whole height in `theme::LINE_SOFT` at low opacity.
///
/// `silent` (whole-cell) is honored independently of any per-segment
/// `Silent` style — when the entire activation is silenced (state ==
/// Silent in the cell), the whole timeline is rendered at reduced
/// opacity so it doesn't compete visually with the active cells in
/// the same view.
#[component]
pub fn ScheduleTimeline(
    total_bars: u32,
    segments: Vec<ScheduleSegment>,
    pattern_color: String,
    silent: bool,
) -> NodeHandle {
    let outer_opacity = if silent { 0.55 } else { 1.0 };
    let outer_style = format!(
        "position: relative; width: 100%; height: 36px; \
         background: {bg0}; border: 1px solid {line}; \
         border-radius: 4px; overflow: hidden; opacity: {opacity};",
        bg0 = crate::theme::BG0,
        line = crate::theme::LINE,
        opacity = outer_opacity,
    );

    let safe_total = total_bars.max(1);
    let unit_pct = 100.0_f32 / safe_total as f32;

    // rsx `for` wraps the iterator source in a `Fn() -> Vec<T>` closure
    // — a pre-computed Vec captured by name moves on each invocation
    // (`cannot move out of value, captured variable`). `.clone()`
    // inside the for source rebuilds a fresh Vec from the borrowed
    // capture, keeping the closure `Fn`. Same trick is used by the
    // chord-loop bar and the cell list (see those modules).
    rsx! {
        div { style: {outer_style.clone()},
            for tick in (1..safe_total).collect::<Vec<u32>>() {
                ScheduleGridLine { bar: tick, total_bars: safe_total }
            }
            for seg in segments.clone() {
                ScheduleSegmentBlock {
                    key: seg.start_bar,
                    seg: seg,
                    pattern_color: pattern_color.clone(),
                    unit_pct: unit_pct,
                }
            }
            for label_bar in (0..safe_total).collect::<Vec<u32>>() {
                ScheduleTickLabel { bar: label_bar, total_bars: safe_total }
            }
        }
    }
}

#[component]
fn ScheduleGridLine(bar: u32, total_bars: u32) -> NodeHandle {
    let left_pct = bar as f32 / total_bars as f32 * 100.0;
    let style = format!(
        "position: absolute; left: {l}%; top: 0; bottom: 0; \
         width: 1px; background: {line_soft}; opacity: 0.6; \
         pointer-events: none;",
        l = left_pct,
        line_soft = crate::theme::LINE_SOFT,
    );
    rsx! { div { style: {style.clone()} } }
}

#[component]
fn ScheduleTickLabel(bar: u32, total_bars: u32) -> NodeHandle {
    // Render every bar's left-edge label inside its own bar slot so
    // labels stay aligned even when bars are narrow. `top` puts the
    // label inside the tick lane (y=23..36).
    let left_pct = bar as f32 / total_bars as f32 * 100.0;
    let width_pct = 100.0_f32 / total_bars as f32;
    let style = format!(
        "position: absolute; left: {l}%; top: 23px; \
         width: {w}%; height: 13px; \
         font-size: 9px; line-height: 13px; \
         color: {text3}; font-feature-settings: \"tnum\" 1; \
         text-align: left; padding-left: 3px; \
         pointer-events: none; box-sizing: border-box;",
        l = left_pct,
        w = width_pct,
        text3 = crate::theme::TEXT3,
    );
    let label = (bar + 1).to_string();
    rsx! { div { style: {style.clone()}, {label.clone()} } }
}

#[component]
fn ScheduleSegmentBlock(
    seg: ScheduleSegment,
    pattern_color: String,
    unit_pct: f32,
) -> NodeHandle {
    let span = seg.end_bar.saturating_sub(seg.start_bar).max(1);
    let left = seg.start_bar as f32 * unit_pct;
    let width = span as f32 * unit_pct;
    let label = seg.label.clone();
    let style = segment_style_css(seg.style, pattern_color.as_str(), left, width);
    let show_label = !label.is_empty() && width > 3.0;
    // Always emit the label span; toggle via `display`. Wrapping a
    // captured `label_style: String` in an rsx `if` block forces the
    // generated `Fn` closure to consume the capture, which fails to
    // compile. (Same hazard as the meta-bar's `↳ base` chip — see
    // `section_editor/meta_bar.rs`.) Bundling display + content style
    // into a single computed string side-steps it.
    let label_style = format!(
        "{base} display: {disp};",
        base = segment_label_css(seg.style),
        disp = if show_label { "inline" } else { "none" },
    );
    rsx! {
        div { style: {style.clone()},
            span { style: {label_style.clone()}, {label.clone()} }
        }
    }
}

fn segment_style_css(
    style: ScheduleSegmentStyle,
    pattern_color: &str,
    left_pct: f32,
    width_pct: f32,
) -> String {
    let common = format!(
        "position: absolute; left: {l}%; width: {w}%; top: 3px; height: 20px; \
         border-radius: 2px; box-sizing: border-box; \
         display: flex; align-items: center; justify-content: center; \
         pointer-events: none;",
        l = left_pct,
        w = width_pct,
    );
    match style {
        ScheduleSegmentStyle::DefaultFill => format!(
            "{common} \
             background: {bg}; border: 1px solid {border};",
            bg = rgba(pattern_color, 0.14),
            border = rgba(pattern_color, 0.30),
        ),
        ScheduleSegmentStyle::NonDefault => format!(
            "{common} \
             background: \
                repeating-linear-gradient(45deg, \
                  {hatch_strong} 0, {hatch_strong} 3px, \
                  {hatch_weak} 3px, {hatch_weak} 6px); \
             border: 1px solid {border};",
            hatch_strong = rgba(pattern_color, 0.40),
            hatch_weak = rgba(pattern_color, 0.18),
            border = rgba(pattern_color, 0.55),
        ),
        ScheduleSegmentStyle::Silent => format!(
            "{common} \
             background: \
                repeating-linear-gradient(45deg, \
                  rgba(232,234,238,0.06) 0, rgba(232,234,238,0.06) 3px, \
                  transparent 3px, transparent 6px); \
             border: 1px dashed {border};",
            border = crate::theme::TEXT3,
        ),
    }
}

fn segment_label_css(style: ScheduleSegmentStyle) -> String {
    let base = "font-size: 9.5px; letter-spacing: 0.3px; \
         font-feature-settings: \"tnum\" 1; pointer-events: none; \
         text-transform: lowercase;";
    match style {
        ScheduleSegmentStyle::Silent => format!(
            "{base} color: {text2}; font-style: italic;",
            text2 = crate::theme::TEXT2,
        ),
        _ => format!("{base} color: {text0};", text0 = crate::theme::TEXT0),
    }
}

/// Where an inherited value came from. Drives the [`InheritanceTag`]
/// label and tooltip text. Default is `RoleDefault` because that's
/// the only kind of inheritance the round-2 cell renders today —
/// section-variant `↳ base` markers live on the meta bar and are
/// inlined there because they fight rsx String-capture moves.
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum InheritanceSource {
    /// Field's value matches the track role's default — i.e. the cell
    /// hasn't overridden it. Rendered as `↳ role default`.
    #[default]
    RoleDefault,
    /// Field's value matches base (used on per-variant fields elsewhere
    /// in the editor; not currently a cell-realization marker, but
    /// keeps the enum extensible).
    Base,
}

impl InheritanceSource {
    fn label(self) -> &'static str {
        match self {
            Self::RoleDefault => "↳ role default",
            Self::Base => "↳ base",
        }
    }

    fn tooltip(self) -> &'static str {
        match self {
            Self::RoleDefault => "value matches the track role's default",
            Self::Base => "inherited from base",
        }
    }
}

/// Small bordered badge announcing where a value came from. Currently
/// surfaces on per-cell realization rows ([`InheritanceSource::RoleDefault`])
/// and reserved for any future "this field is inheriting from base"
/// indicator on per-variant cell fields ([`InheritanceSource::Base`]).
///
/// The address-space caveat in the round-2 README still applies: today
/// `*` means "differs from base" on section meta and "differs from
/// role default" on cell realization. If round 3 ever pairs both axes
/// on the same field, the cell needs a paired indicator
/// (e.g. `*` for role-override and `°` for variant-of-base).
#[component]
pub fn InheritanceTag(source: InheritanceSource) -> NodeHandle {
    let style = format!(
        "display: inline-flex; align-items: center; gap: 3px; \
         color: {text3}; cursor: help; \
         padding: 1px 5px; border-radius: 2px; \
         border: 1px solid {line_soft}; letter-spacing: 0.2px; \
         font-size: 10.5px;",
        text3 = crate::theme::TEXT3,
        line_soft = crate::theme::LINE_SOFT,
    );
    let label_owned = source.label().to_string();
    let tip_owned = source.tooltip().to_string();
    rsx! {
        span { style: {style.clone()}, title: {tip_owned.clone()},
            {label_owned.clone()}
        }
    }
}

/// Small state badge for an activation row / cell: `active` / `silent` /
/// `inherit`, with an optional `*` mark when the value differs from
/// base (variant-overridden). Shared by the round-1 inspector's
/// activation table and the round-2 section editor's activation cell —
/// both pages want the same visual.
#[component]
pub fn StatePill(state: ActivationState, overridden: bool) -> NodeHandle {
    let (bg, fg, border, label) = match state {
        ActivationState::Active => (
            "rgba(127,168,138,0.18)",
            "#A8C9B0",
            "rgba(127,168,138,0.40)",
            "active",
        ),
        ActivationState::Silent => (
            "rgba(232,234,238,0.06)",
            "rgba(232,234,238,0.42)",
            "rgba(232,234,238,0.14)",
            "silent",
        ),
        ActivationState::Inherit => (
            "transparent",
            "rgba(232,234,238,0.28)",
            "rgba(232,234,238,0.14)",
            "inherit",
        ),
    };
    let style = format!(
        "display: inline-flex; align-items: center; gap: 4px; \
         padding: 1px 6px; border-radius: 3px; \
         font-size: 10px; font-weight: 500; letter-spacing: 0.3px; \
         background: {bg}; color: {fg}; border: 1px solid {border};",
    );
    let label_owned = label.to_string();
    rsx! {
        span { style: {style.clone()},
            {label_owned.clone()}
            if overridden {
                span {
                    style: "opacity: 0.6; cursor: help;",
                    title: "overridden in this variant from base",
                    "*"
                }
            }
        }
    }
}
