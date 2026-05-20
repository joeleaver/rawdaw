//! Parser for the shorthand grammar specified in `grammar.md`.
//!
//! The parser is a hand-written recursive-descent scanner over byte
//! indices into the input string. Whitespace inside a token is
//! ignored per grammar §5; error spans use byte offsets into the
//! original input (so leading whitespace in the user's text input
//! still resolves to a sensible highlight).
//!
//! Quality aliases use a longest-match table (e.g. `m7b5` is one
//! token, not `m7` + alteration `b5`). The table is sorted by
//! length descending; first prefix that matches wins.
//!
//! `in_key` resolution: when slash content parses as a sub-chord,
//! its degree+quality-default determine the borrowed scale via the
//! mode table in grammar.md §"Mode selection from `in_key`". The
//! sub-chord's extensions and alterations are ignored for mode
//! selection; only its root degree (+ quality default) builds the
//! new `Scale`.

use std::ops::Range;

use rawdaw_model::chord::{
    Alteration, BassSpec, ChordQuality, ChordSpec, ChordSuffix, Extension, RomanDegree,
};
use rawdaw_model::pitch::{Accidental, PitchClass};
use rawdaw_model::scale::{Mode, Scale};

/// Output of [`parse`]. Bass is broken out because the model stores
/// it on `ChordEvent`, not on `ChordSpec`; the caller assembles the
/// event from these two pieces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedChord {
    pub chord: ChordSpec,
    pub bass: Option<BassSpec>,
}

/// A parse failure with byte-offset span into the input. The inspector
/// uses the span to highlight the failing token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub span: Range<usize>,
    pub message: String,
}

/// Parse a chord shorthand expression. `current_key` is needed to
/// resolve `in_key` shorthand (`V/V` requires the absolute tonic of
/// the borrowed scale).
pub fn parse(input: &str, current_key: &Scale) -> Result<ParsedChord, ParseError> {
    if input.trim().is_empty() {
        return Err(ParseError {
            span: 0..input.len(),
            message: "empty input".into(),
        });
    }
    let mut p = Parser::new(input);
    p.skip_ws();
    let parsed = p.parse_chord_expression(current_key)?;
    p.skip_ws();
    if p.pos < p.input.len() {
        return Err(p.err_at(p.pos, "unexpected trailing input"));
    }
    Ok(parsed)
}

// ─── Parser ──────────────────────────────────────────────────────────────

struct Parser<'a> {
    input: &'a str,
    pos: usize,
}

impl<'a> Parser<'a> {
    fn new(input: &'a str) -> Self {
        Self { input, pos: 0 }
    }

    fn skip_ws(&mut self) {
        while self.pos < self.input.len() {
            let c = self.input.as_bytes()[self.pos];
            if c == b' ' || c == b'\t' {
                self.pos += 1;
            } else {
                break;
            }
        }
    }

    fn peek(&self) -> Option<u8> {
        self.input.as_bytes().get(self.pos).copied()
    }

    fn try_consume(&mut self, prefix: &str) -> bool {
        let remaining = &self.input[self.pos..];
        if remaining.starts_with(prefix) {
            self.pos += prefix.len();
            true
        } else {
            false
        }
    }

    fn err_at(&self, pos: usize, msg: &str) -> ParseError {
        let end = (pos + 1).min(self.input.len());
        ParseError {
            span: pos..end,
            message: msg.into(),
        }
    }

    fn err_span(&self, span: Range<usize>, msg: &str) -> ParseError {
        ParseError {
            span,
            message: msg.into(),
        }
    }

    // ─── Top level ──────────────────────────────────────────────────────

    fn parse_chord_expression(
        &mut self,
        current_key: &Scale,
    ) -> Result<ParsedChord, ParseError> {
        let (chord, _explicit_q) = self.parse_main_chord()?;
        self.skip_ws();
        if !self.try_consume("/") {
            return Ok(ParsedChord { chord, bass: None });
        }
        self.skip_ws();
        match self.parse_slash_content(current_key)? {
            SlashOutcome::AbsoluteBass(bass) => Ok(ParsedChord {
                chord,
                bass: Some(bass),
            }),
            SlashOutcome::InKey(scale, span) => Ok(ParsedChord {
                chord: attach_in_key(chord, scale, span)?,
                bass: None,
            }),
        }
    }

