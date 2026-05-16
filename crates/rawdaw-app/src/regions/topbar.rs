//! Top bar — project identity (left) · transport (center) · tempo/zoom (right).
//!
//! Translates `docs/design/mockups/round-1/components/topbar.jsx`.
//! The chrome row sits at `theme::BG1` so it lifts off the arrangement
//! (`theme::BG0`); the inset readouts inside use `theme::BG0` to look
//! recessed against the lighter chrome — same trick as the mockup.

use rinch::prelude::*;

use crate::fixture;
use crate::parts::Icon;
use crate::theme;

#[component]
pub fn TopBar() -> NodeHandle {
    let r = fixture::round1();
    let p = r.project;

    let bar_style = format!(
        "height: {h}px; flex: 0 0 {h}px; \
         background: {bg}; border-bottom: 1px solid {line}; \
         display: grid; grid-template-columns: 1fr auto 1fr; \
         align-items: center; padding: 0 12px; gap: 12px;",
        h = theme::H_TOPBAR,
        bg = theme::BG1,
        line = theme::LINE,
    );

    let project_name = p.name.to_string();
    let project_meta = format!("{} · {}", p.key, p.time_sig);
    let playhead_bar = p.playhead_bar.to_string();
    let playhead_beat = p.playhead_beat.to_string();
    let tempo = format!("{}.00", p.tempo);

    rsx! {
        div { style: {bar_style.clone()},
            // Left: project identity
            div {
                style: "display: flex; flex-direction: column; \
                        justify-content: center; gap: 1px; min-width: 0;",
                div {
                    style: "font-size: 13px; font-weight: 600; \
                            color: rgba(232,234,238,0.96); \
                            letter-spacing: -0.1px; \
                            overflow: hidden; text-overflow: ellipsis; \
                            white-space: nowrap;",
                    {project_name.clone()}
                }
                div {
                    style: "font-size: 11px; color: rgba(232,234,238,0.42); \
                            font-feature-settings: \"tnum\" 1;",
                    {project_meta.clone()}
                }
            }

            // Center: transport
            div {
                style: "display: flex; align-items: center; gap: 4px;",
                TransportBtn { glyph: "rewind", title: "Return to zero" }
                TransportBtn { glyph: "play",   title: "Play", primary: true }
                TransportBtn { glyph: "stop",   title: "Stop" }
                TransportBtn {
                    glyph: "record",
                    title: "Record (disabled — MIDI v1 has no audio recording)",
                    disabled: true,
                }
                div {
                    style: format!(
                        "width: 1px; height: 18px; background: {line}; margin: 0 6px;",
                        line = theme::LINE,
                    ),
                }
                // Bar / beat readout (inset against bg1)
                div {
                    style: format!(
                        "display: flex; align-items: baseline; gap: 6px; \
                         padding: 3px 9px; border-radius: 4px; \
                         background: {bg0}; border: 1px solid {line}; \
                         font-size: 12px; color: rgba(232,234,238,0.96); \
                         font-feature-settings: \"tnum\" 1; \
                         font-variant-numeric: tabular-nums;",
                        bg0 = theme::BG0, line = theme::LINE,
                    ),
                    Tag { text: "Bar" }
                    span { style: "min-width: 14px; text-align: right;",
                        {playhead_bar.clone()}
                    }
                    span { style: "color: rgba(232,234,238,0.28);", "·" }
                    Tag { text: "Beat" }
                    span { style: "min-width: 8px;", {playhead_beat.clone()} }
                }
            }

            // Right: tempo · zoom · gear
            div {
                style: "display: flex; align-items: center; gap: 6px; \
                        justify-content: flex-end;",
                div {
                    style: format!(
                        "display: flex; align-items: baseline; gap: 6px; \
                         padding: 3px 9px; border-radius: 4px; \
                         background: {bg0}; border: 1px solid {line}; \
                         font-size: 12px; color: rgba(232,234,238,0.96); \
                         font-feature-settings: \"tnum\" 1;",
                        bg0 = theme::BG0, line = theme::LINE,
                    ),
                    span {
                        style: "min-width: 22px; text-align: right; \
                                font-variant-numeric: tabular-nums;",
                        {tempo.clone()}
                    }
                    Tag { text: "BPM" }
                }
                div { style: "display: flex; gap: 1px; margin-left: 4px;",
                    IconBtn { glyph: "zoom-out", title: "Zoom out" }
                    IconBtn { glyph: "zoom-in",  title: "Zoom in"  }
                }
                IconBtn { glyph: "gear", title: "Project settings" }
            }
        }
    }
}

// ─── Helpers (PascalCase components) ──────────────────────────────────────

/// Small uppercase label inside numeric readouts ("Bar", "Beat", "BPM").
#[component]
fn Tag(text: String) -> NodeHandle {
    rsx! {
        span {
            style: "color: rgba(232,234,238,0.42); font-size: 10px; \
                    text-transform: uppercase; letter-spacing: 0.6px;",
            {text.clone()}
        }
    }
}

/// Transport button (28×28). Plays / stops / rewinds / records. Record is
/// rendered as a filled dot, not a stroked path.
#[component]
fn TransportBtn(
    glyph: String,
    title: String,
    primary: bool,
    disabled: bool,
) -> NodeHandle {
    // Color choice: disabled → dim; primary → ok; otherwise → text0.
    let color = if disabled {
        "rgba(232,234,238,0.28)"
    } else if primary {
        theme::OK
    } else {
        "rgba(232,234,238,0.96)"
    };
    let style = format!(
        "height: 28px; width: 28px; border-radius: 4px; \
         background: transparent; border: 1px solid transparent; \
         color: {color}; cursor: {cursor}; padding: 0; \
         display: inline-flex; align-items: center; justify-content: center;",
        color = color,
        cursor = if disabled { "not-allowed" } else { "pointer" },
    );
    let sw = if glyph == "play" { 1.8_f32 } else { 1.6_f32 };
    let sz = if glyph == "play" { 14.0_f32 } else { 13.0_f32 };
    rsx! {
        button {
            r#type: "button",
            title: {title.clone()},
            style: {style.clone()},
            Icon {
                glyph: {glyph.clone()},
                size: sz,
                stroke: color.to_string(),
                stroke_width: sw,
            }
        }
    }
}

/// Plain icon button (26×26). Used for zoom + gear.
#[component]
fn IconBtn(glyph: String, title: String) -> NodeHandle {
    let color = "rgba(232,234,238,0.62)".to_string();
    let style = format!(
        "height: 26px; width: 26px; border-radius: 4px; \
         background: transparent; border: 1px solid transparent; \
         color: {color}; cursor: pointer; padding: 0; \
         display: inline-flex; align-items: center; justify-content: center;",
    );
    let sz = 14.0_f32;
    let sw = 1.6_f32;
    let stroke = color.clone();
    rsx! {
        button {
            r#type: "button",
            title: {title.clone()},
            style: {style.clone()},
            Icon {
                glyph: {glyph.clone()},
                size: sz,
                stroke: {stroke.clone()},
                stroke_width: sw,
            }
        }
    }
}

// Icon now lives at crate::parts::Icon (shared by every region that needs it).
