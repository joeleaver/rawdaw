//! Test-only fixtures shared across `pattern_actions` test modules.

use rawdaw_model::id::{NoteId, PatternId};
use rawdaw_model::pattern::{OctaveSpec, PitchSpec, PitchedEvent};
use rawdaw_model::pitch::{Octave, PitchClass, U7};
use rawdaw_model::project::Project;
use rawdaw_model::scale::{Scale, ScaleDegree};
use rawdaw_model::time::{Duration, MusicalTime};

use super::create_pitched_pattern;

pub(super) const DEFAULT_PATTERN_BARS: i64 = super::DEFAULT_PATTERN_BARS;
pub(super) const DEFAULT_BEATS_PER_BAR: u32 = super::DEFAULT_BEATS_PER_BAR;

pub(super) fn empty_project() -> Project {
    Project::new(Scale::major(PitchClass::C))
}

pub(super) fn pitched_event(id: NoteId, time_ticks: i64, degree: u8) -> PitchedEvent {
    PitchedEvent {
        note_id: id,
        time: MusicalTime::ticks(time_ticks),
        duration: Duration::ticks(240),
        velocity: U7::HALF,
        articulation: None,
        humanization: Default::default(),
        spec: PitchSpec::Scale {
            degree: ScaleDegree::new(degree),
            octave: OctaveSpec::Anchored(Octave(3)),
        },
    }
}

pub(super) fn project_with_empty_pitched_pattern() -> (Project, PatternId) {
    let mut project = empty_project();
    let pid = create_pitched_pattern(&mut project);
    (project, pid)
}
