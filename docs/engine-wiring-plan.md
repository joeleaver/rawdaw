# Engine-wiring milestone plan

Replace `rawdaw-app`'s static UI fixture with a real `rawdaw_model::Project`,
translate the project's realized events into engine commands at launch, surface
the engine's playhead reactively, and hook the top-bar transport buttons.

**Engineering constraints** (from `CLAUDE.md`).

- Architectural correctness over shortcuts. Always.
- Unlimited time and budget — pay the cost of doing it right.
- ~700-line cap per source file. Split by concern when approached.
- No `unwrap()` outside tests / proven-impossible cases. No silent swallowing.
- UI work uses the `rinch:rinch` skill and the `rinch` MCP server.

**Cadence.** Each phase ends with `cargo run -p rawdaw-app` showing concrete
progress, clippy clean (default + `--no-default-features` + `--features
cpal-driver`), and `cargo test --workspace` green.

---

## Phase E0 — Plan + fixture-sharing decision ✅ this doc

**Goal.** Document the phased approach and lock the cross-crate
fixture-sharing decision before any code moves.

**Decision: public `fixtures` module on `rawdaw-model`, gated behind a
`fixtures` Cargo feature.**

Alternatives considered.

- **Separate `rawdaw-fixtures` crate.** Cleanest for crate boundaries but
  adds a third crate that exists only to host `Project` constructors. The
  fixtures live on top of `rawdaw-model` types; a thin wrapper crate is
  bureaucratic without a clear separation gain.
- **rawdaw-app builds its own `Project` independently.** Keeps the model
  crate purely a library, but duplicates the fixture-construction work
  between rawdaw-app (runtime fixture) and rawdaw-model's `tests/common/`
  (test fixtures). The point of this milestone is to *eliminate* the
  duplication.

The public-module-with-feature-gate path:

- `rawdaw-model/src/fixtures/` exposes `build_tiny_project()` (the existing
  test fixture, moved out of `tests/common/`) and `build_round1_project()`
  (new — mirrors the current UI fixture).
- Cargo feature `fixtures` gates the module. Test binaries (`tests/`)
  enable it via `[dev-dependencies] rawdaw-model = { ..., features =
  ["fixtures"] }`; rawdaw-app enables it via a regular `features = [
  "fixtures"]` so the fixture data ships in the binary.
- The existing `tests/common/mod.rs` becomes a one-line re-export so the
  existing test imports keep working.

**Done when.** This file is committed and references it elsewhere
(`docs/design/README.md`, the project_status memory) point at it.

---

## Phase E1 — Move existing fixture + add `build_round1_project()` ✅ done

**Goal.** Land the cross-crate fixture sharing decided in E0. Build the
real `Project` equivalent of the rawdaw-app UI fixture.

**Steps.**

- Add `fixtures` Cargo feature to `rawdaw-model` (no default features
  added — release builds stay lean).
- Move `tests/common/mod.rs` → `src/fixtures/mod.rs` under
  `#[cfg(feature = "fixtures")]`. Rename `build_tiny_project()` to
  `build_tiny_project()` (no change) and re-export from the crate root.
  Update `tests/common/mod.rs` to a one-line `pub use
  rawdaw_model::fixtures::build_tiny_project;` shim (or delete the file
  and update test imports).
- Add `build_round1_project()` — builds a `Project` whose
  patterns / chord-loops / sections / arrangement match the current
  `rawdaw-app::fixture::round1()` UI fixture. Five sections (intro,
  verse base, verse stripped, verse base, chorus) across 24 bars, four
  tracks (bass / lead / drums / pad), patterns `bass-main`, `lead-main`,
  `drums-main` (with `fill` variant), `pad-bed`.
- Add fixture-invariant tests pinning the round-1 shape: arrangement
  length, section / track / pattern counts, the chorus drums `fill`
  schedule entry, the stripped variant overrides.

**Done when.**

- `cargo test --workspace` green; the new `build_round1_project` tests
  pass.
- `cargo clippy --workspace --all-targets --features fixtures` and `cargo
  clippy --workspace --all-targets` both clean (with-feature and
  without-feature builds).
- rawdaw-app still compiles and renders the same UI (still using its own
  static fixture — E2 wires the real one through).

