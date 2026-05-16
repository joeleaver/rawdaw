//! The realization pass.
//!
//! Walks a `Project` and emits sample-timed MIDI 2.0 events. Pure-functional:
//! same project + same sample rate → same output.
//!
//! v1 covers: all `PitchSpec` variants, all `OctaveSpec` variants, `in_key`
//! tonicization, variant scheduling, voice-leading state (minimal-motion).
//!
//! Deferred to a future iteration: per-note overrides, voicing strategies
//! for chord-block events, humanization, lookahead/range slicing, caching,
//! tempo ramps (only constant BPM is honored).

pub mod event;
mod overrides;
pub mod resolve;
mod variants;

use crate::activation::ActivationEntry;
use crate::chord::{ChordEvent, ChordSpec, ChordSuffix};
use crate::id::{ChordLoopId, NoteId, TrackId, VariantId};
use crate::pattern::{DrumEvent, Pattern, PatternBody, PitchSpec, PitchedEvent};
use crate::pitch::{MidiNote, PitchClass};
use crate::project::Project;
use crate::scale::Scale;
use crate::section::{Section, SectionRef};
use crate::time::{BarRange, Duration, MusicalTime, PPQ};
use crate::track::Track;

use overrides::{apply_override, RealizedEvent};
use variants::{
    effective_activations, effective_chord_loops, effective_duration_bars, effective_scale,
    pitched_track_role, variant_at_bar,
};

pub use event::{Midi2Message, MidiChannel, Provenance, TimedEvent, U16Velocity};

/// Realize the entire arrangement to a time-sorted stream of MIDI 2.0 events.
pub fn realize(project: &Project, sample_rate: u32) -> Vec<TimedEvent> {
    Realizer {
        project,
        sample_rate,
    }
    .realize_all()
}

// ---------- The realizer ----------

struct Realizer<'a> {
    project: &'a Project,
    sample_rate: u32,
}

impl<'a> Realizer<'a> {
    fn realize_all(&self) -> Vec<TimedEvent> {
        let mut out: Vec<TimedEvent> = Vec::new();
        for section_ref in &self.project.arrangement.sections {
            let Some(section) = self.project.sections.get(&section_ref.section) else {
                continue; // dangling section ref
            };
            self.realize_section_ref(section, section_ref, &mut out);
        }
        out.sort_by_key(|e| e.time.as_samples());
        out
    }

