//! Transport buttons (Rewind / Play / Pause / Stop / Record).
//!
//! These compose into the TopBar's center column. Handlers read
//! [`AudioResources`] from the rinch store (installed by
//! `main_window` at app launch). Each `move ||` closure re-resolves
//! the store on click, so cloning the store handle into the closure
//! is unnecessary.

use rinch::prelude::*;

use crate::parts::Icon;
use crate::theme;

/// Transport button (28×28). Plays / stops / rewinds / records. Record is
/// rendered as a filled dot, not a stroked path.
///
/// `onclick` defaults to a no-op (the macro's default impl for `Callback`),
/// so the disabled Record button can omit it. Active buttons pass a
/// closure that drives [`crate::audio::AudioResources`].
#[component]
pub(super) fn TransportBtn(
    glyph: String,
    title: String,
    primary: bool,
    disabled: bool,
    onclick: Callback,
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
    let sw = if glyph == "play" || glyph == "pause" { 1.8_f32 } else { 1.6_f32 };
    let sz = if glyph == "play" || glyph == "pause" { 14.0_f32 } else { 13.0_f32 };
    rsx! {
        button {
            r#type: "button",
            title: {title.clone()},
            style: {style.clone()},
            onclick: move || onclick.invoke(),
            Icon {
                glyph: {glyph.clone()},
                size: sz,
                stroke: color.to_string(),
                stroke_width: sw,
            }
        }
    }
}
