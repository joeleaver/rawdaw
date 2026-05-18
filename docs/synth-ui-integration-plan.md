## Synth UI integration plan (v1)

Land the **parameter event protocol**, **patch data model**, **synth-
editor panel scaffolding**, and **per-synth UI dispatch** that gate
all patch-editor work for the wavetable (v2) and drum (v0) synths.
After this milestone the user picks a synth track in the inspector,
edits its envelopes / filter / LFO / oscillator params / mod matrix
live, hears the changes immediately, and the edits persist as part of
the project — no more constant editing in source.

Called out in the "Future plan needed: synth UI integration" sections
of `docs/wavetable-synth-fm-plan.md` and
`docs/wavetable-synth-mod-matrix-plan.md`. Spans `rawdaw-engine`,
`rawdaw-dsp`, `rawdaw-model`, `rawdaw-synth-wavetable`,
`rawdaw-synth-drum`, and `rawdaw-app` — widest scope so far.

**Engineering constraints** (from `CLAUDE.md`): architectural
correctness over shortcuts; unlimited time and budget; ~700-line cap
per source file; no `unwrap()` outside tests; `forbid(unsafe_code)`
in every crate.

**Cadence.** Each phase ends with `cargo test --workspace` green,
clippy clean across all three feature builds, and a one-line
"done when" criterion observably met.

**Multi-session scope.** Largest milestone yet — likely 5–7 sessions.
U1–U3 are the "infra" half (engine + model + synth wiring, no UI
yet); U4–U7 are the "UI" half; U8 is persistence; U9 closes out.

---

## Status — not started

All phases pending. Pick U1 next.

- U0 ◀ this doc.
- U1–U9 pending.

---

## Design decisions locked in U0

### Parameter events extend the existing block-event channel

`BlockEvent { time, target, message: Midi2Message }` carries only
MIDI today. Parameter changes are a second kind of audio-thread-
addressed event that must (a) arrive sample-accurately, (b) order
against MIDI at the same target, (c) survive Stop → Play re-arming
the same way MIDI does.

Considered: (A) extend `BlockEvent::message` to
`BlockMessage::{Midi, Param}` — one queue, sample-accurate
interleaving free, matches CLAP/VST3; (B) sibling SPSC queue for
params — MIDI path unchanged but requires a merge sort to
interleave; (C) treat patches as immutable and swap nodes on edit —
zero new protocol but voices drop on every knob nudge.

**Pick A.** The plumbing cost is small (~5 call sites), the
protocol is the obvious right shape, and existing sample-accurate
delivery applies for free.

```text
enum BlockMessage {
    Midi(Midi2Message),
    Param(ParamEvent),
}
struct ParamEvent { path: [u8; 8], value: f32 }
```

### `ParamPath` is per-synth-typed, encoded as a small byte array

Each synth crate defines its own typed `ParamPath` enum
(`WavetableParam`, `DrumParam`, …). The engine sees these as opaque
`[u8; 8]` — generous for an enum tag plus a u8 slot index — and the
receiving node decodes.

```text
// In rawdaw-synth-wavetable:
enum WavetableParam {
    OscTune(u8), OscFineCents(u8), OscLevel(u8),             // i ∈ 0..=2
    EnvAttackS(u8), EnvDecayS(u8),                           // i ∈ 0..=2
    EnvSustain(u8), EnvReleaseS(u8),
    LfoRateHz, FilterCutoffHz, FilterResonance,
    MatrixSource(u8), MatrixDestination(u8), MatrixAmount(u8), // i ∈ 0..=15
}
```

Encode/decode helpers live alongside the enum:
`encode(&self) -> [u8; 8]`, `decode(&[u8; 8]) -> Option<Self>`. The
audio-thread `match` happens on the decoded enum. Bad bytes are
silently ignored in release (`debug_assert!` in debug) — a bad
parameter event is a host-side bug, not audio-time recoverable.

### Patch types: runtime in synth crates, serialized mirrors in `rawdaw-model`

Two-layer patch story:

1. **Runtime** patch types live in the synth crate that owns them
   (`rawdaw-synth-wavetable::WavetablePatch`,
   `rawdaw-synth-drum::DrumPatch`). They hold `rawdaw-dsp` types
   directly (`WavetableOscParams`, `AdsrParams`, `ModMatrix`, …),
   `Copy + Default`. The synth node mutates these in response to
   `ParamEvent`s; voices copy from the node at the next note_on.