    // ─── Main chord (no slash) ──────────────────────────────────────────
    //
    // Returns `(ChordSpec, used_explicit_quality)` — the second
    // element lets the caller's promotion rule know whether to
    // implicitly upgrade Major → Dominant7 when 9/11/13 ext is
    // present.

    fn parse_main_chord(&mut self) -> Result<(ChordSpec, bool), ParseError> {
        self.skip_ws();
        let start = self.pos;
        // Pitch letters (A–G) lead Absolute mode. Roman (I/V/i/v),
        // digit (1–7), and accidental (b/#) lead Functional mode.
        // Roman uses only I/V letters, so A/B/C/D/E/F/G are
        // unambiguously pitch classes.
        let first = self
            .peek()
            .ok_or_else(|| self.err_at(start, "expected chord token"))?;
        match first {
            b'A' | b'B' | b'C' | b'D' | b'E' | b'F' | b'G' => self.parse_absolute_chord(),
            // All digits route to Functional so the inner
            // `degree must be 1–7` error fires for `0`/`8`/`9`.
            b'b' | b'#' | b'I' | b'V' | b'i' | b'v' | b'0'..=b'9' => {
                self.parse_functional_chord()
            }
            other => Err(self.err_at(
                start,
                &format!("unknown leading character '{}'", other as char),
            )),
        }
    }

    // ─── Functional ─────────────────────────────────────────────────────

    fn parse_functional_chord(&mut self) -> Result<(ChordSpec, bool), ParseError> {
        // [Accidental] DegreeToken QualityBlock
        let accidental = self.try_parse_degree_accidental();
        let (degree, case_implied_quality, used_digit) = self.parse_degree_token(accidental)?;
        let _ = used_digit; // currently unused (promotion uses case_implied_quality)
        self.skip_ws();
        let (quality, explicit, exts, alts) =
            self.parse_quality_block(case_implied_quality)?;
        let suffix = ChordSuffix {
            quality,
            extensions: exts,
            alterations: alts,
        };
        Ok((
            ChordSpec::Functional {
                roman: degree,
                suffix,
                in_key: None,
            },
            explicit,
        ))
    }

    fn try_parse_degree_accidental(&mut self) -> Option<Accidental> {
        if self.try_consume("b") {
            Some(Accidental::Flat)
        } else if self.try_consume("#") {
            Some(Accidental::Sharp)
        } else {
            None
        }
    }

    /// Parse a Roman numeral or Arabic digit degree. Returns the
    /// combined `RomanDegree`, the case-implied default quality
    /// (Major / Minor / Diminished), and whether the input used a
    /// digit (Arabic) form.
    fn parse_degree_token(
        &mut self,
        accidental: Option<Accidental>,
    ) -> Result<(RomanDegree, ChordQuality, bool), ParseError> {
        let start = self.pos;
        // Roman numerals, longest-first so `I` doesn't swallow `II`.
        // (literal, degree 1–7, uppercase-major-default).
        let roman_table: &[(&str, u8, bool)] = &[
            ("III", 3, true),
            ("iii", 3, false),
            ("VII", 7, true),
            ("vii", 7, false),
            ("II", 2, true),
            ("ii", 2, false),
            ("IV", 4, true),
            ("iv", 4, false),
            ("VI", 6, true),
            ("vi", 6, false),
            ("I", 1, true),
            ("i", 1, false),
            ("V", 5, true),
            ("v", 5, false),
        ];
        for (lit, degree, uppercase) in roman_table {
            if self.input[self.pos..].starts_with(lit) {
                // Guard: don't let `I` swallow when the next char would
                // form a longer Roman (e.g. `II`, `IV`). The table
                // ordering already handles this by trying longer first.
                self.pos += lit.len();
                let r = combine_degree(*degree, accidental, start)?;
                let q = if *uppercase {
                    ChordQuality::Major
                } else {
                    ChordQuality::Minor
                };
                return Ok((r, q, false));
            }
        }
        // Arabic digit
        if let Some(c) = self.peek()
            && (b'1'..=b'7').contains(&c)
        {
            self.pos += 1;
            let r = combine_degree(c - b'0', accidental, start)?;
            return Ok((r, ChordQuality::Major, true));
        }
        // Out-of-range digit?
        if let Some(c) = self.peek()
            && c.is_ascii_digit()
        {
            return Err(self.err_at(self.pos, "degree must be 1–7"));
        }
        Err(self.err_at(start, "expected degree (I–VII, i–vii, or 1–7)"))
    }

