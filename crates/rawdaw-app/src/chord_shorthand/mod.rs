//! Chord-shorthand parser and formatter.
//!
//! Reads keyboard-friendly chord shorthand (Roman / Nashville
//! Number System, e.g. `V/V`, `bVImaj7add9#11`, `Cmaj7/E`) into
//! [`rawdaw_model::chord::ChordSpec`] plus optional
//! [`rawdaw_model::chord::BassSpec`]. The grammar is specified
//! in `grammar.md` next to this file; that document is the
//! contract — `parse(...)` and `format(...)` must satisfy every
//! row of its **Normative test table**.
//!
//! Per CL0 design decision 7, the shorthand lives in rawdaw-app
//! rather than its own crate. Per design decision 6, the grammar
//! lands before parser code; if behavior diverges from the spec,
//! the spec is wrong, not the parser — update the spec and the
//! tests in lockstep.
//!
//! ## Public surface
//!
//! - [`parse`] — string → [`ParsedChord`].
//! - [`format`] — [`ParsedChord`] → canonical string.
//! - [`ParsedChord`] — `ChordSpec` + optional `BassSpec`. Bass is
//!   stored on `ChordEvent`, not `ChordSpec`, so the parser
//!   surfaces both pieces and the caller assembles the event.
//! - [`ParseError`] — span-precise (byte offsets) error info.

mod format;
mod parse;
#[cfg(test)]
mod tests;

#[allow(unused_imports)]
pub use format::format;
#[allow(unused_imports)]
pub use parse::{parse, ParseError, ParsedChord};