**Deviations from the original plan.**
- **Returned `(Project, Round1Keys)` tuple.** Plan said
  `build_round1_project() -> Project`; the actual signature exposes a
  `Round1Keys` bundle of every named id (tracks / patterns / sections /
  chord-loops / drum-kit) so downstream callers don't have to re-scan
  by name. The engine routing in Phase E3 needs `TrackId`s by name —
  this avoids a parallel lookup table.
- **`tests/common/mod.rs` kept as a re-export shim**, not deleted —
  the existing test imports (`mod common; common::build_tiny_project()`)
  keep working without touching `smoke.rs` / `roundtrip.rs`. Removing
  it would have been spec-pure but would have meant a parallel test
  rename.
- **Two-variant bass pattern.** The UI library lists `bass-main` as
  `Pitched · 2 variants`; the model's pattern body now carries `main`
  and `alt` (identical content) to match. Round-2 schedule UI doesn't
  exercise the `alt` variant — it's just there for the library count.
- **Pattern bodies are illustrative, not musical.** Each variant has
  1–4 events — enough for realize() to produce events Phase E3 can
  count, but deliberately not composition decisions. Real bass /
  melody / drum content will land when the round-3 pattern editor
  lets us *make* it.
- **`VariantId::new("__silent__")` placeholder for the silent
  sub-range.** The current model treats every `(BarRange, VariantId)`
  schedule entry as naming a real variant; the UI fixture's
  `(3..4, None)` shape (sub-range silence) doesn't yet have a
  first-class model representation. Using a sentinel variant id is
  the smallest viable stand-in; revisit when sub-range silences get
  a proper model type (likely round 3 alongside the schedule editor).

---

## Phase E2 — Presentation adapter; rawdaw-app reads the real Project ✅ done

**Goal.** Drive the UI's `Round1` view from the model's
`build_round1_project()` plus a small presentation overlay carrying the
UI-only fields the model doesn't own (colors, library meta strings,
round-2 cell realization decorations, top-bar transient state).

**What landed.**

- `rawdaw-app/Cargo.toml` enables the `fixtures` feature on
  `rawdaw-model`. The crate now ships the round-1 model fixture in
  the binary.
- `rawdaw-app/src/fixture/mod.rs` keeps the UI's view types
  (`Track`, `Pattern`, `ChordLoop`, `Section`, `Activation`,
  `Arrangement`) — but with `String` / `Vec` fields instead of
  `&'static str` / `&'static [_]` (Phase E2 step 1).
- `rawdaw-app/src/fixture/data.rs` is the adapter: calls
  `build_round1_project()` once, walks the resulting `Project` plus
  an inline `Overlay` keyed by model ids, and produces the UI's
  `Round1`. `OnceLock<Round1>` caches the result so every component
  pulls the same `&'static Round1`.
- `rawdaw-app/src/fixture/chord_naming.rs` carries the
  Roman / absolute / quality-suffix display helpers (split out to
  keep both files under the ~700-line cap).
- `rawdaw-model/src/fixtures/round1.rs` intro section now includes
  `pattern_ref: None` silent activations for bass / lead / drums so
  the round-1 inspector activation table can render the `silent`
  pill (vs the dashed `inherit` placeholder reserved for tracks
  absent from a section). New `silent_activation` helper + test.

**Deviations from the original plan.**

- **Kept the module name `fixture`** (plan said rename to
  `presentation`). The module is unambiguously an adapter now; the
  rename would have churned every import site without clarifying
  intent.
- **UI view types preserved.** Plan implied the UI would consume
  `rawdaw_model::*` types directly + an overlay. In practice the UI's
  `Track` / `Pattern` / etc. carry display-derived fields (role
  display labels, "Pitched · N variants" meta strings, asterisks
  computed against role defaults) that aren't 1:1 with model types.
  The adapter populates the UI types from model data; cleaner than
  threading model + overlay refs through every component.
- **`OnceLock` caches the full UI `Round1`**, not just the model
  `Project`. The adapter runs once on first call; subsequent
  `round1()` calls return a `&'static Round1` directly.