2. **Serialized** mirrors live in `rawdaw-model::patch`
   (`WavetablePatchData`, `DrumPatchData`). Plain serde structs
   using only model-level primitives (f32, u8, fixed-size arrays),
   with an explicit `format_version: u32` for forward compatibility.
   `From` / `TryFrom` impls live in the synth crate.

Why two types and not one: making the rawdaw-dsp patch types serde
and using them directly couples the on-disk format permanently to
the dsp struct layout — a new mod source variant breaks every old
project file. Explicit versioning is the cost of doing it right;
the ~150 lines of mirror + From impls are cheaper than the future
migration cost.

Consequence: **`rawdaw-model` does not gain a dependency on
`rawdaw-dsp`.** The model stays leaf-domain; the conversion happens
in the synth crate where dsp is already a dependency.

### `Track` gains a typed `SynthAssignment` with embedded patch data

```text
// In rawdaw-model:
enum SynthAssignment {
    Wavetable(WavetablePatchData),
    Drum(DrumPatchData),
    // Physical(PhysicalPatchData), // future
}

struct Track {
    id: TrackId,
    name: String,
    kind: TrackKind,            // unchanged
    instrument: InstrumentId,   // unchanged
    mixer: MixerPlacement,
    synth: SynthAssignment,     // NEW
}
```

Default `SynthAssignment` for `TrackKind::Pitched` is
`Wavetable(WavetablePatchData::default())` — a serialized form of
the current `PATCH_OSC_PARAMS` + `PATCH_MATRIX_SLOTS`. Drum tracks
default to `Drum(DrumPatchData::default())`. The round-1 fixture
picks these up automatically; audio unchanged after U2.

Constraint: `SynthAssignment` must match `TrackKind`. Enforced by
`Track::new_pitched` / `Track::new_drum` constructors + a runtime
assertion in `configure_graph`. A generic Track type would balloon
`Project::tracks` into a heterogeneous collection — constructor +
assertion is the right cost.

### Per-synth UI dispatch lives in the inspector (widened)

The 320 px inspector widens to ~600 px when a synth track is
selected and renders a `SynthEditor` that dispatches on the
`SynthAssignment` variant:

```text
match track.synth { /* in rawdaw-app */
    SynthAssignment::Wavetable(_) => rsx! { WavetableEditor { track_idx } },
    SynthAssignment::Drum(_)      => rsx! { DrumEditor      { track_idx } },
}
```

Considered alternatives: a fourth column (library / arrangement /
inspector / synth-editor) crowds the arrangement on small monitors;
a modal breaks the "edit while the song plays" workflow.
Widening reuses the inspector's existing per-selection dispatch and
keeps width changes free via flexbox. The width is reactive:
`Inspector` reads `app.selected_track` and toggles the style.

### Mod matrix editor: Vital-style slot list

Vertical scrollable list of 16 slots, each row showing `[Source
dropdown] [→] [Destination dropdown] [Amount slider] [×]`.
Confirmed reference UX (Joe knows it from Vital).

- The slot list is a `div { overflow: auto }` with 16 fixed-height
  rows. No virtualization — 16 × 4 controls fits easily under
  Rinch's per-frame budget.
- Each row is a `MatrixSlotEditor { slot_idx, track_idx }` that
  renders the four controls and translates edits into
  `WavetableParam::Matrix{Source,Destination,Amount}` events.
- Dropdowns use a `Select` component (port from Rinch's component
  crate if it's not there).
- "×" removes the slot by setting source back to `None`.

### Slider primitive (knobs are v2)

The editor needs ~30 numeric controls (env ADSR ×3, filter ×2, LFO,
per-osc tune/fine/level ×9, matrix amount ×16). v1 ships a
**slider-with-text-input** — horizontal track + draggable thumb +
click-to-type numeric input. Round knobs are a polish item. Matches
GarageBand's plugin parameter UI (Joe reference).

Lives at `crates/rawdaw-app/src/parts/slider.rs` (~150 lines). Props:
`label: String`, `value: f32`, `min: f32`, `max: f32`, `unit:
String`, `on_change: Callback<f32>`.

### Synth-mounted UI subscribes via a polled version counter

Audio thread → UI: `WavetableSynthNode` holds an `Arc<AtomicU64>`
patch-version counter that increments on every `ParamEvent` apply.
The UI's `WavetableEditor` mounts a small poller (same shape as
`PlayheadPoller`) at ~20 Hz that watches the counter and re-pulls
the patch when it advances. Keeps the UI live even when patches
change from outside the editor (future MIDI Learn, automation).

