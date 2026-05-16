//! The main window composition.
//!
//! Wires the five regions together and (eventually) owns the
//! cross-region selection state. For the round-1 smoke build each
//! region is a placeholder; this file stays small through the port —
//! its job is plumbing and selection state, nothing else.

use rinch::prelude::*;

use crate::regions::{Arrangement, BottomStrip, Inspector, Library, TopBar};
use crate::theme;

#[component]
pub fn main_window() -> NodeHandle {
    let style = format!(
        "width: 100%; height: 100%; \
         background: {bg}; color: rgba(232,234,238,0.96); \
         font-family: {font}; font-size: 13px; line-height: 1.4; \
         display: flex; flex-direction: column; overflow: hidden;",
        bg = theme::BG0,
        font = theme::FONT_SANS,
    );
    let row_style = "flex: 1; display: flex; min-height: 0;";
    rsx! {
        div { style: {style.clone()},
            TopBar { }
            div { style: {row_style},
                Library { }
                // Round 1 selection is hardcoded to "verse base at bar 5–8"
                // (idx=1) so the inspector renders its populated state and
                // the arrangement shows linked-highlight on the matching
                // section blocks. The MainWindow port (task #23) will swap
                // this for a `Signal<Option<usize>>` driven by clicks.
                Arrangement { selected_idx: Some(1usize) }
                Inspector { idx: Some(1usize) }
            }
            BottomStrip { }
        }
    }
}
