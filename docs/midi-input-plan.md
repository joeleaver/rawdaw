# MIDI input plan (v1)

Live MIDI input — plug in a USB MIDI keyboard, play notes, hear them
through the synth. v1 also routes incoming CCs into the mod matrix
so external knobs can drive any matrix-routable parameter.

The work splits into six phases. K1 alone satisfies the immediate
ask ("play notes and hear them"); K2 / K3 polish the UX; K4 / K5
add expressive control (pitch bend, sustain, full CCs).

## Status

- K0 ✅ — this document. Updated 2026-05-19 with K1.fix design
  revisions (time=0 scheduling, all-states node processing).
- K1 ✅ — landed 2026-05-19. End-to-end verified with Joe's KeyLab
  MkII 49.
  - `ab8ba9c` core midir wiring + dedicated MIDI input SPSC queue.
  - `8f9cf5a` skip ALSA 'Midi Through' loopback in auto-pick + log
    devices.
  - `d0959a8` K1.fix: graph runs in all transport states (was
    silenced in Stopped/Paused, so the boot state Stopped had no
    live MIDI). Schedule live MIDI at `SampleTime::samples(0)`
    (was racing against Stopped's clock reset at sample_clock+1).
  - `ad99682` K1 diagnostic: `RAWDAW_MIDI_DEBUG=1` traces every
    incoming MIDI message for keyboard / translator debugging.
  - `6b4effe` K1.fix2: `VoicePool::note_off` releases all matching
    voices (was bailing after the first match; voices in Release
    stage are still `is_active()`, so the second press would
    orphan voice 1 in Sustain when the second NoteOff hit
    already-releasing voice 0).
- K4 ✅ — landed 2026-05-19. Pitch bend wheel + sustain pedal
  (CC64) routed through the wavetable synth. Drum synth ignores
  both (unpitched one-shots). `Midi2Message` model grew
  `ControlChange { channel, controller: U7, value: U7 }` and
  `PitchBend { channel, value_14: u16 }` variants. Pitch bend
  range = ±2 semitones (MIDI default; future RPN handling can
  widen). Sustain pedal latches NoteOffs while down; releases
  the deferred queue on pedal up. midir translator parses
  status `0xB_` + `0xE_`. 4 wavetable-side regression tests
  (pedal latching, pitch bend shifts frequency, center is a
  no-op, non-sustain CCs ignored) + translator tests (CC, CC64,
  pitch bend center, pitch bend max).
- K2 ✅ + K3 ✅ — landed together 2026-05-19. Device picker
  dropdown in top bar (`MidiPicker` reads
  `AudioResources::available_midi_inputs` + `current_midi_device`,
  calls `set_midi_device(Option<&str>)` on selection). Sticky
  routing target via new `AppState::midi_target_track` signal +
  matching `AudioResources::midi_target: Arc<AtomicU32>`; an
  Effect in `main_window` propagates signal changes to the
  atomic, midir's callback reads the atomic on every event.
  TracksPane shows a "♪" badge on the active MIDI target row
  (sticky against section-block selection). `audio/mod.rs` was
  at 707 lines; split out `audio/midi.rs` (247 lines) for all
  MIDI-input AudioResources impls before adding K2/K3.
- K5 — not started.

## Phase K0 — Plan + design decisions ◀ this doc

Lock the architectural choices below. No code changes. The decisions
are detailed in "Design decisions locked in K0" near the bottom.

**Done when:** This doc lands on `main`.

## Phase K1 — `midir` + dedicated MIDI input SPSC queue + auto-pick

The minimum that satisfies *"play notes and hear them"*. No UI yet
(K2); fixed lead-track routing (K3); notes only (K4 / K5 extend).

**Engine side** (`rawdaw-engine`):

- New `MidiInputHandle` type. Holds a *second* SPSC `Producer<BlockEvent>`
  separate from `EngineHandle::event_tx`. `Send + !Sync` — owned by
  whichever thread is pushing MIDI input (typically `midir`'s
  callback thread).
- `Engine::new` builds the second queue alongside the existing one.
- `Engine::split` returns `(AudioEngine, EngineHandle, MidiInputHandle)`.
- `AudioEngine::process_block` drains both event queues and merges
  them into the per-block event list. Sort by `time` is already
  applied to the merged set; MIDI events scheduled at `sample_clock
  + 1` (the new convention for MIDI in) land in the next block.

**App side** (`rawdaw-app`):

- New `midir = "0.10"` dep (latest at write time).
- New module `crates/rawdaw-app/src/midi_input/mod.rs`:
  - `list_input_ports() -> Vec<MidiInputPort>` — wraps midir's enumeration.
  - `auto_pick() -> Option<MidiInputPort>` — picks the first port.
  - `open(port, handle, routing) -> MidiInputConnection` — captures
    the MidiInputHandle + a routing target into midir's callback.
    Callback parses each incoming message, translates to a
    `BlockEvent`, pushes directly via the handle.
- Message translation: `midir` exposes raw 3-byte MIDI 1.0 (status
  + 2 data bytes). v1 handles status-byte `0x9_` (Note On with
  velocity > 0), `0x8_` / `0x9_` with velocity 0 (Note Off).
  Velocity widens to `U16Velocity::from_u7`.
- `AudioResources::build` calls `auto_pick` + `open` and stores the
  `MidiInputConnection` on the struct so dropping AudioResources
  closes the port.
- Sample-accurate timing: MIDI events fire at
  `sample_clock.load() + 1` so they land in the next block. Latency
  ceiling = one block (≤ 5.4 ms at 256/48k).
- Routing for K1: hardcoded to the lead-track NodeId (first
  `Pitched` track). K3 makes this dynamic.

**Tests:**

- midir message parse (note-on / note-off / velocity widening).
- MidiInputHandle round-trip: push → drain → block event.
- Engine block events show two-queue merge (one event from each
  queue at the same time → sorted by node, both delivered).

**Done when:** Plug in a USB MIDI keyboard, run rawdaw, play a key,
hear the lead synth. Tested visually + audibly with the rinch MCP
and an actual MIDI device. Workspace test count grows by ~8.

## Phase K2 — Device picker UI

Top bar gains a `[MIDI: device-name ▾]` dropdown listing available
ports + a "None" option.

- `AppState.midi_device_name: Signal<Option<String>>`. `None`
  means disconnected; `Some(name)` means a port is open.
- AudioResources gains an interior-mutable slot for the current
  `MidiInputConnection` (Rc<RefCell<Option<MidiInputConnection>>>)
  + a `set_midi_device(name: Option<&str>)` method that tears
  down the existing connection and opens the new one.
- `regions/topbar.rs` gets a new `MidiPicker` component — uses
  Rinch's built-in `Select` like the matrix editor does.
- Auto-picked device on boot is shown as selected in the dropdown
  (`midi_device_name` is seeded by `auto_pick().name()`).
- Styling stays placeholder per
  [project-ui-redesign-pending] memory.

**Done when:** Dropdown lists available ports; switching tears
down + opens the new port; notes route through the new port.
Workspace test count grows by ~3.

## Phase K3 — Sticky routing target

Per Joe's spec: route to selected track when one is selected,
otherwise fall back to the last-edited track. Specifically:

- `AppState.midi_target_track: Signal<Option<usize>>`.
- `select_track(Some(idx))` sets *both* `selected_track` and
  `midi_target_track` to `Some(idx)`.
- `select_track(None)` clears `selected_track` but **preserves**
  `midi_target_track` — so clicking a section block keeps MIDI
  playing the synth you were just editing.
- `set_selected_idx(Some(_))` doesn't touch `midi_target_track` —
  same reason.
- MIDI bridge resolves the target on every event:
  `midi_target_track.get().unwrap_or(first_pitched_track_idx)`.
- TracksPane shows a small "♪" badge on the active MIDI target
  row.

**Done when:** Selecting a track lights it as the MIDI target;
clicking a section block doesn't change the target; the badge
visually tracks where MIDI is going. Workspace test count grows
by ~4 (mutex coverage extends).

## Phase K4 — Pitch bend + sustain pedal (CC64)

The first chunk of "controllers". Pitch bend is per-note expression;
sustain pedal latches notes. Together they cover the common
keyboard-performance gestures beyond note on/off.

**Model side** (`rawdaw-model`):

- `Midi2Message` grows `ControlChange { channel, controller: U7,
  value: U7 }` and `PitchBend { channel, value_14: u16 }` variants.
- Existing realize / translate paths pass these through unchanged.

**Synth side** (`rawdaw-synth-wavetable`, `rawdaw-synth-drum`):

- `Voice` trait gains `set_pitch_bend_semitones(f32)`. Wavetable
  voice applies the offset to its oscillator frequencies.
- Synth node maintains a per-channel pitch-bend state. Pitch-bend
  events update the state; subsequent voices read it at note-on,
  active voices update on every event.
- Sustain pedal: synth node tracks pedal state per channel. With
  pedal down, note-off events are queued (note enters "released
  but held" state). On pedal release, all queued note-offs fire.

**App side:**

- midir bridge translates 0xB_ (control change) + 0xE_ (pitch
  bend) into the new Midi2Message variants.

**Done when:** Pitch bend wheel detunes held notes audibly;
sustain pedal latches notes audibly. Workspace test count grows
by ~10.

## Phase K5 — All CCs → mod matrix

Forward every CC (0..127) into the mod-matrix source side, so
external knobs can drive any matrix-routable parameter.

**DSP side** (`rawdaw-dsp::modulation`):

- `ModSource` grows `MidiCC(u8)` variant. Modulation evaluation
  reads from a per-node `MidiCcState [f32; 128]` (normalized
  0..1 per controller).
- Encode/decode in `WavetableParam` / `DrumParam` covers the new
  variant. Matrix editor's source dropdown lists the CCs (subset
  for v1: the common keyboard controllers — mod wheel CC1,
  breath CC2, volume CC7, expression CC11, pan CC10; full 0..127
  picker is a UI follow-on).

**Synth side:**

- Synth nodes maintain `MidiCcState`. Incoming CC events update
  the state at sample-accurate offsets within a block (so a slow
  CC sweep within a block is honored, not snapped to block
  boundaries).
- Mod-matrix evaluation passes the CC state to slot eval.

**App side:**

- midir bridge already passes CCs through (K4); routing here is
  on the synth side.

**Done when:** A mod-matrix slot with source = MidiCC(1) (mod
wheel) → destination FilterCutoff sweeps the cutoff when the
mod wheel moves on the MIDI keyboard. Verified visually +
audibly via the rinch MCP. Workspace test count grows by ~15.

---

## Design decisions locked in K0

### `midir` for cross-platform MIDI input.

The de-facto Rust MIDI library. Wraps ALSA on Linux (we test
here), CoreMIDI on macOS, WinMM on Windows. No competition in
the Rust ecosystem at this maturity.

Considered: directly binding ALSA (Linux-only, more surface area
for us to maintain). `coremidi-rs`, `winmidi-rs` (single-platform).

### Dedicated MIDI SPSC queue in the engine, not a shared Mutex.

The existing `EngineHandle::event_tx` is a single-producer rtrb
queue. The UI thread owns it. Sharing it with `midir`'s callback
thread requires either (a) wrapping in `Arc<Mutex<>>` and breaking
the SPSC contract for marginal lock contention, or (b) adding a
second SPSC queue dedicated to MIDI input.

**Pick (b).** Engine surgery is small (~50 lines: extra queue +
extra split-return + extra drain at process_block start), and the
SPSC contract stays clean. The audio thread merges both queues
into the per-block event list, sort by `time` (already applied).
This is the rule #1 / CLAUDE.md choice — pay the small extra cost
to keep the architecture honest.

`MidiInputHandle` is `Send`. midir's callback closure captures it
by move; midir spawns its own thread for the callback. No mpsc
intermediary needed.

### Live MIDI schedules at `SampleTime::samples(0)`, NOT `sample_clock + 1`.

**Revised in K1.fix** after Joe testing surfaced a transport-state
race. The original plan said `sample_clock + 1`; that broke under
Stop → Play transitions because midir would read `sample_clock` at
its pre-Stop value, push an event at that high time, and then the
audio thread would reset `sample_clock` to 0 — leaving the event
stuck above `block_end` forever.

The fix: push live MIDI events at `SampleTime::samples(0)`. The
engine's partition step computes
`offset_in_block = saturating_sub(time, ctx.absolute_time)`. With
`time = 0`, this always saturates to 0, so live MIDI events fire
at offset 0 of the next block they're partitioned into. The
block-window check `time < block_end` is also always true since
`block_end ≥ 1`, so events always drain on the next block
regardless of transport state.

What's lost: sample-accuracy within a block. A human key-press is
already "now-ish" — sub-block latency (≤ 5.4 ms at 256/48k) isn't
perceivable. K4/K5 features that *do* want sample-accurate
scheduling (e.g. a multi-CC sweep within a block) can still opt
in by reading `sample_clock` themselves.

### Transport-state model: graph runs in all states; transport gates only song events + clock.

**Revised in K1.fix.** The original engine model short-circuited
node processing in Paused / Stopped (output silenced, nodes
skipped). That made live MIDI silent outside Playing — a fatal
UX issue for "play notes through the synth".

New contract:

- **Playing**: song events drain, all nodes process, output is
  live, sample clock advances.
- **Paused**: song events stay queued (resume from where you left
  off), all nodes still process so live MIDI is audible, sample
  clock is frozen.
- **Stopped**: song events drain (host re-arms before next Play),
  all nodes still process so live MIDI is audible, sample clock
  forced back to 0.

Live MIDI is **unconditional** — the dedicated MIDI input queue
drains and partitions in every block in every transport state.
Pressing a key on a MIDI keyboard always makes sound.

### Routing target = sticky last-track-edited.

Joe's spec — "selected track, unless you're editing the synth,
then the synth you're editing." Interpretation:
`midi_target_track` is updated on every track selection, preserved
on section-block selection / deselection. Falls back to the first
Pitched track when nothing has been selected yet.

This means:
- Click drum track → MIDI plays drums.
- Click section block → MIDI still plays drums (last-track-edited
  is preserved).
- Click pitched track → MIDI plays that synth.

Considered: per-track arm toggle (Ableton-style — multiple armed
tracks, layering). Cleaner mental model but more UI state per
track. Worth doing once the UI redesign pass lands; sticky
last-edited is the smaller v1 that matches the editing-and-
auditioning workflow.

### CC routing reuses the existing mod-matrix.

The synth's mod matrix already routes signals to destinations.
The natural place for CCs is as a new `ModSource` variant.
This means CC routing is *editable from the matrix editor* —
matrix slot source dropdown gains "Mod Wheel", "Volume", etc.,
and the existing source-string-round-trip / encode-decode
infrastructure carries over.

Considered: a separate "MIDI mapping" pane parallel to the
matrix. Cleaner conceptually but duplicates the matrix's slot
metaphor. Reusing the matrix means MIDI-driven destinations and
internal-source destinations sum into the same modulation bag
(audibly: CC + LFO can both push the same filter cutoff).

### CC scope: subset of the 128 CCs surfaced in UI, all 128 stored.

The synth node stores all 128 CC values. The matrix-editor source
dropdown lists a *curated* subset for v1: Mod Wheel (1), Breath
(2), Volume (7), Pan (10), Expression (11). A "Full picker"
follow-on lets users pick any CC by number; for v1 the common
keyboard CCs are enough.

Considered: dropdown lists all 128 names. Too noisy. Curated
subset matches the matrix editor's existing "Source / Destination"
dropdown vocabulary.

### MIDI device selection is per-host, not per-project.

A `.rawdaw` project shouldn't pin "the user's MIDI device" — that
moves between machines. Selection lives on the host (AppState
signal) and isn't serialized. Future: persist in a separate
user-preferences file.

### Out of process for v1: MIDI output, MPE, polyphonic aftertouch.

- **MIDI Out** — sending MIDI from rawdaw to external gear. Out
  of scope; bigger story (routing, channel allocation, hardware
  sync).
- **MPE** (MIDI Polyphonic Expression) — multi-channel
  per-note bend/pressure. The synth voice model would need
  per-note state for bend/aftertouch. Out of scope.
- **Polyphonic aftertouch** — same per-note-state story. Out of
  scope.
- **MIDI clock / SPP** — sync to external transport. Out of
  scope; the engine has its own transport.
- **Velocity curves / hardware-specific quirks** — a velocity
  curve UI is a polish piece for a later session.

---

## Future plans needed

- **MIDI Out** — sending MIDI from rawdaw. Likely lives in
  `rawdaw-midi-output` mirroring `midir` output.
- **MPE / polyphonic aftertouch** — voice-model extension. Big
  enough to deserve its own plan.
- **MIDI Learn UI** — right-click a slider → "Learn" → next CC
  binds. Picklist item #3 from `project-status`. Builds on K5's
  CC infrastructure.
- **MIDI device persistence** — remember the last-used device
  across sessions. Lands with the host-preferences file.

## Out of scope (deferred)

- MIDI 2.0 (UMP packets, per-note controllers) — `midir` exposes
  MIDI 1.0; our internal `Midi2Message` is already-named for
  forward compat but doesn't yet exploit MIDI 2.0 wire format.
- Channel filtering (route channel 1 to one track, channel 2 to
  another). Multi-channel routing is a per-track-arm thing;
  deferred with arm.
- Bus-level MIDI FX (arpeggiator, chord mode). Pre-synth MIDI
  processing; future feature.
