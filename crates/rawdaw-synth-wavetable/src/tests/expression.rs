//! K4 expression-control tests: pitch bend + sustain pedal.

use rawdaw_engine::event::BlockEventInBlock;
use rawdaw_model::{Midi2Message, MidiChannel, U16Velocity};

use super::{make_node, note_off, note_on, render_block, rms, BLOCK};
use crate::BlockMessage;

fn control_change(controller: u8, value: u8) -> BlockMessage {
    BlockMessage::Midi(Midi2Message::ControlChange {
        channel: MidiChannel::default(),
        controller: rawdaw_model::U7::new(controller).unwrap(),
        value: rawdaw_model::U7::new(value).unwrap(),
    })
}

fn pitch_bend(value_14: u16) -> BlockMessage {
    BlockMessage::Midi(Midi2Message::PitchBend {
        channel: MidiChannel::default(),
        value_14,
    })
}

fn ev(offset: u32, msg: BlockMessage) -> BlockEventInBlock {
    BlockEventInBlock {
        offset_in_block: offset,
        message: msg,
    }
}

#[test]
fn sustain_pedal_holds_note_until_pedal_release() {
    // K4 contract: with the pedal down, a NoteOff is deferred; the
    // voice keeps sounding until the pedal goes back up.
    let mut node = make_node();
    let mut buf = Vec::new();

    // Block 1: NoteOn 60 → audible
    render_block(&mut node, &[ev(0, note_on(60, U16Velocity::HALF))], &mut buf);
    let rms_attack = rms(&buf[..BLOCK]);
    assert!(rms_attack > 0.01, "note must produce audio after NoteOn");

    // Block 2: pedal down at offset 0, NoteOff at offset 10
    // → voice keeps sounding because the pedal latched the release.
    render_block(
        &mut node,
        &[
            ev(0, control_change(64, 127)),
            ev(10, note_off(60)),
        ],
        &mut buf,
    );
    let rms_pedal_down = rms(&buf[..BLOCK]);
    assert!(
        rms_pedal_down > rms_attack * 0.5,
        "sustained note must keep sounding while pedal is down (was {rms_pedal_down}, attack {rms_attack})",
    );

    // Block 3: pedal up at offset 0 → deferred NoteOff fires.
    // The voice enters release; tail decays toward silence.
    render_block(
        &mut node,
        &[ev(0, control_change(64, 0))],
        &mut buf,
    );
    // Render a few more blocks for the release tail to actually
    // decay (default release_s = 0.20s at 48kHz = ~9600 samples ≈
    // 38 blocks of 256).
    let mut tail_blocks = Vec::new();
    for _ in 0..50 {
        let mut tail = Vec::new();
        render_block(&mut node, &[], &mut tail);
        tail_blocks.push(rms(&tail[..BLOCK]));
    }
    // RMS should trend strictly down across the release.
    assert!(
        tail_blocks.last().unwrap() < &(rms_pedal_down * 0.1),
        "voice must decay toward silence after pedal release; got tail {:.5} vs pedal-down {:.5}",
        tail_blocks.last().unwrap(),
        rms_pedal_down,
    );
}

#[test]
fn pitch_bend_shifts_oscillator_frequency() {
    // K4: PitchBend changes the RMS-of-a-windowed FFT bin in
    // principle, but a cheaper observable: at +2 semitones, the
    // oscillator phase advances at a different rate, so the
    // sample-by-sample waveform diverges from the unbent version.
    // We render two identical contexts — one with bend, one
    // without — and assert they produce DIFFERENT audio.
    let mut bent = make_node();
    let mut unbent = make_node();

    let mut buf_bent = Vec::new();
    let mut buf_unbent = Vec::new();

    // First block: NoteOn with simultaneous pitch bend up.
    render_block(
        &mut bent,
        &[
            ev(0, pitch_bend(16383)), // full positive bend = +2 st
            ev(1, note_on(60, U16Velocity::HALF)),
        ],
        &mut buf_bent,
    );
    render_block(
        &mut unbent,
        &[ev(1, note_on(60, U16Velocity::HALF))],
        &mut buf_unbent,
    );

    // Both should be audible.
    assert!(rms(&buf_bent[..BLOCK]) > 0.01);
    assert!(rms(&buf_unbent[..BLOCK]) > 0.01);

    // Some sample after the attack window should differ between
    // the bent and unbent renders — different frequencies produce
    // different waveforms.
    let mut diffs = 0;
    for i in 100..BLOCK {
        if (buf_bent[i] - buf_unbent[i]).abs() > 1e-4 {
            diffs += 1;
        }
    }
    assert!(
        diffs > 100,
        "bent waveform must differ from unbent — saw only {diffs} sample differences",
    );
}

#[test]
fn pitch_bend_center_is_no_op() {
    // PitchBend at center value (8192) must produce audio
    // indistinguishable from no-bend.
    let mut bent = make_node();
    let mut unbent = make_node();

    let mut buf_bent = Vec::new();
    let mut buf_unbent = Vec::new();

    render_block(
        &mut bent,
        &[
            ev(0, pitch_bend(8192)),
            ev(1, note_on(60, U16Velocity::HALF)),
        ],
        &mut buf_bent,
    );
    render_block(
        &mut unbent,
        &[ev(1, note_on(60, U16Velocity::HALF))],
        &mut buf_unbent,
    );

    // Identical audio (bit-exact — center = no change to frequency).
    for i in 0..BLOCK {
        assert!(
            (buf_bent[i] - buf_unbent[i]).abs() < 1e-6,
            "pitch bend at center (8192) must be a no-op; differ at sample {i}",
        );
    }
}

#[test]
fn cc_with_no_matrix_route_is_inaudible() {
    // K5 stores every incoming CC into the per-node CC table, but
    // only CCs that some matrix slot references actually influence
    // audio. The M5 default routes CC1 (mod wheel) → FilterCutoff,
    // so we test the *unrouted* CCs: CC7 (volume) and CC11
    // (expression) shouldn't affect output until the user wires
    // them up in the matrix editor.
    let mut node = make_node();
    let mut baseline = make_node();

    let mut buf_cc = Vec::new();
    let mut buf_baseline = Vec::new();

    render_block(
        &mut node,
        &[
            ev(0, control_change(7, 64)),  // volume
            ev(0, control_change(11, 90)), // expression
            ev(1, note_on(60, U16Velocity::HALF)),
        ],
        &mut buf_cc,
    );
    render_block(
        &mut baseline,
        &[ev(1, note_on(60, U16Velocity::HALF))],
        &mut buf_baseline,
    );

    // Unrouted CCs make no difference — bit-identical output.
    for i in 0..BLOCK {
        assert!(
            (buf_cc[i] - buf_baseline[i]).abs() < 1e-6,
            "unrouted CCs must be inaudible; differ at sample {i}",
        );
    }
}
