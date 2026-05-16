# Architecture

## Layering

```
┌────────────────────────────────────────────────────┐
│  UI layer  (Rinch)                                 │
│  Song-form view, piano-roll detail, mixer, library │
└────────────────────┬───────────────────────────────┘
                     │ commands / observations
┌────────────────────┴───────────────────────────────┐
│  Project / composition model (non-RT)              │
│  Sections, ChordLoops, Patterns, Tracks, Library   │
│  Realization: structural → concrete MIDI events    │
└────────────────────┬───────────────────────────────┘
                     │ scheduled MIDI events + audio graph updates
┌────────────────────┴───────────────────────────────┐
│  Audio engine (RT)                                 │
│  DAG of AudioNodes; topo-sorted; serial processing │
│  Tempo map; sample-accurate event delivery         │
└────────────────────┬───────────────────────────────┘
                     │ audio buffers
┌────────────────────┴───────────────────────────────┐
│  Audio I/O  (cpal)                                 │
└────────────────────────────────────────────────────┘
```

The composition model and the audio engine talk via RT-safe channels (lock-free ring buffers: `rtrb` or `ringbuf`). The UI never talks to the audio engine directly — it talks to the project model, which schedules events and graph updates into the engine.

## Audio engine

### Graph

A DAG of `AudioNode`s, topologically sorted once per routing change. Each node implements:

```rust
trait AudioNode {
    fn process(
        &mut self,
        inputs: &[&[f32]],
        outputs: &mut [&mut [f32]],  // multi-output supported: one buffer per declared output
        events: &EventBlock,         // MIDI events scheduled for this block
        nframes: usize,
        ctx: &ProcessContext,        // sample rate, tempo, transport state
    );
    fn latency(&self) -> usize;
    fn output_descriptors(&self) -> &[OutputDescriptor];  // names + channel layouts
    // ...
}
```

- **Processing model:** serial single-threaded on the RT thread for v1. Topological sort + process in order. Parallel scheduling (work-stealing across the DAG) is a future concern; serial will handle a lot of tracks before it's a bottleneck.
- **Plugin delay compensation:** each node reports latency; the engine inserts implicit delays so all paths arrive sample-aligned at outputs.
- **Routing topology:** user-facing model is tracks-with-inserts and sends to bus/return tracks; internally that's a DAG. Sidechains are just additional input edges.

### RT-safety rules

- No allocation, no locks, no syscalls on the audio thread. Enforced in debug with `assert_no_alloc`.
- Communication with the non-RT side via lock-free ring buffers (`rtrb`).
- Shared state that needs deferred drop uses `basedrop`.
- Parameter changes flow audio-thread-ward through the same event channel as MIDI events; node implementations apply them sample-accurately within a block.

### Audio I/O

`cpal` handles the audio backend. Linux primary target (ALSA, JACK, PipeWire via JACK shim). Cross-platform falls out for free — CoreAudio, WASAPI work via the same `cpal` API. JACK is the default Linux backend for low-latency.

## MIDI / event model

Internal event types target MIDI 2.0:

- 32-bit note attributes (pitch + per-note attribute), high-resolution velocity, per-note controllers.
- Channel voice messages with the MIDI 2.0 wider data ranges.
- Sample-accurate timing: events are attached to specific sample offsets within a processing block.

External I/O (via `midir`) is MIDI 1.0 for v1; conversion happens at the boundary. Most modern controllers and synths are still 1.0-only.

### Tempo map

A first-class structure mapping musical time (bars/beats/ticks) ↔ sample time. Owns:
- BPM events (constant, ramped, or curve).
- Time signature events.
- Conversion functions used by the engine, the realization pass, and the UI.

The tempo map is part of the project model; the engine receives an immutable snapshot per processing block.

## Composition model layer (non-RT)

