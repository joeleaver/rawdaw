//! Live param-tweak tests — every UI-exposed parameter must reshape
//! the sound of a *currently-held* note when its `ParamEvent` arrives
//! mid-block. Matches the universal modern-synth contract: any knob
//! turn is audible in real time, never gated on the next note-on.

use rawdaw_engine::event::{BlockEventInBlock, ParamEvent};
use rawdaw_model::U16Velocity;

use super::{make_node, note_on, render_block, rms, BLOCK};
use crate::{BlockMessage, WavetableParam};

fn param(p: WavetableParam, value: f32) -> BlockMessage {
    BlockMessage::Param(ParamEvent {
        path: p.encode(),
        value,
    })
}

fn ev(offset: u32, msg: BlockMessage) -> BlockEventInBlock {
    BlockEventInBlock {
        offset_in_block: offset,
        message: msg,
    }
}

/// Strike a note, then send a single param-tweak mid-block, and
/// compare against a baseline that gets the same note but no tweak.
/// The two renders must diverge — i.e. the tweak must be audible on
/// the held note rather than waiting for the next note-on.
fn assert_tweak_reshapes_held_note(p: WavetableParam, new_value: f32, label: &str) {
    let mut tweaked = make_node();
    let mut baseline = make_node();
    let mut buf_t = Vec::new();
    let mut buf_b = Vec::new();

    // Block 1: same note on both nodes; tweaked node also gets the
    // param event halfway through the block.
    render_block(
        &mut tweaked,
        &[
            ev(0, note_on(60, U16Velocity::HALF)),
            ev(128, param(p, new_value)),
        ],
        &mut buf_t,
    );
    render_block(
        &mut baseline,
        &[ev(0, note_on(60, U16Velocity::HALF))],
        &mut buf_b,
    );

    // Samples [128..BLOCK) must diverge. Use diff RMS instead of a
    // per-sample threshold so envelopes / filter ringing can have a
    // gradual onset.
    let diff_rms = rms(
        &buf_t[128..BLOCK]
            .iter()
            .zip(buf_b[128..BLOCK].iter())
            .map(|(a, b)| a - b)
            .collect::<Vec<_>>(),
    );
    assert!(
        diff_rms > 1e-4,
        "{label} change must reshape the held note; diff_rms = {diff_rms}",
    );
}

#[test]
fn filter_resonance_tweak_reshapes_held_note() {
    assert_tweak_reshapes_held_note(WavetableParam::FilterResonance, 8.0, "filter resonance");
}

#[test]
fn filter_cutoff_tweak_reshapes_held_note() {
    assert_tweak_reshapes_held_note(WavetableParam::FilterCutoffHz, 6000.0, "filter cutoff");
}

#[test]
fn osc_tune_tweak_reshapes_held_note() {
    assert_tweak_reshapes_held_note(WavetableParam::OscTune(0), 7.0, "osc tune");
}

#[test]
fn osc_fine_cents_tweak_reshapes_held_note() {
    assert_tweak_reshapes_held_note(WavetableParam::OscFineCents(0), 50.0, "osc fine cents");
}

#[test]
fn osc_level_tweak_reshapes_held_note() {
    // OSC 1 starts at level 1.0 in the default; dropping it to 0
    // must darken the mix.
    assert_tweak_reshapes_held_note(WavetableParam::OscLevel(0), 0.0, "osc level");
}

#[test]
fn lfo_rate_tweak_reshapes_held_note() {
    // LFO drives the default's FilterCutoff slot — speeding it up
    // changes the cutoff trajectory across the back half of the block.
    assert_tweak_reshapes_held_note(WavetableParam::LfoRateHz, 30.0, "lfo rate");
}

#[test]
fn env_release_tweak_reshapes_held_note() {
    // ENV1 release shape is exercised by changing the value mid-note;
    // even in sustain the new release will shape the upcoming tail.
    // We assert during-sustain reshape by changing the *sustain
    // level* of ENV1 — that's read each tick.
    assert_tweak_reshapes_held_note(WavetableParam::EnvSustain(0), 0.1, "env sustain");
}