    fn realize_section_ref(
        &self,
        section: &Section,
        section_ref: &SectionRef,
        out: &mut Vec<TimedEvent>,
    ) {
        let variant = &section_ref.variant;
        let section_scale = effective_scale(self.project, section, variant);
        let beats_per_bar = self.project.tempo_map.beats_per_bar_at(section_ref.start);

        for (track_id, activation) in effective_activations(section, variant) {
            let Some(track) = self.project.tracks.iter().find(|t| t.id == track_id) else {
                continue;
            };
            let Some(pattern_id) = activation.pattern_ref else {
                continue;
            };
            let Some(pattern) = self.project.patterns.get(&pattern_id) else {
                continue;
            };
            self.realize_activation(
                section,
                section_ref,
                track,
                activation,
                pattern,
                section_scale,
                beats_per_bar,
                out,
            );
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn realize_activation(
        &self,
        section: &Section,
        section_ref: &SectionRef,
        track: &Track,
        activation: &ActivationEntry,
        pattern: &Pattern,
        section_scale: &Scale,
        beats_per_bar: u32,
        out: &mut Vec<TimedEvent>,
    ) {
        let chord_loops = effective_chord_loops(section, &section_ref.variant);
        let total_bars = effective_duration_bars(section, &section_ref.variant);
        let pattern_length = pattern.length();
        if pattern_length.as_ticks() == 0 {
            return;
        }

        let mut voice_leading = VoiceLeadingState::default();

        for bar_index in 0..total_bars {
            let variant_id = variant_at_bar(activation, bar_index, &pattern.default_variant);
            let pattern_items: Vec<(MusicalTime, PatternItem<'_>)> =
                pattern_variant_items(pattern, variant_id);
            if pattern_items.is_empty() {
                continue;
            }

            let bar_start = MusicalTime::ticks(bar_index as i64 * beats_per_bar as i64 * PPQ);
            let bar_end =
                MusicalTime::ticks((bar_index + 1) as i64 * beats_per_bar as i64 * PPQ);
            let bar_duration = beats_per_bar as i64 * PPQ;
            let iterations = (bar_duration / pattern_length.as_ticks()).max(1);

            for iter in 0..iterations {
                let iter_start = bar_start
                    + MusicalTime::ticks(iter * pattern_length.as_ticks());
                if iter_start >= bar_end {
                    break;
                }
                for &(item_time, item) in &pattern_items {
                    let event_time_in_section = iter_start + item_time;
                    if event_time_in_section >= bar_end {
                        break;
                    }
                    let absolute_time = section_ref.start + event_time_in_section;
                    self.realize_pattern_item(
                        section_ref,
                        track,
                        pattern,
                        variant_id,
                        activation,
                        &chord_loops,
                        section_scale,
                        item,
                        absolute_time,
                        beats_per_bar,
                        &mut voice_leading,
                        out,
                    );
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn realize_pattern_item(
        &self,
        section_ref: &SectionRef,
        track: &Track,
        pattern: &Pattern,
        variant: &VariantId,
        activation: &ActivationEntry,
        chord_loops: &[(BarRange, ChordLoopId)],
        section_scale: &Scale,
        item: PatternItem<'_>,
        absolute_time: MusicalTime,
        beats_per_bar: u32,
        voice_leading: &mut VoiceLeadingState,
        out: &mut Vec<TimedEvent>,
    ) {
        match item {
            PatternItem::Pitched(e) => {
                let role = pitched_track_role(track);
                let role_anchor = resolve::role_register(role);
                let chord = self.chord_at_time(
                    chord_loops,
                    absolute_time - section_ref.start,
                    beats_per_bar,
                );
                let Some(midi) = resolve_pitched(
                    e,
                    section_scale,
                    chord.as_ref(),
                    voice_leading.last_pitch,
                    role_anchor,
                ) else {
                    return;
                };
                // Voice-leading state updates regardless of override: the muted
                // note "would have been" this pitch, so subsequent chromatic /
                // nearest events reason as if it played.
                voice_leading.last_pitch = Some(midi);

                let mut realized = RealizedEvent::from_pitched(midi, e, absolute_time);
                apply_override(&activation.per_note_overrides, e.note_id, &mut realized);
                if realized.muted {
                    return;
                }
                self.emit_event(section_ref, track, pattern, variant, e.note_id, realized, out, MidiChannel::default());
            }
            PatternItem::Drum(e) => {
                let Some(midi) = resolve::gm_drum_note(&e.voice) else {
                    return;
                };
                let mut realized = RealizedEvent::from_drum(midi, e, absolute_time);
                apply_override(&activation.per_note_overrides, e.note_id, &mut realized);
                if realized.muted {
                    return;
                }
                self.emit_event(section_ref, track, pattern, variant, e.note_id, realized, out, MidiChannel::DRUMS);
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn emit_event(
        &self,
        section_ref: &SectionRef,
        track: &Track,
        pattern: &Pattern,
        variant: &VariantId,
        note_id: NoteId,
        realized: RealizedEvent,
        out: &mut Vec<TimedEvent>,
        channel: MidiChannel,
    ) {
        let provenance = Provenance {
            pattern: pattern.id,
            variant: variant.clone(),
            event_note_id: note_id,
            override_applied: realized.override_id,
            section: section_ref.id,
        };
        let velocity = U16Velocity::from_u7(realized.velocity);
        self.push_note_pair(
            out,
            track.id,
            realized.note,
            velocity,
            channel,
            realized.time,
            realized.duration,
            provenance,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn push_note_pair(
        &self,
        out: &mut Vec<TimedEvent>,
        target: TrackId,
        note: MidiNote,
        velocity: U16Velocity,
        channel: MidiChannel,
        start: MusicalTime,
        duration: Duration,
        provenance: Provenance,
    ) {
        let start_sample = self
            .project
            .tempo_map
            .musical_to_sample(start, self.sample_rate);
        let end_sample = self
            .project
            .tempo_map
            .musical_to_sample(start + duration.0, self.sample_rate);

        out.push(TimedEvent {
            time: start_sample,
            target,
            message: Midi2Message::NoteOn {
                channel,
                note,
                velocity,
            },
            provenance: provenance.clone(),
        });
        out.push(TimedEvent {
            time: end_sample,
            target,
            message: Midi2Message::NoteOff {
                channel,
                note,
                velocity,
            },
            provenance,
        });
    }

    fn chord_at_time(
        &self,
        chord_loops: &[(BarRange, ChordLoopId)],
        time_in_section: MusicalTime,
        beats_per_bar: u32,
    ) -> Option<ResolvedChord> {
        let bar_ticks = beats_per_bar as i64 * PPQ;
        let bar = (time_in_section.as_ticks() / bar_ticks) as u32;
        let (range, loop_id) = chord_loops.iter().find(|(r, _)| r.contains(bar))?;
        let chord_loop = self.project.chord_loops.get(loop_id)?;
        if chord_loop.length.as_ticks() == 0 {
            return None;
        }
        let range_start_ticks = range.start as i64 * bar_ticks;
        let offset_in_range = time_in_section.as_ticks() - range_start_ticks;
        let offset_in_loop = offset_in_range.rem_euclid(chord_loop.length.as_ticks());
        let event = chord_loop
            .events
            .iter()
            .rev()
            .find(|e| e.time.as_ticks() <= offset_in_loop)?;
        Some(self.resolve_chord_event(event))
    }

    fn resolve_chord_event(&self, event: &ChordEvent) -> ResolvedChord {
        let (root, suffix) = match &event.chord {
            ChordSpec::Functional {
                roman,
                suffix,
                in_key,
            } => {
                let scale = in_key.as_ref().unwrap_or(&self.project.default_key);
                (resolve::resolve_chord_root(*roman, scale), suffix.clone())
            }
            ChordSpec::Absolute { root, suffix } => (*root, suffix.clone()),
        };
        ResolvedChord { root, suffix }
    }
}

// ---------- Per-event helpers ----------

#[derive(Clone, Copy)]
enum PatternItem<'a> {
    Pitched(&'a PitchedEvent),
    Drum(&'a DrumEvent),
}

fn pattern_variant_items<'a>(
    pattern: &'a Pattern,
    variant: &VariantId,
) -> Vec<(MusicalTime, PatternItem<'a>)> {
    let mut out = Vec::new();
    match &pattern.body {
        PatternBody::Pitched(b) => {
            if let Some(events) = b.variants.get(variant) {
                for e in events {
                    out.push((e.time, PatternItem::Pitched(e)));
                }
            }
        }
        PatternBody::Drum(b) => {
            if let Some(events) = b.variants.get(variant) {
                for e in events {
                    out.push((e.time, PatternItem::Drum(e)));
                }
            }
        }
    }
    out
}

fn resolve_pitched(
    event: &PitchedEvent,
    section_scale: &Scale,
    chord: Option<&ResolvedChord>,
    last_pitch: Option<MidiNote>,
    role_anchor: MidiNote,
) -> Option<MidiNote> {
    match &event.spec {
        PitchSpec::Scale { degree, octave } => {
            let pc = resolve::resolve_scale_degree(*degree, section_scale);
            resolve::pick_octave(pc, *octave, last_pitch, role_anchor)
        }
        PitchSpec::Chord { degree, octave } => {
            let chord = chord?;
            let pc = resolve::resolve_chord_degree(*degree, chord.root, &chord.suffix)?;
            resolve::pick_octave(pc, *octave, last_pitch, role_anchor)
        }
        PitchSpec::Absolute {
            pitch_class,
            octave,
        } => resolve::absolute_pitch(*pitch_class, *octave),
        PitchSpec::Chromatic {
            semitones_from_prev,
        } => {
            let last = last_pitch.unwrap_or(role_anchor).get() as i32;
            let n = (last + *semitones_from_prev as i32).clamp(0, 127) as u8;
            MidiNote::new(n)
        }
        PitchSpec::Rest => None,
    }
}

// ---------- Resolved chord ----------

/// The chord in effect at some point in time. Pulled out of `ChordSpec` so the
/// realization pass works against a uniform shape rather than the
/// Functional/Absolute split.
#[derive(Debug, Clone)]
pub struct ResolvedChord {
    pub root: PitchClass,
    pub suffix: ChordSuffix,
}

// ---------- Voice-leading state ----------

#[derive(Debug, Default, Clone)]
struct VoiceLeadingState {
    last_pitch: Option<MidiNote>,
}
