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

use super::mutate_event;

#[component]
pub(super) fn ExtensionChips(id: ChordLoopId, idx: usize, current_csv: String) -> NodeHandle {
    use Extension::*;
    let all = [Add9, Add11, Add13, Ninth, Eleventh, Thirteenth];
    rsx! {
        div { style: {chip_row_style()},
            for ext in all {
                // The rsx for-loop's `key:` closure binds the
                // pattern by reference (rinch `for_each_dom_typed`
                // signature: `K: Fn(&T) -> String`), while the body
                // closure binds it by value. So `key:` sees
                // `&Extension` and the other props see `Extension`
                // — deref the key expression with `*ext` and keep
                // the body expressions unchanged.
                ExtChip {
                    key: encode_extension(*ext),
                    id: id,
                    idx: idx,
                    ext_str: encode_extension(ext),
                    label: ext_label(ext),
                    active: current_csv.split(',').any(|x| x == encode_extension(ext)),
                }
            }
        }
    }
}

#[component]
fn ExtChip(id: ChordLoopId, idx: usize, ext_str: String, label: String, active: bool) -> NodeHandle {
    let style = chip_button_style(active);
    rsx! {
        button {
            r#type: "button",
            style: {style.clone()},
            onclick: move || {
                if let Some(ext) = decode_extension(&ext_str) {
                    toggle_extension(id, idx, ext);
                }
            },
            {label.clone()}
        }
    }
}

#[component]
pub(super) fn AlterationChips(id: ChordLoopId, idx: usize, current_csv: String) -> NodeHandle {
    use Alteration::*;
    let all = [Flat5, Sharp5, Flat9, Sharp9, Sharp11, Flat13, NoFifth, NoThird];
    rsx! {
        div { style: {chip_row_style()},
            for alt in all {
                AltChip {
                    key: encode_alteration(*alt),
                    id: id,
                    idx: idx,
                    alt_str: encode_alteration(alt),
                    label: alt_label(alt),
                    active: current_csv.split(',').any(|x| x == encode_alteration(alt)),
                }
            }
        }
    }
}

#[component]
fn AltChip(id: ChordLoopId, idx: usize, alt_str: String, label: String, active: bool) -> NodeHandle {
    let style = chip_button_style(active);
    rsx! {
        button {
            r#type: "button",
            style: {style.clone()},
            onclick: move || {
                if let Some(alt) = decode_alteration(&alt_str) {
                    toggle_alteration(id, idx, alt);
                }
            },
            {label.clone()}
        }
    }
}

fn encode_extension(e: Extension) -> String {
    format!("{e:?}")
}

fn decode_extension(s: &str) -> Option<Extension> {
    use Extension::*;
    Some(match s {
        "Add9" => Add9, "Add11" => Add11, "Add13" => Add13,
        "Ninth" => Ninth, "Eleventh" => Eleventh, "Thirteenth" => Thirteenth,
        _ => return None,
    })
}

fn ext_label(e: Extension) -> String {
    use Extension::*;
    match e {
        Add9 => "add9", Add11 => "add11", Add13 => "add13",
        Ninth => "9", Eleventh => "11", Thirteenth => "13",
    }
    .into()
}

pub(super) fn encode_extension_csv(list: &[Extension]) -> String {
    list.iter().map(|e| encode_extension(*e)).collect::<Vec<_>>().join(",")
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

fn decode_alteration(s: &str) -> Option<Alteration> {
    use Alteration::*;
    Some(match s {
        "Flat5" => Flat5, "Sharp5" => Sharp5, "Flat9" => Flat9, "Sharp9" => Sharp9,
        "Sharp11" => Sharp11, "Flat13" => Flat13, "NoFifth" => NoFifth, "NoThird" => NoThird,
        _ => return None,
    })
}

fn alt_label(a: Alteration) -> String {
    use Alteration::*;
    match a {
        Flat5 => "♭5", Sharp5 => "♯5", Flat9 => "♭9", Sharp9 => "♯9",
        Sharp11 => "♯11", Flat13 => "♭13", NoFifth => "no 5", NoThird => "no 3",
    }
    .into()
}

pub(super) fn encode_alteration_csv(list: &[Alteration]) -> String {
    list.iter().map(|a| encode_alteration(*a)).collect::<Vec<_>>().join(",")
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
    fn extension_alteration_csv_round_trip() {
        let exts = vec![Extension::Add9, Extension::Ninth];
        let csv = encode_extension_csv(&exts);
        for e in &exts {
            assert!(csv.split(',').any(|x| x == encode_extension(*e)));
        }
        let alts = vec![Alteration::Flat5, Alteration::Sharp9];
        let csv = encode_alteration_csv(&alts);
        for a in &alts {
            assert!(csv.split(',').any(|x| x == encode_alteration(*a)));
        }
    }
}
