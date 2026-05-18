//! Modulation matrix types — slot-based routing of modulation
//! sources (envelopes, LFOs, oscillator audio) to destinations
//! (filter parameters, per-osc tune/level/PM/AM/RM amounts, …).
//!
//! The matrix is the v2 replacement for v1's hardcoded LFO→cutoff
//! routing and per-osc `mod_source` / `mod_mode` / `mod_amount`
//! fields. Patches own a fixed-size `[ModSlot; N]` array; the
//! voice's per-sample tick evaluates active slots into a
//! [`Modulations`] bag that filter / per-osc render consult.
//!
//! See `docs/wavetable-synth-mod-matrix-plan.md` for the design
//! rationale (PM-not-FM phase semantics, topo-sort-and-reject cycle
//! handling, per-destination scale units, etc.).

mod matrix;

pub use matrix::{
    ModDestination, ModMatrix, ModSlot, ModSource, Modulations, NUM_OSCS_PER_VOICE,
};