    // ─── Absolute ───────────────────────────────────────────────────────

    fn parse_absolute_chord(&mut self) -> Result<(ChordSpec, bool), ParseError> {
        let start = self.pos;
        let root = self.parse_pitch_class(start)?;
        self.skip_ws();
        let (quality, explicit, exts, alts) = self.parse_quality_block(ChordQuality::Major)?;
        let suffix = ChordSuffix {
            quality,
            extensions: exts,
            alterations: alts,
        };
        Ok((ChordSpec::Absolute { root, suffix }, explicit))
    }

    fn parse_pitch_class(&mut self, span_start: usize) -> Result<PitchClass, ParseError> {
        let letter = self
            .peek()
            .ok_or_else(|| self.err_at(span_start, "expected pitch class"))?;
        let base = match letter {
            b'C' => 0,
            b'D' => 2,
            b'E' => 4,
            b'F' => 5,
            b'G' => 7,
            b'A' => 9,
            b'B' => 11,
            _ => return Err(self.err_at(self.pos, "expected pitch letter A–G")),
        };
        self.pos += 1;
        let offset: i32 = if self.try_consume("b") {
            -1
        } else if self.try_consume("#") {
            1
        } else {
            0
        };
        Ok(PitchClass::from_semitones_mod12(base + offset))
    }

    // ─── Quality block (quality + ext + alt) ────────────────────────────

    fn parse_quality_block(
        &mut self,
        case_implied: ChordQuality,
    ) -> Result<(ChordQuality, bool, Vec<Extension>, Vec<Alteration>), ParseError> {
        let (quality, explicit) = self.parse_quality(case_implied);
        let (extensions, alterations) = self.parse_extensions_and_alterations()?;

        // Promotion: implicit-Major quality + a 7th-implying extension
        // → upgrade. Mirrors `V9` → Dom7+Ninth.
        let promoted = if !explicit && implies_seventh(&extensions) {
            promote_for_seventh(&quality)
        } else {
            quality
        };
        Ok((promoted, explicit, extensions, alterations))
    }

    fn parse_quality(&mut self, case_implied: ChordQuality) -> (ChordQuality, bool) {
        for (suffix, quality) in quality_table() {
            if self.input[self.pos..].starts_with(suffix) {
                self.pos += suffix.len();
                return (quality.clone(), true);
            }
        }
        (case_implied, false)
    }

    fn parse_extensions_and_alterations(
        &mut self,
    ) -> Result<(Vec<Extension>, Vec<Alteration>), ParseError> {
        let mut extensions = Vec::new();
        let mut alterations = Vec::new();
        loop {
            self.skip_ws();
            if let Some(ext) = self.try_parse_extension() {
                extensions.push(ext);
                continue;
            }
            if let Some(alt) = self.try_parse_alteration() {
                alterations.push(alt);
                continue;
            }
            // Lookahead: if next char is `add` or a digit we don't
            // recognize, surface a malformed-extension error rather
            // than letting it fall through and become "trailing input".
            if self.input[self.pos..].starts_with("add") {
                let bad_start = self.pos;
                return Err(self.err_span(
                    bad_start..self.input.len().min(bad_start + 5),
                    "unknown extension (expected add9, add11, add13)",
                ));
            }
            break;
        }
        Ok((extensions, alterations))
    }

    fn try_parse_extension(&mut self) -> Option<Extension> {
        for (suffix, ext) in extension_table() {
            if self.input[self.pos..].starts_with(suffix) {
                self.pos += suffix.len();
                return Some(*ext);
            }
        }
        None
    }

    fn try_parse_alteration(&mut self) -> Option<Alteration> {
        for (suffix, alt) in alteration_table() {
            if self.input[self.pos..].starts_with(suffix) {
                self.pos += suffix.len();
                return Some(*alt);
            }
        }
        None
    }

    // ─── Slash content ──────────────────────────────────────────────────