- **Cell realization decorations stay in the overlay.** The model's
  `RealizationParams` carries voicing + a u8-jitter humanization
  scalar; the UI's `Realization` carries voicing + octave +
  fractional humanization. The mapping isn't 1:1 yet — until the
  model grows richer realization fields, the per-cell display
  values live in the overlay keyed by
  `(SectionId, variant-name, TrackId)`.
- **Pinned-note counts flow through the overlay** but the UI's
  cell footer still reads a hardcoded `0` in `identity_column.rs`
  (a Phase 6 stub). The plumbing is in place; surfacing the real
  count is a follow-up.
- **Visible regression in intro's activation table.** Phase 8
  showed `bass · bass-main · silent` / `lead · lead-main · silent`
  / `drums · drums-main · silent` in the round-1 inspector when
  intro was selected. Post-E2 the rows still render and the
  `silent` pill is preserved, but no pattern name is shown — the
  model's `pattern_ref: None` state doesn't bind a pattern. The
  three round-2 artboards exercised in Phase 8 (verse base / verse
  stripped / chorus base) are visually identical; default render
  (verse@bar5 selected) matches exactly.
- **The `__silent__` variant id sentinel survived.** Phase E1
  flagged it as needing a first-class model representation; the
  adapter still translates it back to `Option::None` for the UI's
  schedule entries. Resolution deferred to a later round.

**Verification.**

- 30 rawdaw-app unit tests pass — new tests pin the intro
  silent-vs-active shape, chord-name resolution in C major
  (`["I","V","vi","IV"]` → `["C","G","Am","F"]`), and the
  five-step / 24-bar arrangement.
- `cargo test --workspace` green.
- Clippy clean across default / `--no-default-features` /
  `--features cpal-driver` builds.
- Visual verification via the rinch MCP: default arrangement
  view + inspector match Phase 8; verse / verse@stripped / chorus
  blocks render the expected variant chips and inspector
  activation tables; section editor for verse (artboards A + B)
  and chorus (artboard C) shows identical layout, realization
  values, and variant schedules to Phase 8.

---

## Phase E3 — Engine instantiation + per-track sine node graph ✅ done

**Goal.** At app launch, build a `rawdaw_engine::Engine`, set up an audio
graph with a per-track instrument node (`SineNode` for now), translate
the realized events from the Project, and feed them into the engine's
event queue. **No audio thread yet.** The engine is ready to render but
nothing is calling `process_block`.

**What landed.**

- `rawdaw-engine/src/nodes/mixer.rs` — new `MixerNode`: N stereo
  inputs declared at construction → 1 stereo summing output. Sized
  per construction call so a 4-track round-1 project gets a 4-input
  mixer, etc. Tests pin zero / one / many inputs and the descriptor
  channel shape.
- `rawdaw-app/src/audio.rs` — new `AudioResources` Rinch store
  (`Rc<RefCell<Engine>>` + `NodeId master` + `Rc<TrackRouting>` +
  initial event count). `AudioResources::build()` calls
  `build_round1_project()`, installs the mixer at `NodeId(0)` plus
  one `SineNode` at `NodeId(i+1)` per track in a single
  `GraphCommand::Batch` (one topo recompute), then runs
  `realize() → translate_events() → push_event` to arm the engine.
- `rawdaw-app/src/app.rs` — `main_window` installs the
  `AudioResources` store alongside `AppState` at app launch.
- `rawdaw-app/src/main.rs` — `audio` module added; architecture
  doc-comment updated.

**Deviations from the original plan.**

- **`Rc<RefCell<Engine>>` instead of bare `Engine` in the store.**
  rinch's `create_store<T: Clone + 'static>` requires the type to be
  cheaply cloneable. The engine isn't `Clone`; wrapping it in
  `Rc<RefCell<>>` gives the store a Clone shape without contention
  overhead (single-threaded UI). Phase E4 splits the engine — at
  that point the `AudioEngine` moves to the audio thread and only
  the `EngineHandle` remains here.
- **`Rc<TrackRouting>` instead of inline `TrackRouting`.** Same
  reason: keep the store cheap to clone. The routing is immutable
  after build.
- **Aggregate type is named `AudioResources`, not `BuiltEngine`.**
  The plan called the aggregate `BuiltEngine`; the actual struct
  carries more than just the engine (routing, event count) and the
  store is consumed via `use_store::<AudioResources>()`, so the
  more descriptive name reads better at call sites.
- **No UI handler consumes the store yet.** The plan says "the
  handle stays accessible from the UI layer." For E3 that's a
  preparation step — phases E4 (cpal callback), E5 (playhead read),
  and E6 (transport buttons) are the actual consumers. `_audio = …`
  binds the store builder; the fields are marked `#[allow(dead_code)]`
  pending those phases.
