//! Round-1 overlay builder. Populates a [`ProjectOverlay`] with the
//! UI-only decorations the model doesn't carry — pattern / section /
//! chord-loop colors, library meta strings, and per-cell realization
//! decorations for the round-2 section editor.
//!
//! Originally lived in `fixture/data.rs`; moved here in C1b so the
//! production `initial_project::build_initial()` factory owns it. The
//! fixture module re-imports `build_round1_overlay` from here while
//! `fixture::round1()` still exists.

use rawdaw_model::fixtures::Round1Keys;

use crate::overlay::{
    CellOverlay, Humanization, OctaveSpec, ProjectOverlay, Realization, Voicing,
};
use crate::theme;

pub fn build_round1_overlay(k: &Round1Keys) -> ProjectOverlay {
    let mut o = ProjectOverlay::empty();

    o.pattern_color.insert(k.patterns.bass,  theme::PAL_TEAL.to_string());
    o.pattern_color.insert(k.patterns.lead,  theme::PAL_PLUM.to_string());
    o.pattern_color.insert(k.patterns.drums, theme::PAL_SAGE.to_string());
    o.pattern_color.insert(k.patterns.pad,   theme::PAL_SLATE.to_string());

    // P1 of the pattern-editor plan derives kind / bars / variant
    // count from the model in `regions/library/patterns.rs`, so the
    // placeholder overlay seeds the old read-only PatternsGroup
    // rendered are no longer needed — they'd just duplicate the
    // derived meta. `pattern_meta` is still available for user-
    // written annotations and the new module appends them to the
    // derived string in parentheses when set.

    o.section_color.insert(k.sections.intro,  theme::PAL_ROSE.to_string());
    o.section_color.insert(k.sections.verse,  theme::PAL_BLUE.to_string());
    o.section_color.insert(k.sections.chorus, theme::PAL_SAND.to_string());

    o.chord_loop_color.insert(k.chord_loops.verse,  theme::PAL_TERRA.to_string());
    o.chord_loop_color.insert(k.chord_loops.chorus, theme::PAL_OLIVE.to_string());

    // Round-2 cell realization decorations. Mirrors the mockup data in
    // `mockups/round-2/components/data.js`. The model's
    // `RealizationParams` only carries voicing + humanization scalars;
    // octave is per-event, and the UI's humanization semantics
    // (fractional velocity, ticks, swing, seed) differ from
    // `RealizationParams`'s u8 jitter fields. Until the model grows a
    // richer realization vocabulary, the per-cell display values live
    // here.
    let verse = k.sections.verse;
    let chorus = k.sections.chorus;
    let bass = k.tracks.bass;
    let lead = k.tracks.lead;
    let drums = k.tracks.drums;
    let pad = k.tracks.pad;

    o.cell.insert((verse, "base".into(), bass), CellOverlay {
        realization: pitched(Voicing::Power, OctaveSpec::Nearest,
            Humanization { velocity: 0.04, timing: 4, swing: 0.0, seed: 1742 }),
        pinned: 0,
    });
    o.cell.insert((verse, "base".into(), lead), CellOverlay {
        realization: pitched(Voicing::TriadClose, OctaveSpec::Anchored(4),
            Humanization { velocity: 0.06, timing: 5, swing: 0.0, seed: 913 }),
        pinned: 2,
    });
    o.cell.insert((verse, "base".into(), drums), CellOverlay {
        realization: drum(Humanization { velocity: 0.10, timing: 7, swing: 0.05, seed: 8821 }),
        pinned: 0,
    });

    // Verse-stripped: bass is fully silenced (no realization needed);
    // lead is replaced with anchored(4) but lower humanization.
    o.cell.insert((verse, "stripped".into(), lead), CellOverlay {
        realization: pitched(Voicing::TriadClose, OctaveSpec::Anchored(4),
            Humanization { velocity: 0.05, timing: 4, swing: 0.0, seed: 913 }),
        pinned: 2,
    });

    // Chorus base — all four tracks active.
    o.cell.insert((chorus, "base".into(), bass), CellOverlay {
        realization: pitched(Voicing::Power, OctaveSpec::Nearest,
            Humanization { velocity: 0.04, timing: 4, swing: 0.0, seed: 1742 }),
        pinned: 0,
    });
    o.cell.insert((chorus, "base".into(), lead), CellOverlay {
        realization: pitched(Voicing::TriadClose, OctaveSpec::UpFromPrev,
            Humanization { velocity: 0.07, timing: 5, swing: 0.0, seed: 913 }),
        pinned: 3,
    });
    o.cell.insert((chorus, "base".into(), drums), CellOverlay {
        realization: drum(Humanization { velocity: 0.12, timing: 8, swing: 0.05, seed: 8821 }),
        pinned: 0,
    });
    o.cell.insert((chorus, "base".into(), pad), CellOverlay {
        // drop2 overrides role:pad's default triad-open; Nearest
        // overrides role:pad's default Anchored(3).
        realization: pitched(Voicing::Drop2, OctaveSpec::Nearest,
            Humanization { velocity: 0.02, timing: 2, swing: 0.0, seed: 3104 }),
        pinned: 0,
    });

    o
}

fn pitched(voicing: Voicing, octave: OctaveSpec, h: Humanization) -> Realization {
    Realization { voicing: Some(voicing), octave: Some(octave), humanization: h }
}

fn drum(h: Humanization) -> Realization {
    Realization { voicing: None, octave: None, humanization: h }
}