Lives in regular Rust code with no RT constraints. Owns:
- Project state (key, tempo map, library, tracks, arrangement). See `composition-model.md`.
- The **realization pass**: turning `(Pattern, ChordLoop, scale, role, realization_params)` into concrete MIDI events. Pure-functional, cached, incremental. Designed in `realization.md`.
- Scheduling: walking the arrangement to produce a stream of MIDI events ahead of the playhead, pushed into the engine via the event channel.

The realization pass runs on a worker thread ahead of the playhead (lookahead window: 100–500ms) so the audio thread always has events queued. It re-runs incrementally when structural objects change, and a cache keyed by activation entry / chord loop / scale / tempo window keeps the hot path cheap.

## Plugin host boundary (future)

When plugin hosting is added, it lives in a **separate crate** (e.g. `rawdaw-plugin-host`). The main app consumes it through a trait:

```rust
trait Plugin {
    fn process(&mut self, …);  // same signature as built-in AudioNode
    fn parameters(&self) -> &[ParamDescriptor];
    fn save_state(&self) -> Vec<u8>;
    fn load_state(&mut self, data: &[u8]) -> Result<(), …>;
    // ...
}
```

External plugins are wrapped in `AudioNode` adapters so the engine doesn't care whether a node is built-in or hosted. CLAP-first; VST3 only if there's strong demand later.

All `unsafe` FFI lives in the plugin-host crate; the rest of rawdaw remains safe Rust.

## Built-in instruments and effects

v1 ships with:
- **Sampler** — at least SF2/SFZ support via `oxisynth` or similar, plus our own simple sample player for one-shot drums.
- **Synth** — at least one rawdaw-built subtractive synth.
- **Drum kits** — TOML-defined kits backed by the sampler (or future synth kits). See `drum-patterns.md`.
- **Effects** — gain/pan/mute (per-track utility), parametric EQ, basic reverb, basic delay. Enough to mix the built-in instruments competently.

Each built-in instrument/effect is its own library crate exposing a clean DSP/parameter API, wrapped in an `AudioNode` impl. This shape means wrapping them as CLAP plugins later (so they're usable in other DAWs) is straightforward.

### Multi-output instruments (v1)

The engine supports multi-output instruments from the start. An `AudioNode` declares N named output buses; its `process` writes into each. The mixer auto-creates a channel per output when a multi-out instrument is added to a track. This is required for drum kits to expose per-voice mixing (kick, snare, hats… each addressable), which falls out as the natural "drum bus" workflow once those channels are routed to a shared return track.

This is the same DAG mechanism — multi-out just means a node has more than one outgoing edge by index. No new RT-side concepts. The mixer model (channel creation, routing UI) is where the work is.

## UI

Rinch is the leading candidate. The UI is reactive: it subscribes to project-model observations and dispatches commands. UI thread is *not* the RT thread; it can allocate freely.

Major views:
- **Song-form / arrangement view** — centerpiece. Sections as blocks, chord-loop overlays, track activations.
- **Piano-roll detail view** — opens on a clip or pattern. Shows derived notes with structural context.
- **Mixer view** — channel strips for tracks and return tracks. Inserts, sends, fader/pan.
- **Library panel** — chord loops, patterns, sections; drag-and-drop reuse.

## Crate layout (tentative)

```
rawdaw/                  # workspace root
├── crates/
│   ├── rawdaw-app/      # the binary; UI + glue
│   ├── rawdaw-engine/   # audio graph, RT thread, tempo map
│   ├── rawdaw-model/    # composition data model, realization
│   ├── rawdaw-midi/     # MIDI 2.0 event types, midir wrapper
│   ├── rawdaw-synth-*/  # built-in synth(s) as separate crates
│   ├── rawdaw-sampler/  # built-in sampler (drives drum kits too)
│   ├── rawdaw-drumkits/ # drum kit TOML loader + voice/output resolution
│   ├── rawdaw-fx/       # built-in effects
│   └── rawdaw-plugin-host/  # (future)
```

Splitting now (rather than starting as one crate) is cheap and gives clean boundaries. The synth/sampler/fx crates becoming standalone CLAP plugins later is a wrap-and-ship operation, not a refactor.
