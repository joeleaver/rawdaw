//! Extension + Alteration multi-select chip rows.
//!
//! Each chip is a one-of-N toggle that flips the corresponding
//! `Vec<Extension>` / `Vec<Alteration>` membership in the focused
//! event's `ChordSuffix`. CSV-encoded "current" string is passed
//! down to the chip components so they can render their active
//! state from a single owned String (the rsx macro's prop-Default
//! rules forbid passing the live Vec directly).

use rinch::prelude::*;

use rawdaw_model::chord::{Alteration, ChordSpec, Extension};
use rawdaw_model::id::ChordLoopId;

use crate::theme;

use super::{fetch_event, mutate_event};

/// Extension chip row. Each chip is a raw `button` (not a separate
/// component) so the per-iteration `Extension` value — Copy, no
/// String needed — moves into the style + onclick closures
/// cleanly. Sub-componentizing would force a String prop and run
/// into rinch's FnMut-capture rules on multiple closures.
#[component]
pub(super) fn ExtensionChips(id: ChordLoopId, idx: usize) -> NodeHandle {
    use Extension::*;
    let all = [Add9, Add11, Add13, Ninth, Eleventh, Thirteenth];
    rsx! {
        div { style: {chip_row_style()},
            for ext in all {
                button {
                    // `key:` runs `Fn(&T) -> String` so `ext` is bound
                    // by reference here; the body props receive
                    // `Extension` by value, so deref with `*ext` only
                    // in the key expression.
                    key: encode_extension(*ext),
                    r#type: "button",
                    style: {move || chip_button_style(is_extension_active(id, idx, ext))},
                    onclick: move || toggle_extension(id, idx, ext),
                    {ext_label(ext)}
                }
            }
        }
    }
}

#[component]
pub(super) fn AlterationChips(id: ChordLoopId, idx: usize) -> NodeHandle {
    use Alteration::*;
    let all = [Flat5, Sharp5, Flat9, Sharp9, Sharp11, Flat13, NoFifth, NoThird];
    rsx! {
        div { style: {chip_row_style()},
            for alt in all {
                button {
                    key: encode_alteration(*alt),
                    r#type: "button",
                    style: {move || chip_button_style(is_alteration_active(id, idx, alt))},
                    onclick: move || toggle_alteration(id, idx, alt),
                    {alt_label(alt)}
                }
            }
        }
    }
}

fn is_extension_active(id: ChordLoopId, idx: usize, ext: Extension) -> bool {
    fetch_event(id, idx)
        .map(|ev| match &ev.chord {
            ChordSpec::Functional { suffix, .. } | ChordSpec::Absolute { suffix, .. } => {
                suffix.extensions.contains(&ext)
            }
        })
        .unwrap_or(false)
}

fn is_alteration_active(id: ChordLoopId, idx: usize, alt: Alteration) -> bool {
    fetch_event(id, idx)
        .map(|ev| match &ev.chord {
            ChordSpec::Functional { suffix, .. } | ChordSpec::Absolute { suffix, .. } => {
                suffix.alterations.contains(&alt)
            }
        })
        .unwrap_or(false)
}

fn encode_extension(e: Extension) -> String {
    format!("{e:?}")
}

fn ext_label(e: Extension) -> String {
    use Extension::*;
    match e {
        Add9 => "add9", Add11 => "add11", Add13 => "add13",
        Ninth => "9", Eleventh => "11", Thirteenth => "13",
    }
    .into()
}

fn toggle_extension(id: ChordLoopId, idx: usize, value: Extension) {
    mutate_event(id, idx, move |ev| match &mut ev.chord {
        ChordSpec::Functional { suffix, .. } | ChordSpec::Absolute { suffix, .. } => {
            if let Some(pos) = suffix.extensions.iter().position(|e| *e == value) {
                suffix.extensions.remove(pos);
            } else {
                suffix.extensions.push(value);
            }
        }
    });
}

fn encode_alteration(a: Alteration) -> String {
    format!("{a:?}")
}

fn alt_label(a: Alteration) -> String {
    use Alteration::*;
    match a {
        Flat5 => "♭5", Sharp5 => "♯5", Flat9 => "♭9", Sharp9 => "♯9",
        Sharp11 => "♯11", Flat13 => "♭13", NoFifth => "no 5", NoThird => "no 3",
    }
    .into()
}

fn toggle_alteration(id: ChordLoopId, idx: usize, value: Alteration) {
    mutate_event(id, idx, move |ev| match &mut ev.chord {
        ChordSpec::Functional { suffix, .. } | ChordSpec::Absolute { suffix, .. } => {
            if let Some(pos) = suffix.alterations.iter().position(|a| *a == value) {
                suffix.alterations.remove(pos);
            } else {
                suffix.alterations.push(value);
            }
        }
    });
}

fn chip_row_style() -> String {
    "display: flex; flex-wrap: wrap; gap: 4px;".into()
}

fn chip_button_style(active: bool) -> String {
    if active {
        format!(
            "height: 22px; padding: 0 8px; \
             border-radius: 11px; background: rgba(124,158,194,0.20); \
             border: 1px solid {accent}; color: rgba(232,234,238,0.96); \
             font-size: 11px; cursor: pointer;",
            accent = theme::PAL_BLUE,
        )
    } else {
        format!(
            "height: 22px; padding: 0 8px; \
             border-radius: 11px; background: transparent; \
             border: 1px solid {line}; color: rgba(232,234,238,0.62); \
             font-size: 11px; cursor: pointer;",
            line = theme::LINE,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extension_encode_round_trips() {
        use Extension::*;
        for e in [Add9, Add11, Add13, Ninth, Eleventh, Thirteenth] {
            assert_eq!(encode_extension(e), format!("{e:?}"));
        }
    }

    #[test]
    fn alteration_encode_round_trips() {
        use Alteration::*;
        for a in [Flat5, Sharp5, Flat9, Sharp9, Sharp11, Flat13, NoFifth, NoThird] {
            assert_eq!(encode_alteration(a), format!("{a:?}"));
        }
    }
}