    fn parse_slash_content(
        &mut self,
        current_key: &Scale,
    ) -> Result<SlashOutcome, ParseError> {
        let start = self.pos;
        let first = self
            .peek()
            .ok_or_else(|| self.err_at(start, "expected slash content"))?;
        match first {
            b'A' | b'B' | b'C' | b'D' | b'E' | b'F' | b'G' => {
                let pc = self.parse_pitch_class(start)?;
                // No quality suffix allowed after slash bass — if
                // there's more input, it's an error.
                self.skip_ws();
                if self.peek().is_some() {
                    return Err(self.err_at(
                        self.pos,
                        "slash bass takes a pitch class only, not a chord",
                    ));
                }
                Ok(SlashOutcome::AbsoluteBass(BassSpec::Absolute(pc)))
            }
            b'b' | b'#' | b'I' | b'V' | b'i' | b'v' | b'1'..=b'7' => {
                let (sub_chord, _) = self.parse_functional_chord()?;
                let span = start..self.pos;
                let scale =
                    build_in_key_scale(&sub_chord, current_key, span.clone())?;
                Ok(SlashOutcome::InKey(scale, span))
            }
            other => Err(self.err_at(
                start,
                &format!(
                    "slash content must be pitch class or degree (got '{}')",
                    other as char
                ),
            )),
        }
    }
}

enum SlashOutcome {
    AbsoluteBass(BassSpec),
    InKey(Scale, Range<usize>),
}

// ─── Tables ──────────────────────────────────────────────────────────────

fn quality_table() -> &'static [(&'static str, ChordQuality)] {
    // Sorted by length descending so longest match wins
    // (e.g. `m7b5` beats `m7`, `maj7` beats `maj`, `mMaj7` beats `m`).
    use ChordQuality::*;
    &[
        ("minMaj7", MinorMajor7),
        ("mMaj7", MinorMajor7),
        ("7sus4", Sus7),
        ("m7b5", HalfDiminished7),
        ("m7♭5", HalfDiminished7),
        ("-7b5", HalfDiminished7),
        ("dim7", Diminished7),
        ("aug7", AugmentedDom7),
        ("maj7", Major7),
        ("Maj7", Major7),
        ("MAJ7", Major7),
        ("sus2", Sus2),
        ("sus4", Sus4),
        ("sus9", Sus9),
        ("min7", Minor7),
        ("min6", Minor6),
        ("°7", Diminished7),
        ("ø7", HalfDiminished7),
        ("Δ7", Major7),
        ("M7", Major7),
        ("m7", Minor7),
        ("-7", Minor7),
        ("m6", Minor6),
        ("-6", Minor6),
        ("+7", AugmentedDom7),
        ("maj", Major),
        ("Maj", Major),
        ("MAJ", Major),
        ("min", Minor),
        ("dim", Diminished),
        ("aug", Augmented),
        ("sus", Sus4),
        ("ø", HalfDiminished7),
        ("Δ", Major7),
        ("°", Diminished),
        ("o7", Diminished7),
        ("o", Diminished),
        ("M", Major),
        ("m", Minor),
        ("-", Minor),
        ("+", Augmented),
        ("7", Dominant7),
        ("6", Major6),
        ("5", Power),
    ]
}

fn extension_table() -> &'static [(&'static str, Extension)] {
    use Extension::*;
    &[
        ("add13", Add13),
        ("add11", Add11),
        ("add9", Add9),
        ("13", Thirteenth),
        ("11", Eleventh),
        ("9", Ninth),
    ]
}

fn alteration_table() -> &'static [(&'static str, Alteration)] {
    use Alteration::*;
    &[
        ("#11", Sharp11),
        ("♯11", Sharp11),
        ("b13", Flat13),
        ("♭13", Flat13),
        ("no3", NoThird),
        ("no5", NoFifth),
        ("b5", Flat5),
        ("♭5", Flat5),
        ("#5", Sharp5),
        ("♯5", Sharp5),
        ("b9", Flat9),
        ("♭9", Flat9),
        ("#9", Sharp9),
        ("♯9", Sharp9),
    ]
}

// ─── Helpers ─────────────────────────────────────────────────────────────

