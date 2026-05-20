//! Round-trip cases — `parse(input)` then `format(...)` must
//! produce the canonical column. Covers the canonical-form
//! contract in `grammar.md`'s "Aliases" block plus the standard
//! rows where input is already canonical.

use super::round_trip;

#[test]
fn round_trip_functional_basic() {
    round_trip("I", "I");
    round_trip("i", "i");
    round_trip("V", "V");
    round_trip("v", "v");
    round_trip("5", "V");
    round_trip("5m", "v");
    round_trip("bVI", "bVI");
    round_trip("#IV", "#IV");
    round_trip("bVII", "bVII");
}

#[test]
fn round_trip_qualities() {
    round_trip("V7", "V7");
    round_trip("Vmaj7", "Vmaj7");
    round_trip("Vm7", "vm7");
    round_trip("vm7", "vm7");
    round_trip("Vsus4", "Vsus4");
    round_trip("Vsus2", "Vsus2");
    round_trip("V7sus4", "V7sus4");
    round_trip("V5", "V5");
    round_trip("V6", "V6");
    round_trip("Vm6", "vm6");
    round_trip("vii°", "viidim");
    round_trip("vii°7", "viidim7");
    round_trip("viim7b5", "viim7b5");
}

#[test]
fn round_trip_extensions_alterations() {
    round_trip("Iadd9", "Iadd9");
    round_trip("V9", "V9");
    round_trip("V13", "V13");
    round_trip("bVImaj7add9", "bVImaj7add9");
    round_trip("V7b9", "V7b9");
    round_trip("V7#11", "V7#11");
    round_trip("V7no5", "V7no5");
}

#[test]
fn round_trip_secondary_dominants() {
    round_trip("V/V", "V/V");
    round_trip("5/5", "V/V");
    round_trip("V/v", "V/v");
    round_trip("V7/IV", "V7/IV");
    round_trip("vii°7/V", "viidim7/V");
    round_trip("ii/bVII", "ii/bVII");
    round_trip("V/bIII", "V/bIII");
}

#[test]
fn round_trip_absolute_bass() {
    round_trip("V/B", "V/B");
    round_trip("V/Bb", "V/A#");
    round_trip("Imaj7/E", "Imaj7/E");
    round_trip("bVI/F#", "bVI/F#");
}

#[test]
fn round_trip_absolute_mode() {
    round_trip("C", "C");
    round_trip("Cm", "Cm");
    round_trip("Cmaj7", "Cmaj7");
    round_trip("F#m7", "F#m7");
    round_trip("Bbm7b5", "A#m7b5");
    round_trip("G7b9", "G7b9");
    round_trip("Cmaj7/E", "Cmaj7/E");
    round_trip("Cmaj7add9#11", "Cmaj7add9#11");
}

#[test]
fn round_trip_aliases_normalize_to_canonical() {
    round_trip("C-", "Cm");
    round_trip("Cmin", "Cm");
    round_trip("CM7", "Cmaj7");
    round_trip("CΔ", "Cmaj7");
    round_trip("Cø", "Cm7b5");
    round_trip("Cm7♭5", "Cm7b5");
}
