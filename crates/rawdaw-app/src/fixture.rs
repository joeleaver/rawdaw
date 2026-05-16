//! Round-1 fixture data, mirroring
//! `docs/design/mockups/round-1/components/data.js`.
//!
//! This is a stand-in for the real `rawdaw_model::Project` until the
//! engine is wired into the app. The shapes line up with
//! `composition-model.md` so swapping in a real project is mostly a
//! matter of building the view from `Project` fields instead of these.

#![allow(dead_code)] // fixture fields accrete with the UI; not all are read yet

use crate::theme;

#[derive(Clone, PartialEq, Eq)]
pub struct Project {
    pub name: &'static str,
    pub key: &'static str,
    pub time_sig: &'static str,
    pub tempo: u32,
    pub playhead_bar: u32,
    pub playhead_beat: u32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TrackKind {
    Pitched,
    Drum,
}

#[derive(Clone, PartialEq, Eq)]
pub struct Track {
    pub id: &'static str,
    pub name: &'static str,
    pub kind: TrackKind,
    pub role: &'static str,
}

#[derive(Clone, PartialEq, Eq)]
pub struct Pattern {
    pub id: &'static str,
    pub name: &'static str,
    pub color: &'static str,
    pub kind: &'static str, // "Pitched" | "Drum"
    pub variants: u32,
    pub meta: &'static str,
}

#[derive(Clone, PartialEq, Eq)]
pub struct ChordEvent {
    pub roman: &'static str,
    pub quality: &'static str, // empty for the default quality of the case
    pub absolute: &'static str,
}

#[derive(Clone, PartialEq, Eq)]
pub struct ChordLoop {
    pub id: &'static str,
    pub name: &'static str,
    pub color: &'static str,
    pub length_bars: u32,
    pub events: &'static [ChordEvent],
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum ActivationState {
    Active,
    Silent,
    /// No entry in the section's activation map for this track. Displayed
    /// as a third pill purely so the user sees something; per the round-1
    /// README this MUST NOT become an `ActivationOverride::Inherit`
    /// variant in the engine-side data model.
    #[default]
    Inherit,
}

#[derive(Clone, PartialEq, Eq)]
pub struct Activation {
    /// Pattern name, or empty when state is `Inherit`.
    pub pattern: &'static str,
    pub state: ActivationState,
    /// True when this variant of the section silences a base-active
    /// activation. Drives the "*" mark next to the state pill.
    pub overridden: bool,
}

#[derive(Clone, PartialEq, Eq)]
pub struct Variant {
    pub id: &'static str,
    pub name: &'static str,
}

/// Sparse variant override: pairs of (track id, new state).
pub type VariantOverride = &'static [(&'static str, ActivationState)];

#[derive(Clone, PartialEq, Eq)]
pub struct Section {
    pub id: &'static str,
    pub name: &'static str,
    pub color: &'static str,
    pub variants: &'static [Variant],
    pub default_variant: &'static str,
    pub base_duration_bars: u32,
    /// Chord-loop names attached to this section, in order.
    pub chord_loops: &'static [&'static str],
    /// Activations keyed by track id.
    pub activations: &'static [(&'static str, Activation)],
    /// Sparse variant overrides keyed by variant id.
    pub variant_overrides: &'static [(&'static str, VariantOverride)],
}

#[derive(Clone, PartialEq, Eq)]
pub struct SectionRef {
    pub idx: usize,
    pub section_key: &'static str,
    pub variant: &'static str,
    pub start_bar: u32,
    pub bars: u32,
}

pub struct Round1 {
    pub project: Project,
    pub tracks: &'static [Track],
    pub patterns: &'static [Pattern],
    pub chord_loops: &'static [ChordLoop],
    pub sections: &'static [Section],
    pub arrangement: &'static [SectionRef],
    pub total_bars: u32,
}

// ─── Data tables ──────────────────────────────────────────────────────────