- **Master is the mixer.** The plan said "first cut can be a custom
  `MixerNode` that's the topo-root" — that's what landed. No
  passthrough alternative.
- **Sample rate hardcoded to 48 kHz, block size 256.** Matches the
  engine's `tests/render.rs`. Phase E4 picks up the actual sample
  rate from the opened cpal device.

**Verification.**

- `audio::tests::round_1_pushes_one_block_event_per_realized_event`
  is the milestone done-when's cross-check: builds the resources,
  realizes the model project independently, asserts
  `initial_event_count == realize(&project).len()`. Companion tests
  pin every track routes to a sine, the NodeId layout
  (master=0, sines=1..=N), and that the built engine renders offline
  without panicking.
- 113 workspace tests pass (rawdaw-app 34, rawdaw-engine 29
  including 4 new mixer tests, rawdaw-model 12 + 21 integration,
  plus crate-tests).
- Clippy clean across default / `--no-default-features` /
  `--features cpal-driver`.
- Rinch MCP visual verification: UI unchanged from post-E2 state.
  App launches; engine builds without panicking on the real
  project.

**Out-of-scope, flagged for later:**

- The realize pass currently does not exercise `PatternBody::Chord`
  block events (model design exists, no implementation). Round-1
  fixture doesn't reach this code path, so it isn't blocking — but
  E4 might want to widen the cross-check test to include a project
  that does.

---

## Phase E4 — cpal driver + audio thread wiring ✅ done

**Goal.** Replace the in-process `Rc<RefCell<Engine>>` from E3 with a
real cpal audio thread. `engine.split()` hands the `AudioEngine` to
`CpalDriver`; the host-side `EngineHandle` stays in the rinch store
ready for the UI to push commands and events.

**What landed.**

- `rawdaw-engine/src/cpal_driver.rs`: `CpalDriver::new` now takes an
  `error_handler: FnMut(StreamError) + Send + 'static` closure
  (resolving the engine README's acknowledged-debt note about
  stderr-only error paths). `cpal::StreamError` is re-exported so
  downstream crates don't need a direct cpal dep.
- `rawdaw-app/Cargo.toml`: `rawdaw-engine` dep gains
  `features = ["cpal-driver"]`. The app crate hard-requires the
  driver — there's no headless-only build path for rawdaw-app
  itself.
- `rawdaw-app/src/audio.rs`: rewritten for the cpal driver.
  Probes `CpalDriver::probe_default_sample_rate` (falls back to a
  48 kHz constant when no device available), builds the engine at
  that rate, runs the E3-style graph + event push, splits into
  AudioEngine + EngineHandle, hands the AudioEngine to
  `CpalDriver::new` with an `mpsc::Sender<String>` as the error
  callback. On cpal-open failure the resources still build — the
  UI runs silent, `audio_enabled()` reports `false`, and
  `play`/`pause` become no-ops.
- The cpal stream is **paused** at construction. Phase E6 wires the
  transport button to `AudioResources::play()`.
- `AudioResources::next_stream_error()` exposes a non-blocking
  `try_recv` over the error queue; future UI work surfaces these
  as a banner.

**Deviations from the original plan.**

- **Engine transport flag not added.** The plan said "engine starts
  with transport paused; audio thread runs but outputs silence."
  E4 instead leaves the cpal stream itself paused (callback not
  invoked) — semantically equivalent for E4's done-when (no audio
  output), and avoids an engine-side transport state machine that
  E6 will need anyway. When E6 adds proper Playing/Paused/Stopped
  semantics, calling `driver.play()` will start the callback and
  the engine's transport state will gate event consumption.
- **Fallback to silent when no audio device.** Plan didn't address
  the headless-test case; the audio module now gracefully falls
  back when `CpalDriver::new` fails. Tests for `AudioResources`
  build successfully on systems without an audio device (the
  channel-receiver still works for the test surface).
