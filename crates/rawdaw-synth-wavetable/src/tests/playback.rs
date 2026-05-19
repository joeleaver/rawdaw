//! Basic playback behaviour: silence pre-note-on, audible during
//! sustain, releases to silence, L/R mirror, mid-block offset.

use rawdaw_engine::event::BlockEventInBlock;
use rawdaw_model::U16Velocity;

use super::{make_node, note_off, note_on, render_block, rms, BLOCK};

#[test]
fn silent_until_note_on() {
    let mut node = make_node();
    let mut buf = Vec::new();
    render_block(&mut node, &[], &mut buf);
    assert!(buf.iter().all(|s| *s == 0.0));
}

#[test]
fn note_on_produces_audible_output() {
    let mut node = make_node();
    let mut buf = Vec::new();
    // Run a few blocks so the amp envelope has time to ramp through
    // attack + into sustain; the first 5ms of attack at 48 kHz is
    // ~240 samples (well inside one 256-frame block, so output
    // builds quickly).
    render_block(
        &mut node,
        &[BlockEventInBlock {
            offset_in_block: 0,
            message: note_on(60, U16Velocity::HALF),
        }],
        &mut buf,
    );
    // Render a second block to skip the initial attack ramp window.
    render_block(&mut node, &[], &mut buf);
    assert!(
        rms(&buf[..BLOCK]) > 0.01,
        "expected audible sustained signal; got rms {}",
        rms(&buf[..BLOCK]),
    );
}

#[test]
fn note_off_releases_to_silence() {
    let mut node = make_node();
    let mut buf = Vec::new();
    render_block(
        &mut node,
        &[BlockEventInBlock {
            offset_in_block: 0,
            message: note_on(60, U16Velocity::HALF),
        }],
        &mut buf,
    );
    // NoteOff, then run a long tail — at 200 ms release, ~9600
    // samples are needed, so render 50 blocks of silence.
    render_block(
        &mut node,
        &[BlockEventInBlock {
            offset_in_block: 0,
            message: note_off(60),
        }],
        &mut buf,
    );
    for _ in 0..50 {
        render_block(&mut node, &[], &mut buf);
    }
    // The last block should be effectively silent — release has
    // completed.
    let tail_rms = rms(&buf[..BLOCK]);
    assert!(
        tail_rms < 1e-4,
        "release should reach silence; tail rms = {tail_rms}",
    );
}

#[test]
fn lr_outputs_are_identical() {
    // v0 mirrors L = R; spatialization is a v1 growth.
    let mut node = make_node();
    let mut buf = Vec::new();
    render_block(
        &mut node,
        &[BlockEventInBlock {
            offset_in_block: 0,
            message: note_on(60, U16Velocity::HALF),
        }],
        &mut buf,
    );
    let l = &buf[..BLOCK];
    let r = &buf[BLOCK..2 * BLOCK];
    for i in 0..BLOCK {
        assert_eq!(l[i], r[i], "L and R must match in v0 at sample {i}");
    }
}

#[test]
fn mid_block_note_on_starts_at_offset() {
    let mut node = make_node();
    let mut buf = Vec::new();
    render_block(
        &mut node,
        &[BlockEventInBlock {
            offset_in_block: 100,
            message: note_on(60, U16Velocity::HALF),
        }],
        &mut buf,
    );
    // Pre-onset samples must be exactly 0. Post-onset attack ramps
    // through ~240 samples — most of the rest of the block — and
    // amp_level starts at 0, so individual sample magnitudes ramp
    // up from 0 too. Confirm that no pre-onset sample is non-zero.
    let left = &buf[..BLOCK];
    for (i, s) in left[..100].iter().enumerate() {
        assert_eq!(*s, 0.0, "pre-onset sample {i} should be silent");
    }
    // Make sure something happens post-onset across two blocks of
    // amp ramp-up.
    render_block(&mut node, &[], &mut buf);
    assert!(
        rms(&buf[..BLOCK]) > 0.005,
        "should be audible after the attack ramp",
    );
}