Per-parameter `Signal<f32>` was the alternative — rejected because
the editor has ~80 parameters with matrix slots, and Rinch's
per-signal cost at that count exceeds the polling overhead.

UI → audio: the slider's `on_change` builds a `ParamEvent` and pushes
it at `current_sample_clock + 1`. Fast mouse drags producing hundreds
of events/sec all flow through the queue — at sane queue capacities
this is fine; a "latest-value-wins" coalescer is a small optimization
deferred until measurement shows it's needed.

### Patch persistence: factory presets in U8; full save/load deferred

U2 lands `WavetablePatchData::default()` mirroring the current M5
patch. U8 ships a **factory preset bank** — JSON patch files in
`assets/presets/wavetable/*.json` loaded into a preset dropdown
above the editor; picking a preset overwrites the current track's
patch. No save-as-preset in U8 (small follow-up).

Full project save/load (write a `Project` JSON, open it back) is
deferred to its own plan — separate concerns: file dialog, format
migration, undo/redo restoration, unsaved-changes indicator.
Bundling here would push scope past 9 phases.

---

## Phase U0 — Plan + design decisions ◀ this doc

**Goal.** Lock the cross-cutting design before any code lands.

**Done when.** This doc is committed; the design-decisions section
above describes every decision the U1–U9 phases will rely on.

---

## Phase U1 — Parameter event protocol in `rawdaw-engine`

**Goal.** Land the engine-side scaffolding for sample-accurate
parameter events alongside MIDI. No synth-side consumption yet — the
synth nodes still ignore `Param` variants and only act on
`Midi`. This phase is pure infrastructure.

**Steps.**

- Introduce `BlockMessage::{Midi(Midi2Message), Param(ParamEvent)}`
  in `rawdaw-engine::event`. `ParamEvent { path: [u8; 8], value: f32 }`.
- Replace `BlockEvent { time, target, message: Midi2Message }` with
  `BlockEvent { time, target, message: BlockMessage }`. Replace
  `BlockEventInBlock { offset_in_block, message: Midi2Message }`
  with the new `message: BlockMessage`.
- All MIDI-only call sites (`translate_events`, `AudioResources::
  rearm_events`, the synth crates' `apply_event`, tests) thread
  the enum tag through. `apply_event` in each synth currently
  takes `&Midi2Message`; bump to `&BlockMessage` and add a
  `BlockMessage::Param` arm that's a no-op (with `debug_assert!` so
  unexpected param events surface in tests).
- New helper on `EngineHandle`: `push_param(time, target, path,
  value) -> Result<(), PushError<BlockEvent>>` that wraps the
  encoding. Symmetric helper `push_midi` for clarity.
- Round-1 audio path unchanged: no `Param` events flow yet.

**Done when.** Workspace tests still pass byte-for-byte for audio
output (`AudioResources::build` produces the same `BlockEvent`
stream as before; only the message variant tag is new). New tests
pin: (a) `BlockMessage::Param` round-trips through the event queue
with the same `(time, target, path, value)`; (b) a synth that
receives a `Param` event whose path it doesn't recognize doesn't
crash the audio thread (silent ignore in release, debug_assert
in debug); (c) `push_param` and `push_midi` interleave correctly
when `time` is equal (existing FIFO ordering on the rtrb queue
preserves push order). Workspace tests + all three clippy gates
clean.

---

## Phase U2 — Patch data types in `rawdaw-model` + `rawdaw-dsp` types unchanged

**Goal.** Land the serialized patch shape and wire `Track::synth`
into the project model. Audio behaviour unchanged: every track in
round-1 gets a default patch that matches today's hardcoded
constants byte-for-byte.

**Steps.**