- **String for stream errors, not `StreamError`.** Host wraps the
  cpal error in `to_string()` before sending — keeps `cpal` out of
  rawdaw-app's public type surface and lets a future UI banner
  display a human-readable message directly.
- **No xrun counter exposed.** The plan suggested verifying "no
  buffer underruns on a quiet system" via a `RenderResult`-style
  counter; the engine's `RenderResult` is offline-only and the
  realtime path doesn't yet surface xrun stats. Deferred — fold
  into a future engine-side change when xruns become measurable.

**Verification.**

- 118 workspace tests pass (35 rawdaw-app + 33 rawdaw-engine + 12
  rawdaw-model + integration). New
  `audio::tests::stream_errors_queue_is_empty_at_startup` test
  exercises the receiver.
- Clippy clean across default / `--no-default-features` /
  `--features cpal-driver` builds.
- Rinch MCP visual verification: UI renders identically. No
  panics; the audio thread initializes silently (cpal probe
  succeeds on the dev machine; the stream stays paused).

---

## Phase E5 — Reactive playhead ✅ done

**Goal.** Drive the arrangement view's playhead from the engine's
actual transport position instead of the hardcoded bar 5 beat 2.

**What landed.**

- `rawdaw-engine::audio_engine::AudioEngine` gained an
  `Arc<AtomicU64> sample_clock`. At the end of every `process_block`
  the engine stores `absolute_time_samples + frames` with
  `Ordering::Release`. Exposed via
  `AudioEngine::sample_clock()` and the convenience
  `Engine::sample_clock()` delegator. Allocation-free; works
  uniformly across the realtime cpal path and `render_offline`.
- `rawdaw-model::TempoMap::sample_to_musical` — inverse of
  `musical_to_sample` under the same constant-BPM assumption. Returns
  `MusicalTime::ZERO` for a zero sample rate (defensive).
