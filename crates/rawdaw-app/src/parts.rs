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
    let _ = children; // children are auto-appended by the rsx macro
    rsx! {
        div { style: {style.clone()} }
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