static TRACKS: &[Track] = &[
    Track { id: "t_bass",  name: "bass",  kind: TrackKind::Pitched, role: "bass"    },
    Track { id: "t_lead",  name: "lead",  kind: TrackKind::Pitched, role: "melodic" },
    Track { id: "t_drums", name: "drums", kind: TrackKind::Drum,    role: "—"       },
    Track { id: "t_pad",   name: "pad",   kind: TrackKind::Pitched, role: "pad"     },
];

static PATTERNS: &[Pattern] = &[
    Pattern { id: "p_bass",  name: "bass-main",  color: theme::PAL_TEAL,
              kind: "Pitched", variants: 2, meta: "Pitched · 2 variants" },
    Pattern { id: "p_lead",  name: "lead-main",  color: theme::PAL_PLUM,
              kind: "Pitched", variants: 1, meta: "Pitched · 1 variant"  },
    Pattern { id: "p_drums", name: "drums-main", color: theme::PAL_SAGE,
              kind: "Drum",    variants: 2, meta: "Drum · 2 variants"    },
    Pattern { id: "p_pad",   name: "pad-bed",    color: theme::PAL_SLATE,
              kind: "Pitched", variants: 1, meta: "Pitched · 1 variant"  },
];

static VERSE_PROGRESSION_EVENTS: &[ChordEvent] = &[
    ChordEvent { roman: "I",  quality: "", absolute: "C"  },
    ChordEvent { roman: "V",  quality: "", absolute: "G"  },
    ChordEvent { roman: "vi", quality: "", absolute: "Am" },
    ChordEvent { roman: "IV", quality: "", absolute: "F"  },
];
static CHORUS_PROGRESSION_EVENTS: &[ChordEvent] = &[
    ChordEvent { roman: "vi", quality: "", absolute: "Am" },
    ChordEvent { roman: "IV", quality: "", absolute: "F"  },
    ChordEvent { roman: "I",  quality: "", absolute: "C"  },
    ChordEvent { roman: "V",  quality: "", absolute: "G"  },
];

static CHORD_LOOPS: &[ChordLoop] = &[
    ChordLoop { id: "cl_verse",  name: "verse-progression",
                color: theme::PAL_TERRA, length_bars: 4,
                events: VERSE_PROGRESSION_EVENTS },
    ChordLoop { id: "cl_chorus", name: "chorus-progression",
                color: theme::PAL_OLIVE, length_bars: 4,
                events: CHORUS_PROGRESSION_EVENTS },
];

// ─── Section activations ──────────────────────────────────────────────────

static INTRO_ACTIVATIONS: &[(&str, Activation)] = &[
    ("t_bass",  Activation { pattern: "bass-main",  state: ActivationState::Silent, overridden: false }),
    ("t_lead",  Activation { pattern: "lead-main",  state: ActivationState::Silent, overridden: false }),
    ("t_drums", Activation { pattern: "drums-main", state: ActivationState::Silent, overridden: false }),
    ("t_pad",   Activation { pattern: "pad-bed",    state: ActivationState::Active, overridden: false }),
];

static VERSE_ACTIVATIONS: &[(&str, Activation)] = &[
    ("t_bass",  Activation { pattern: "bass-main",  state: ActivationState::Active,  overridden: false }),
    ("t_lead",  Activation { pattern: "lead-main",  state: ActivationState::Active,  overridden: false }),
    ("t_drums", Activation { pattern: "drums-main", state: ActivationState::Active,  overridden: false }),
    ("t_pad",   Activation { pattern: "pad-bed",    state: ActivationState::Inherit, overridden: false }),
];

static VERSE_STRIPPED_OVERRIDE: &[(&str, ActivationState)] = &[
    ("t_bass",  ActivationState::Silent),
    ("t_drums", ActivationState::Silent),
];

static VERSE_VARIANT_OVERRIDES: &[(&str, VariantOverride)] = &[
    ("stripped", VERSE_STRIPPED_OVERRIDE),
];