- `rawdaw-app::audio::AudioResources` gained four UI-facing fields:
  `sample_clock: Arc<AtomicU64>` (the engine's atomic, shared),
  `playhead_samples: Signal<u64>`, `tempo_map: TempoMap` (cloned
  from the project), and `_poller: Option<Rc<PlayheadPoller>>`.
- `PlayheadPoller` — a `std::thread` named
  `rawdaw-playhead-poller` that loops at 16 ms (~60 Hz), reads the
  atomic, and `Signal::send`s the new value through rinch's
  registered cross-thread dispatcher when it changes. Drop sets a
  stop flag (`Arc<AtomicBool>`) and joins the thread. Attached only
  by `AudioResources::build()` (production); the test-facing
  `build_from_project_and_rate()` leaves `_poller = None` because
  unit tests run outside the rinch runtime, where `Signal::send`
  from a background thread would panic.
- `AudioResources::playhead_position()` — shared helper that reads
  the signal once and returns `PlayheadPosition { bar, beat,
  bars_f64 }`. Used by both `regions/arrangement.rs::playhead_percent`
  (sub-bar percent positioning) and `regions/topbar.rs` (bar / beat
  readout). Calling it inside an rsx attribute closure subscribes
  the closure to the signal via the macro's effect-tracker, so the
  surgical DOM update happens without any imperative re-render.

**Done when (met).**

- App launches; engine sample clock + UI signal both read 0.
- Arrangement playhead sits at the left edge (bar 1, sample 0).
- Top-bar readout shows "BAR 1 · BEAT 1" — consistent with the
  arrangement.
- `cargo test --workspace` green (128 tests). New tests:
  `rawdaw_engine::sample_clock_advances_with_render_offline`,
  `rawdaw_engine::sample_clock_handles_are_shared`, five
  `rawdaw_model::tempo::tests::*` cases pinning the
  sample ↔ musical round-trip, and three `rawdaw_app::audio::tests::*`
  cases for the new AudioResources fields.
- Clippy clean across default / `--no-default-features` /
  `--features cpal-driver`.
- Rinch MCP visual: identical to post-E4 layout; the playhead has
  moved from "bar 5 beat 2" (the round-1 fixture's static
  `playhead_bar` / `playhead_beat`) to bar 1 — the engine is now
  the source of truth, the fixture's display fields are unused.

**Deviations from the original plan.**

- **Engine owns the atomic, not the driver.** The plan listed two
  options ("`CpalDriver::new` gains an `Arc<AtomicU64> sample_clock`
  parameter" vs "engine takes the clock and updates it from within
  `process_block`"). We took the engine-side path the plan flagged
  as cleaner — it works uniformly for `render_offline` (used by the
  engine's own test surface) and the cpal callback, and it avoids
  duplicating the absolute-time accounting that the engine already
  has via `ProcessContext::absolute_time_samples`. The cpal driver's
  per-callback `absolute_time` local counter is unchanged.
- **`std::thread` poller, not a Rinch `Effect` on a frame timer.**
  The plan suggested "a small `Effect` polls the atomic on a frame
  timer (target ~60Hz)." Rinch `Effect`s are dependency-triggered,
  not time-triggered, and the framework has no public frame-tick
  API yet — so the poller is a plain `std::thread` that uses
  `Signal::send` (Rule 11 in the rinch skill: cross-thread updates
  use `send()`, not `set()`). Rinch's main-thread dispatcher,
  registered by `run_rinch_*`, routes the send to the UI thread.
- **Top-bar readout wired alongside the arrangement playhead.**
  Plan only mentioned `regions/arrangement.rs::playhead_percent`,
  but leaving the top-bar BAR/BEAT readout reading the fixture's
  static `playhead_bar` / `playhead_beat` would have meant the
  readout disagreed with the visible playhead. Both now flow
  through the same `AudioResources::playhead_position()` helper.
  The fixture's `playhead_bar` / `playhead_beat` fields are now
  unused at runtime; left in the round-1 fixture data structure
  for now since they're not in the way.
- **Poller attached at `build()`, skipped at
  `build_from_project_and_rate()`.** Unit tests don't run inside
  the rinch runtime, so the cross-thread dispatcher isn't
  registered — `Signal::send` would panic. The constructor split
  makes the test path safe by construction: production calls
  `build()` which adds the poller, tests call the lower-level
  entry which doesn't.

**Re-evaluation point.** The 60 Hz polling thread is the engine
plan's explicit anti-pattern. When rinch grows a native
"audio-thread → UI signal" bridge (or a frame-tick API the host
can subscribe to), `PlayheadPoller` should be ripped out and
replaced. The bridge is independent of rawdaw work; track it
through Rinch's roadmap.

---

## Phase E6 — Play / pause / stop wiring ✅ done

**Goal.** The top bar's transport buttons drive the engine's playback
state. Pressing play starts the audio engine consuming events from the
queue; pause halts the consumption; stop resets the transport to
sample 0.

**What landed.**

- New `rawdaw-engine::transport` module with a `Transport` enum
  (`Stopped` / `Paused` / `Playing`) and a cloneable
  `TransportHandle` wrapping `Arc<AtomicU8>`. Pack stable; `Stopped`
  is `0` so `AtomicU8::default()` lands on the safest state (silent,
  clock at 0) without an explicit init step.
- `AudioEngine` carries a `TransportHandle` and reads it at the top
  of every `process_block`:
  - **Playing**: existing path.
  - **Paused**: drain commands, do NOT drain events, zero master
    output, leave `sample_clock` alone.
  - **Stopped**: drain commands, drain ALL queued events, zero
    master output, force `sample_clock = 0`.
  The drain-on-Stopped is a bounded-time loop (queue capped by
  `event_queue_capacity`).
- `Engine::render_offline` flips transport into `Playing` for the
  duration of the render and restores the prior state on exit, so
  the offline render path stays decoupled from whatever state a
  caller left the engine in (default `Stopped` would otherwise
  silence every render).
- `CpalDriver`'s f32 stream callback consults transport per block:
  resets its local `absolute_time` to 0 on Stopped; freezes it on
  Paused; advances on Playing. `playing: bool` in the
  `ProcessContext` mirrors the transport (Playing iff
  `Transport::Playing`).
- `AudioResources` exposes `play()` / `pause()` / `stop()`. `play()`
  re-arms the cached realized events when transitioning out of
  Stopped (the engine drained the queue on the way in), then sets
  transport = Playing and calls `driver.play()` idempotently.
  `pause()` and `stop()` just flip the atomic; the cpal stream
  keeps running so the audio thread can drain commands while
  paused / stopped. `realized_events: Rc<Vec<BlockEvent>>` is the
  cached translation that gets re-pushed.
- `regions/topbar.rs` `TransportBtn` gains an `onclick: Callback`
  prop (no-op default for the disabled Record button). The Play
  button is a toggle: pressing while Playing pauses, otherwise
  plays. Stop / Rewind both call `audio.stop()`. A `pause` glyph
  was added to `parts::Icon` for future visual feedback (not yet
  used reactively — see deviations).

**Done when (met for unit-testable surface).**

- Engine transport tests pin: Stopped silences + drains events;
  Paused silences + holds clock; Playing produces audio. (3 new
  tests in `crates/rawdaw-engine/tests/render.rs`.)
- AudioResources tests pin: starts Stopped; walks
  Stopped → Playing → Paused → Playing → Stopped via the public
  API; `realized_events` matches `initial_event_count`. (2 new
  tests in `audio::tests`.)
- 137 workspace tests green.
- Clippy clean across default / `--no-default-features` /
  `--features cpal-driver`.
- Rinch MCP visual: app launches, layout identical, click events
  on the Play button reach the handler. Audible-output
  verification could NOT be demonstrated on this dev machine —
  cpal opens but the ALSA backend reports the slave device
  unavailable (`audio_enabled() == false`), so callbacks never
  run and the playhead doesn't visually advance. On a machine
  with a working f32 cpal device, the unit-tested logic should
  drive: Play → audible sine + advancing playhead; Pause →
  silence + frozen playhead; Stop → silence + playhead at bar 1;
  Play after Stop → audible from bar 1.

**Deviations from the original plan.**

- **Transport state via `Arc<AtomicU8>`, not a GraphCommand queue.**
  Plan offered both options ("Transition commands flow through the
  existing `GraphCommand` queue (or a sibling `TransportCommand`
  queue if conflating audio-graph mutations with transport state
  would be surprising)"). The atomic was cleaner: the cpal driver
  needs to consult transport per-callback to manage its
  `absolute_time` counter, and an SPSC command queue is the wrong
  shape for that read pattern. The host writes via
  `TransportHandle::set` (lock-free, wait-free); the audio thread
  loads via `TransportHandle::get`. Graph mutations still flow
  through the existing command queue.
- **`Stopped` is the default, not `Playing`.** Matches user
  perception (transport bar shows Bar 1, no audio) and the safest
  initial state. `render_offline` flips to `Playing` internally so
  the offline test surface is unaffected.
- **No dedicated `Pause` button — Play is a toggle.** The round-1
  mockup has Rewind / Play / Stop / Record but no Pause button.
  Plan suggested a spacebar shortcut for play/pause; the Play
  button is the simplest equivalent for first cut. Spacebar
  shortcut still deferred — see below.
- **Spacebar shortcut deferred.** Plan called it out ("lift from
  round-1 follow-up list if `keyboard shortcuts` was on it — wire
  just this one"). Rinch's keyboard-event API for global
  shortcuts is unfamiliar to me; rather than guess, leaving this
  for E7 / round-3 follow-up so it gets the proper investigation.
- **Play button glyph is static `"play"`.** A reactive glyph
  (showing `pause` while Playing) requires either a dedicated
  component that reads `Transport` inside its rsx attribute
  closures, or proper closure-prop support on `String` props
  through the component macro. For E6's first cut, the static
  glyph + tooltip ("Play / Pause") + toggle behavior is enough.
  Adding the reactive glyph is a small follow-up in E7's polish.
- **The cpal stream stays running across Pause / Stop.** Calling
  `cpal::Stream::pause()` during Pause would also work, but means
  the host can't push commands (the audio thread isn't draining
  the command queue while paused). Leaving the stream running and
  gating purely on transport state keeps the command-edit-while-
  paused flow open — important once the round-3 graph editor
  lands.

**Verification limitation.** The audio-thread / cpal callback path
isn't unit-testable on this dev machine (no working ALSA device for
the f32 driver). End-to-end "click Play → hear audio" requires a
real audio device; the existing engine-side tests cover transport
gating at the API layer, and the AudioResources tests cover the
state machine the buttons drive.

---

## Phase E7 — Polish + final sweep ✅ done

**Goal.** Close out the milestone.

**What landed.**

- File-size audit: `audio.rs` had climbed to 600 lines (86% of the
  700 cap). Extracted `PlayheadPoller` into its own
  `audio/poller.rs` module; `audio.rs` became `audio/mod.rs` at 547
  lines. Other near-cap files (`arrangement.rs` 577,
  `inspector/mod.rs` 572) checked but left in place — they're stable
  with no pending additions.
- Spacebar shortcut. `set_keyboard_interceptor` installed once in
  `app::main_window` after the audio store is created. Returns
  `true` only for Space without modifiers; everything else returns
  `false` so rinch's normal key handling continues. The handler
  reads the audio-thread transport atomic and calls
  `audio.play()` / `audio.pause()` to toggle.
- Reactive Play/Pause glyph. A new `transport_state: Signal<Transport>`
  field on `AudioResources` mirrors the audio-thread atomic; the
  public play/pause/stop methods now go through a `set_transport`
  helper that updates both views atomically. The top bar's Play
  button uses an rsx `match` on `transport_state.get()` (a real
  Signal read, so the reactivity tracker subscribes), with
  re-mounting `TransportBtn` variants per arm. Title strings updated
  to "Play (Space)" / "Pause (Space)" so the keyboard shortcut is
  discoverable via hover.
- Cpal device picker (delivered alongside E6 but technically a
  cross-cutting fix): `pick_output_device()` falls back to the
  first f32 device with a working config when cpal's `default`
  fails. Unblocks audio on stock Kubuntu without `pipewire-alsa`.

**Done when (met).**

- All three clippy gates clean (default / `--no-default-features` /
  `--features cpal-driver`).
- `cargo test --workspace` green (137 tests).
- Visual end-to-end verified via Rinch MCP:
  Click Play / Space → playhead advances, glyph swaps to Pause;
  Space / click Pause → playhead freezes, glyph swaps back;
  Stop → playhead returns to bar 1; Play after Stop → audio
  re-arms and plays from bar 1 again.

**Deviations from the original plan.**

- **Reactive glyph needed a UI-facing Signal, not the engine
  atomic.** The original wiring tried to read
  `TransportHandle::get()` from inside an rsx `match` scrutinee
  expecting reactivity. Caught during visual verification: the
  glyph didn't swap. Rinch's reactivity tracker only subscribes to
  `Signal` reads, not atomic loads — so `transport_state` was
  added as a UI mirror that play/pause/stop now keep in lockstep
  with the audio-thread atomic.
- **`audio.rs` split was minimal.** Only `PlayheadPoller` got
  extracted (~80 lines). The remaining file is cohesive
  (AudioResources struct + constructors + transport helpers +
  configure_graph), so further splitting would have meant churning
  module boundaries without a clear separation gain.
- **Spacebar interceptor is global, not focus-scoped.** Rinch's
  keyboard interceptor is a global singleton; only one can be
  active at a time. Round-1 has no text inputs yet, so this is
  fine — but when text-editing UI lands (round 3 pattern editor),
  the interceptor will need to defer to focused inputs (return
  false when a contenteditable has focus).

**Milestone graduated.** Round-3 priorities live in the
project_status memory: `rawdaw-sampler` (SF2/SFZ via oxisynth
+ drum sample player), real subtractive synth, `rawdaw-drumkits`
TOML loader, `rawdaw-fx` (EQ/reverb/delay/gain), tempo ramps,
project file load/save UI, pattern editor / piano roll.

---

## Out of scope (later milestones)

- Real instruments (`rawdaw-sampler`: SF2/SFZ via oxisynth + drum sample
  player). The sine-per-track is a plumbing placeholder.
- Tempo ramps. The engine's `ProcessContext` only honors a constant BPM
  (first `BpmEvent`). Tempo-map honoring is part of "engine iteration 4".
- Plugin hosting (CLAP-first). Explicitly deferred.
- Worker-pool parallel graph scheduling. Deferred until single-threaded
  hits a bottleneck.
- Project file I/O — load / save UI affordances. The model layer supports
  it; surfacing it in the UI is its own UX exercise.
- Editing UI (round 3 — pattern editor / piano roll).