- New module `rawdaw-model::patch`. Submodules `wavetable` and
  `drum`. Public types:
  - `WavetableOscParamsData { tune_semitones: i8, fine_cents: i8,
    level: f32 }`.
  - `AdsrParamsData { attack_s: f32, decay_s: f32, sustain_level:
    f32, release_s: f32 }`.
  - `ModSourceData { ... }` mirror of `rawdaw-dsp::ModSource`.
  - `ModDestinationData { ... }` mirror.
  - `ModSlotData { source: ModSourceData, destination:
    ModDestinationData, amount: f32 }`.
  - `WavetablePatchData { format_version: u32, osc_params: [...; 3],
    env_params: [...; 3], lfo_rate_hz: f32, filter_cutoff_hz: f32,
    filter_resonance: f32, matrix: [ModSlotData; 16] }`.
  - `DrumPatchData { format_version: u32, kick: KickPatchData,
    snare: SnarePatchData, closed_hat: HatPatchData, open_hat:
    HatPatchData }` with appropriate inner shapes (envelope params,
    base pitch, noise level — mirror today's per-voice constants).
  - `SynthAssignment::{Wavetable(WavetablePatchData),
    Drum(DrumPatchData)}` enum.
- `Track` gets a `synth: SynthAssignment` field. `Default` impl
  picks the right variant based on `TrackKind`. The round-1
  fixture's `Track` constructions populate `synth` with the
  current hardcoded patch values lifted from
  `crates/rawdaw-synth-wavetable/src/lib.rs` and
  `crates/rawdaw-synth-drum/src/voices/*` (Status: today these
  live as constants in the synth crates — the lift makes them
  data).
- A `Track::new_pitched(...)` / `Track::new_drum(...)` constructor
  guarantees the `(kind, synth)` invariant; existing direct-field
  construction in tests is migrated.
- `Project` serializes with the new `synth` field; deserialization
  of pre-U2 project JSONs is **not supported** (this is the first
  version of the format; there's no real project-file path yet).
- All `serde` derives gated on the existing `serde` feature (no
  new optional dep).

**Done when.** Round-1 fixture produces the same audio
byte-for-byte (the lifted constants match the hardcoded values).
Workspace tests pass. New tests pin: (a) `WavetablePatchData::
default()` matches what `WavetableSynthNode::new`-equivalent reads
post-U3; (b) `SynthAssignment::Wavetable(_)` paired with
`TrackKind::Drum` panics in `Track::new_*` (debug) or fails the
`debug_assert` in realize; (c) `serde_json::to_string` /
`from_str` round-trips a round-1 track + its patch.

---

## Phase U3 — Synth nodes consume runtime patches + parameter events

**Goal.** Replace `PATCH_OSC_PARAMS` and friends in the synth crates
with runtime `WavetablePatch` / `DrumPatch` types constructed from
the model's `WavetablePatchData` / `DrumPatchData` at node
construction. Parameter events arriving on the audio thread update
the runtime patch + propagate to voices.

**Steps.**

- `rawdaw-synth-wavetable::WavetablePatch` lands as the runtime
  patch type:
  ```text
  pub struct WavetablePatch {
      pub osc_params: [WavetableOscParams; 3],
      pub env_params: [AdsrParams; 3],
      pub lfo_rate_hz: f32,
      pub filter_cutoff_hz: f32,
      pub filter_resonance: f32,
      pub matrix: [ModSlot; 16],
  }
  ```
  `From<WavetablePatchData>` impl converts model → runtime.
  Constructors: `WavetablePatch::default()` (matches today's
  hardcoded patch), `WavetablePatch::from_data(&WavetablePatchData)`.
- `WavetableSynthNode` takes the patch at construction:
  `WavetableSynthNode::with_patch(patch: WavetablePatch)`.
  `WavetableSynthNode::new()` delegates to `with_patch(default())`.
  The `PATCH_*` constants in the synth crate's `lib.rs` are
  **removed** — patches are data now.
- `WavetableSynthNode` implements `apply_event` for
  `BlockMessage::Param`: decodes `WavetableParam`, mutates the
  relevant field on the node's patch, and pushes the change into
  all voices via `propagate_patch_to_voices()`. The propagation
  cost is small (16 voices × patch size); the audio thread does it
  per-event, not per-sample.
- `WavetableParam` enum + `encode`/`decode` helpers land in
  `rawdaw-synth-wavetable::param`. ~13 variants (see U0 design
  decisions for the inventory).
- Mirror: `DrumPatch` in `rawdaw-synth-drum` + `DrumParam` + the
  per-voice param routing.
- `AudioResources::configure_graph` reads each track's
  `SynthAssignment` and constructs the synth node with the lifted
  patch:
  ```text
  TrackKind::Pitched { .. } => {
      let SynthAssignment::Wavetable(data) = &track.synth else {
          unreachable!("Track::new_* enforces the invariant");
      };
      WavetableSynthNode::with_patch(WavetablePatch::from_data(data))
  }
  ```
  Same shape for Drum.

**Done when.** Round-1 audio byte-identical to pre-U3 (the lifted
patches resolve to the same constants). Workspace tests pass; the
existing `WavetableSynthNode` tests still hold. New tests pin: (a)
`WavetableSynthNode::with_patch(custom)` actually uses the custom
patch (rendered audio differs from default); (b) a synthetic
`WavetableParam::FilterCutoffHz @ 2000.0` event arriving at the
node updates the next voice's filter (rendered audio diverges from
no-event baseline); (c) `WavetableParam` round-trips through
`encode/decode` byte-for-byte for every variant.

---

## Phase U4 — Per-synth UI dispatch + selection model

**Goal.** Add a `selected_track: Signal<Option<usize>>` to the
`AppState` and a `SynthEditor` component in the inspector that
dispatches to a per-synth panel placeholder. No actual editor
controls yet — the dispatcher is the surface.

**Steps.**

- `AppState` gains `pub selected_track: Signal<Option<usize>>`
  alongside the existing `selected_idx` (which is section-block
  selection — a different axis).
- The arrangement's track-row header gets a click handler that
  writes `app.selected_track.set(Some(track_idx))`. Tracks are
  clickable via their existing left-pane label.
- `Inspector` gains a branch: if `app.selected_track.get()` is
  `Some(idx)` AND the selected section/block (the existing
  `selected_idx`) is `None`, render `SynthEditor { track_idx }`.
  Section-block selection takes precedence so the existing
  inspector behavior is preserved.
- `SynthEditor` component reads
  `project.tracks[track_idx].synth` and dispatches:
  ```text
  match synth_kind {
      SynthAssignment::Wavetable(_) =>
          rsx! { WavetableEditor { track_idx: track_idx } },
      SynthAssignment::Drum(_) =>
          rsx! { DrumEditor { track_idx: track_idx } },
  }
  ```
- `WavetableEditor` and `DrumEditor` are placeholder components
  this phase — they render `"Wavetable patch editor (U5)"` and
  `"Drum patch editor (U7)"` text only.
- The inspector pane's `width` style is reactive: 600 px when a
  synth track is selected, 320 px otherwise. Style closure reads
  the same selection signal.

**Done when.** Selecting the round-1 lead track shows
`"Wavetable patch editor (U5)"` in a 600-px-wide inspector pane;
selecting the kick track shows `"Drum patch editor (U7)"`;
selecting nothing shows the empty inspector at 320 px. Workspace
tests + all three clippy gates clean. New tests pin: (a) the
inspector width signal changes on selection; (b) the dispatch
component renders the right placeholder for the round-1 track
kinds.

---

## Phase U5 — `WavetableEditor` scalar controls (no matrix yet)

**Goal.** Real numeric controls for the wavetable patch's
non-matrix fields. Slider primitive lands in this phase; matrix
editor is deferred to U6.

**Steps.**

- `crates/rawdaw-app/src/parts/slider.rs` lands the v1 slider
  component:
  ```text
  #[component]
  pub fn Slider(
      label: String,
      value: f32,
      min: f32,
      max: f32,
      unit: String,         // "Hz" / "s" / "cents" / ""
      on_change: Callback<f32>,
  ) -> NodeHandle { ... }
  ```
  Horizontal track + draggable thumb. Click-to-type-numeric
  value via a hidden input that focuses on label double-click.
  Drag-to-scrub for fine control.
- `WavetableEditor` component renders sections:
  - **Oscillators** — three rows, each with sliders for tune
    (-24..=24), fine (-100..=100), level (0..=1).
  - **Envelopes** — three columns (ENV1/2/3), each with
    attack/decay/sustain/release sliders.
  - **Filter** — cutoff (20..=20000 Hz log scale), resonance
    (0..=4).
  - **LFO** — rate (0..=20 Hz).
- Each slider's `on_change` builds a `WavetableParam::...`
  variant + pushes a `BlockEvent` with `BlockMessage::Param` at
  `time = current_sample_clock + 1` and target =
  the track's `NodeId`.
- The editor pulls patch state via an `Rc<WavetablePatchView>` —
  a UI-thread copy of the patch that's refreshed by a polling
  thread (described in U0 design decisions). Polling at ~20 Hz
  via the same pattern as `PlayheadPoller`.
- All control labels use Joe's standard reference-DAW vocabulary
  (Attack / Decay / Sustain / Release, Cutoff, Resonance, …) —
  no rawdaw-internal jargon in the UI.

**Done when.** Joe selects the lead track, drags the filter cutoff
slider, and hears the cutoff change in real time on the round-1
fixture (the round-1 loop plays continuously while editing). All
sliders are wired and round-trip through the event queue. Workspace
tests pass; new tests pin: (a) a slider's `on_change` produces a
`BlockMessage::Param` event at the right target with the right
encoded path; (b) the editor's polling thread cleans up on
unmount (mirror of `PlayheadPoller`'s `Drop` semantics). Manual
verification via the rinch MCP — screenshot of the editor, drag a
slider, screenshot again, confirm the visual + audio change.

---

## Phase U6 — Mod matrix editor

**Goal.** Vital-style slot list for the 16 mod matrix slots. Each
row: `[Source dropdown] [→] [Destination dropdown] [Amount slider]
[×]`. Adding/removing slots is just setting source ∈ {None, ...}.

**Steps.**

- `crates/rawdaw-app/src/parts/select.rs` lands a `Select`
  component if Rinch's component crate doesn't already have a
  usable one (check `../rinch/crates/rinch-components/` first;
  reuse if available, port otherwise).
- `MatrixSlotEditor` component:
  ```text
  #[component]
  fn MatrixSlotEditor(slot_idx: u8, track_idx: usize) -> NodeHandle {
      // Read the slot from the patch view.
      // Source dropdown: ModSourceData variants.
      // Destination dropdown: ModDestinationData variants.
      // Amount slider: -1.0..=1.0.
      // Each control's on_change pushes a corresponding
      // WavetableParam::Matrix{Source,Destination,Amount} event.
  }
  ```
- `MatrixEditor` component renders 16 `MatrixSlotEditor` rows
  inside a scroll container. The slot list shows all 16 rows
  always (no virtualization needed at this count). Empty rows
  (source = None) render dimmed so the user can tell which slots
  are active.
- The matrix is a sub-section of `WavetableEditor` — opens
  collapsed by default to keep the editor pane scannable; click
  to expand.

**Done when.** Joe can wire LFO → FilterCutoff from a fresh
"empty matrix" state, set the amount, hear the result, and see
the cutoff slider wobble as the LFO drives it. Adding a cyclic
audio-rate routing (e.g. `Osc0 → PmAmountOf(0)`) gets
silently sanitized (release behavior; the slot reverts to
source = None and the dropdown reflects it). Workspace tests pass.
New tests pin: (a) `MatrixSlotEditor`'s controls push the right
`WavetableParam` variants; (b) the matrix UI reflects an
externally-pushed matrix change (sets some slots via direct
`apply_event`, polls, sees the dropdowns update).

---

## Phase U7 — `DrumEditor` per-voice controls

**Goal.** Mirror of U5 + U6 for the drum synth. Per-voice ADSR for
each of the four drum types + the per-voice noise/pitch
parameters that today live as constants in
`crates/rawdaw-synth-drum/src/voices/`.

**Steps.**

- Lift the drum synth's per-voice constants into runtime patch
  fields in U3 (KickPatchData → KickPatch with envelope, base
  pitch, pitch-envelope amount; SnarePatchData → SnarePatch with
  noise level, body envelope, snap envelope; HatPatchData → HatPatch
  with closed/open decay, noise filter cutoff). This work
  *technically* lands in U3 but if it's a meaningful chunk it
  spreads into U7 — flag this in U3's "done when" if so.
- `DrumEditor` component renders four columns (Kick / Snare /
  ClosedHat / OpenHat), each with the relevant sliders. Reuses
  the `Slider` component from U5.
- Each slider pushes a `DrumParam::...` variant.

**Done when.** Joe can change the kick's pitch envelope amount
from the UI and hear the kick get higher-pitched. All drum voice
parameters are exposed; workspace tests pass.

---

## Phase U8 — Factory preset bank + per-track preset switcher

**Goal.** A directory of JSON patch files loaded into the synth
editor's preset dropdown. Picking a preset overwrites the current
track's patch.

**Steps.**

- New `assets/presets/wavetable/` directory committed to the repo
  with starter JSON patch files. Initial set:
  - `default.json` — today's M5 default.
  - `pluck.json` — strong ENV2-on-filter envelope.
  - `bass.json` — low octave, slow ENV1, no PM.
  - `bell.json` — strong PM, fast decay.
  - `pad.json` — slow envelopes, deep LFO.
- New `assets/presets/drum/` directory with initial kits:
  - `default.json` — today's defaults.
  - `acoustic.json` — pitched-down kick, softer snare.
  - `electronic.json` — sharper, harder.
- `crates/rawdaw-app/src/presets.rs` loads the JSON files at
  startup (via `include_str!` for v1 — embedded so the binary is
  self-contained; v2 can switch to a filesystem-relative loader
  for user presets).
- `PresetDropdown` component above each synth editor lets the
  user pick a preset. Picking overwrites the current track's
  patch via a single batched `BlockMessage::Param` per field
  (cheap — ~80 events × one block ≈ negligible).
- No "Save as preset" in U8. That's a small follow-up (file
  dialog plumbing + user preset directory).

**Done when.** Joe picks "bell" from the wavetable preset dropdown
and the lead track sounds like a bell on the round-1 fixture.
Switching back to "default" returns to the M5 sound.
Workspace tests pass; new tests pin: (a) every shipped preset
JSON parses successfully; (b) picking a preset issues the right
batch of `BlockMessage::Param` events.

---

## Phase U9 — Plan + memory updates

**Goal.** Close out the synth UI integration milestone.

**Steps.**

- This doc's per-phase "Done when" markers updated with ✅ status
  and any deviations recorded.
- `project_status` memory updated: synth UI integration U0–U8
  done. Picklist for next session edited so the listed growth
  items (full project save/load, "save as preset" workflow, MIDI
  Learn, automation lanes, more presets, more synths' editors)
  reflect what U-milestone leaves on the table.
- Cross-reference notes added to `wavetable-synth-fm-plan.md` and
  `wavetable-synth-mod-matrix-plan.md`'s "Future plan needed"
  sections, marking them resolved.

---

## Future plans needed

These cross-cutting items are out of scope for U1–U9 but warrant
their own plan docs once this milestone is done:

- **Project file save/load** (`docs/project-file-plan.md`). File
  dialog, format versioning + migration, "unsaved changes"
  indicator, autosave, recovery. Cross-cuts UI + model + serde.
- **Synth parameter automation** (`docs/automation-plan.md`).
  Automation lanes in the arrangement that emit `Param` events
  on a schedule. The U1 event protocol is designed to accommodate
  this — no extra plumbing needed at the engine level.
- **MIDI Learn** for hardware controllers. Maps physical MIDI CC
  to `WavetableParam` / `DrumParam` paths. Light scope but a
  separate concern from the in-UI editor.
- **Wavetable bank + per-osc wavetable selection** — already
  flagged in `wavetable-synth-mod-matrix-plan.md` as v3+ growth.
  Adds another control to the per-osc UI section.
- **Soft-clipper on the master + parameter-driven fx** — overlaps
  with this plan's event protocol (rawdaw-fx nodes will consume
  the same `Param` events for cutoff/gain/etc. automation).

---

## Out of scope (deferred)

- Full project save/load (lives in its own plan).
- "Save as preset" workflow — small follow-up after U8.
- Round knobs (sliders ship first; knobs are a UI polish item).
- Wavetable visualization (spectrum scope, envelope curve display,
  matrix-routing graph view). Visual polish; not load-bearing for
  the editor's function.
- MIDI Learn / hardware-controller mapping.
- Drag-to-route in the matrix editor (drag a source onto a
  destination on the synth diagram). Vital has this; a separate
  exploration once the slot-list editor is in place.
- A/B patch comparison (Vital's "compare to previous" button).
- Patch undo/redo independent of the project-level undo stack.
- Multi-track patch operations (copy patch from track A to track
  B). Trivial to add after U8 but not part of v1 minimum.

These are deliberate U-milestone omissions, called out so the v1
done-when isn't muddled with them.
