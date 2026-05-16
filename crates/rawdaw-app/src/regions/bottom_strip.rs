//! Bottom detail strip — collapsed state for round 1.
//!
//! Translates the `BottomDetailStrip` portion of
//! `docs/design/mockups/round-1/components/main-window.jsx`. When the
//! piano roll / pattern editor lands in round 2, the expanded state
//! takes over this region; today it's a quiet 32 px strip with a
//! chevron-up affordance and a tip about how to open the detail view.

use rinch::prelude::*;

use crate::theme;

#[component]
pub fn BottomStrip() -> NodeHandle {
    let bar_style = format!(
        "height: {h}px; flex: 0 0 {h}px; \
         background: {bg}; border-top: 1px solid {line}; \
         display: flex; align-items: center; gap: 8px; padding: 0 12px;",
        h = theme::H_DETAIL,
        bg = theme::BG1,
        line = theme::LINE,
    );

    rsx! {
        div { style: {bar_style.clone()},
            // Chevron-up icon: clicking would expand the detail view in round 2.
            svg {
                viewBox: "0 0 24 24",
                fill: "none",
                stroke: "rgba(232,234,238,0.42)",
                stroke-width: "1.6",
                stroke-linecap: "round",
                stroke-linejoin: "round",
                style: "width: 13px; height: 13px; flex: 0 0 auto; display: block;",
                path { d: "M6 15l6 -6l6 6" }
            }
            span {
                style: "font-size: 11.5px; color: rgba(232,234,238,0.42);",
                "No detail view open"
            }
            span { style: "flex: 1;", "" }
            span {
                style: "font-size: 10.5px; color: rgba(232,234,238,0.28);",
                "piano roll · pattern editor open here on drill-down"
            }
        }
    }
}