fn combine_degree(
    degree: u8,
    accidental: Option<Accidental>,
    span_start: usize,
) -> Result<RomanDegree, ParseError> {
    use RomanDegree::*;
    let r = match (degree, accidental) {
        (1, None) => I,
        (2, None) => II,
        (3, None) => III,
        (4, None) => IV,
        (5, None) => V,
        (6, None) => VI,
        (7, None) => VII,
        (2, Some(Accidental::Flat)) => FlatII,
        (3, Some(Accidental::Flat)) => FlatIII,
        (5, Some(Accidental::Flat)) => FlatV,
        (6, Some(Accidental::Flat)) => FlatVI,
        (7, Some(Accidental::Flat)) => FlatVII,
        (1, Some(Accidental::Sharp)) => SharpI,
        (2, Some(Accidental::Sharp)) => SharpII,
        (4, Some(Accidental::Sharp)) => SharpIV,
        (5, Some(Accidental::Sharp)) => SharpV,
        (6, Some(Accidental::Sharp)) => SharpVI,
        _ => {
            return Err(ParseError {
                span: span_start..(span_start + 1),
                message: "no enum variant for this accidental + degree combination".into(),
            });
        }
    };
    Ok(r)
}

fn implies_seventh(extensions: &[Extension]) -> bool {
    extensions
        .iter()
        .any(|e| matches!(e, Extension::Ninth | Extension::Eleventh | Extension::Thirteenth))
}

fn promote_for_seventh(quality: &ChordQuality) -> ChordQuality {
    match quality {
        ChordQuality::Major => ChordQuality::Dominant7,
        ChordQuality::Minor => ChordQuality::Minor7,
        other => other.clone(),
    }
}

/// Build the `Scale` for the `in_key` field from a parsed sub-chord.
/// The sub-chord's root degree (interpreted in `current_key`) becomes
/// the new tonic; its quality default picks the mode.
fn build_in_key_scale(
    sub: &ChordSpec,
    current_key: &Scale,
    span: Range<usize>,
) -> Result<Scale, ParseError> {
    let (degree, suffix) = match sub {
        ChordSpec::Functional { roman, suffix, .. } => (roman, suffix),
        ChordSpec::Absolute { .. } => {
            return Err(ParseError {
                span,
                message: "in_key shorthand requires a Roman/digit degree, not a pitch class"
                    .into(),
            });
        }
    };
    let (degree_index, offset) = roman_to_degree_offset(*degree);
    let intervals = current_key.mode.intervals();
    let semitones_from_tonic = intervals
        .get((degree_index - 1) as usize)
        .copied()
        .ok_or_else(|| ParseError {
            span: span.clone(),
            message: "degree out of range for current key's mode".into(),
        })?;
    let total = current_key.tonic.semitones_from_c() as i32
        + semitones_from_tonic as i32
        + offset as i32;
    let tonic = PitchClass::from_semitones_mod12(total);
    let mode = mode_for_quality(&suffix.quality);
    Ok(Scale { tonic, mode })
}

/// Returns (1-indexed-degree, semitone offset). E.g. `FlatVII` →
/// (7, -1); `SharpIV` → (4, +1).
fn roman_to_degree_offset(r: RomanDegree) -> (u8, i8) {
    use RomanDegree::*;
    match r {
        I => (1, 0),
        II => (2, 0),
        III => (3, 0),
        IV => (4, 0),
        V => (5, 0),
        VI => (6, 0),
        VII => (7, 0),
        FlatII => (2, -1),
        FlatIII => (3, -1),
        FlatV => (5, -1),
        FlatVI => (6, -1),
        FlatVII => (7, -1),
        SharpI => (1, 1),
        SharpII => (2, 1),
        SharpIV => (4, 1),
        SharpV => (5, 1),
        SharpVI => (6, 1),
    }
}

fn mode_for_quality(q: &ChordQuality) -> Mode {
    match q {
        ChordQuality::Minor
        | ChordQuality::Minor7
        | ChordQuality::Minor6
        | ChordQuality::MinorMajor7
        | ChordQuality::HalfDiminished7 => Mode::Aeolian,
        ChordQuality::Diminished | ChordQuality::Diminished7 => Mode::Locrian,
        _ => Mode::Ionian,
    }
}

fn attach_in_key(
    outer: ChordSpec,
    scale: Scale,
    span: Range<usize>,
) -> Result<ChordSpec, ParseError> {
    match outer {
        ChordSpec::Functional { roman, suffix, .. } => Ok(ChordSpec::Functional {
            roman,
            suffix,
            in_key: Some(scale),
        }),
        ChordSpec::Absolute { .. } => Err(ParseError {
            span,
            message: "in_key shorthand requires a Functional outer chord, not Absolute".into(),
        }),
    }
}