static CHORUS_ACTIVATIONS: &[(&str, Activation)] = &[
    ("t_bass",  Activation { pattern: "bass-main",  state: ActivationState::Active, overridden: false }),
    ("t_lead",  Activation { pattern: "lead-main",  state: ActivationState::Active, overridden: false }),
    ("t_drums", Activation { pattern: "drums-main", state: ActivationState::Active, overridden: false }),
    ("t_pad",   Activation { pattern: "pad-bed",    state: ActivationState::Active, overridden: false }),
];

// ─── Sections ─────────────────────────────────────────────────────────────

static INTRO_VARIANTS: &[Variant] = &[Variant { id: "base", name: "base" }];
static VERSE_VARIANTS: &[Variant] = &[
    Variant { id: "base",     name: "base"     },
    Variant { id: "stripped", name: "stripped" },
];
static CHORUS_VARIANTS: &[Variant] = &[Variant { id: "base", name: "base" }];

static EMPTY_OVERRIDES: &[(&str, VariantOverride)] = &[];

static SECTIONS: &[Section] = &[
    Section {
        id: "s_intro", name: "intro", color: theme::PAL_ROSE,
        variants: INTRO_VARIANTS, default_variant: "base",
        base_duration_bars: 4,
        chord_loops: &["verse-progression"],
        activations: INTRO_ACTIVATIONS,
        variant_overrides: EMPTY_OVERRIDES,
    },
    Section {
        id: "s_verse", name: "verse", color: theme::PAL_BLUE,
        variants: VERSE_VARIANTS, default_variant: "base",
        base_duration_bars: 4,
        chord_loops: &["verse-progression"],
        activations: VERSE_ACTIVATIONS,
        variant_overrides: VERSE_VARIANT_OVERRIDES,
    },
    Section {
        id: "s_chorus", name: "chorus", color: theme::PAL_SAND,
        variants: CHORUS_VARIANTS, default_variant: "base",
        base_duration_bars: 8,
        chord_loops: &["chorus-progression"],
        activations: CHORUS_ACTIVATIONS,
        variant_overrides: EMPTY_OVERRIDES,
    },
];

static ARRANGEMENT: &[SectionRef] = &[
    SectionRef { idx: 0, section_key: "intro",  variant: "base",     start_bar: 0,  bars: 4 },
    SectionRef { idx: 1, section_key: "verse",  variant: "base",     start_bar: 4,  bars: 4 },
    SectionRef { idx: 2, section_key: "verse",  variant: "stripped", start_bar: 8,  bars: 4 },
    SectionRef { idx: 3, section_key: "verse",  variant: "base",     start_bar: 12, bars: 4 },
    SectionRef { idx: 4, section_key: "chorus", variant: "base",     start_bar: 16, bars: 8 },
];

pub fn round1() -> Round1 {
    Round1 {
        project: Project {
            name: "untitled-1",
            key: "C major",
            time_sig: "4/4",
            tempo: 96,
            playhead_bar: 5,
            playhead_beat: 2,
        },
        tracks: TRACKS,
        patterns: PATTERNS,
        chord_loops: CHORD_LOOPS,
        sections: SECTIONS,
        arrangement: ARRANGEMENT,
        total_bars: 24,
    }
}

// ─── Lookup helpers ───────────────────────────────────────────────────────

pub fn section_by_key<'a>(r: &'a Round1, key: &str) -> Option<&'a Section> {
    r.sections.iter().find(|s| s.name == key)
}

pub fn pattern_by_name<'a>(r: &'a Round1, name: &str) -> Option<&'a Pattern> {
    r.patterns.iter().find(|p| p.name == name)
}

pub fn chord_loop_by_name<'a>(r: &'a Round1, name: &str) -> Option<&'a ChordLoop> {
    r.chord_loops.iter().find(|c| c.name == name)
}

pub fn track_by_id<'a>(r: &'a Round1, id: &str) -> Option<&'a Track> {
    r.tracks.iter().find(|t| t.id == id)
}

/// Number of arrangement blocks that point at `section_key`. Drives the
/// "N instances in arrangement" readout in the inspector header.
pub fn instance_count(r: &Round1, section_key: &str) -> usize {
    r.arrangement.iter().filter(|b| b.section_key == section_key).count()
}
