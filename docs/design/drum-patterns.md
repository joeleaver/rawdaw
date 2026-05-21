# Drum Patterns and Drum Tracks

Drums need a distinct model from pitched content because they're pitch-symbolic (not chord-relative), naturally multi-voice, and typically processed per-voice in the mixer. This doc covers drum-specific decisions; the general pattern/clip model is in `composition-model.md`.

## Pattern kind

`Pattern` is a sum type:

```rust
enum PatternKind {
    Pitched(PitchedPatternBody),
    Drum(DrumPatternBody),
}
```

A pitched pattern's events are `(time, duration, chord-or-scale-degree, velocity)`; realization depends on chord/scale context.

A drum pattern's events are `(time, duration, DrumVoice, velocity, articulation?, micro_timing?)`. No chord context applies; voices are symbolic and resolved through the track's drum kit.

## DrumVoice — symbolic, not MIDI

A drum voice is an abstract name like `kick`, `snare`, `hat.closed`, `hat.open`, `tom.lo`, `crash`, `ride.bell`. **Patterns store voices, not MIDI notes.** The drum kit on the track maps voices to MIDI notes (and to outputs — see below).

Vocabulary is **hybrid**:
- A fixed enum of common pieces (kick, snare, hat.closed, hat.open, tom.lo/mid/hi, crash, ride, ride.bell, clap, rim) gets first-class UI affordances (icons, default step-sequencer lane order).
- An open `extras.*` namespace handles less common voices (`extras.808.sub`, `extras.shaker`, `extras.cowbell`). Kits declare which `extras` they support.

Pattern events that reference a voice the current kit doesn't support: realize as silent, with a UI warning. Lets you swap kits without losing pattern data — voices come back when you swap back to a compatible kit.

## Articulations

Articulations (snare ghost / rim / flam; hat closed / half-open / open / foot; ride bell / edge / bow) are **per-event tags**, not separate voices.

```rust
struct DrumEvent {
    voice: DrumVoice,
    articulation: Option<ArticulationTag>,
    velocity: U7,            // MIDI 2.0 widens; using MIDI 1 here for clarity
    micro_timing: i16,       // sub-tick offset
    // ...
}
```

The kit decides how to realize an articulation: different sample, different MIDI note, or — once on MIDI 2.0 — a per-note attribute. This keeps the pattern's vocabulary small and stable while letting kits express the nuance.

## Pattern variants

Drum patterns (and pitched patterns, but it especially matters here) come in families: main groove, fill, build, breakdown. Variants are first-class.

A `Pattern` owns:

```rust
struct Pattern {
    id: PatternId,
    name: String,
    kind: PatternKind,
    shared: PatternMetadata,  // length, voice list (drum), key/scale hints (pitched)
    variants: BTreeMap<VariantId, PatternBody>,
    default_variant: VariantId,
}
```

- Each variant is a **complete pattern body**. Variants do not inherit; a fill can be radically different from the main groove.
- The parent pattern owns shared metadata so all variants share length and voice vocabulary. Editing shared metadata (e.g. shortening from 4 bars to 2) propagates to all variants — out-of-range events are dropped with a warning.
- Variants are a flat list, not a tree. Convention is to have a `main` variant and additional named ones (`fill`, `build`, `breakdown`, `walk-down`, `chorus-end`, etc.).

## Variant scheduling via sub-section time ranges

Activation entries (see `composition-model.md`) gain a variant schedule:

```rust
struct ActivationEntry {
    pattern_ref: PatternRef,
    variant_schedule: Vec<(BarRange, VariantId)>,
    realization: RealizationParams,
    per_note_overrides: Vec<NoteOverride>,
}
```

- Empty schedule → the pattern's `default_variant` plays for the whole activation.
- Schedule entries pin a variant to a sub-section bar range. Uncovered ranges use the default variant.
- "Fill in the last bar of the chorus" is one schedule entry: `[(bar 3, "fill")]`.

This mechanism is **not drum-specific** — it works equally for a `verse-bass` pattern with a `walk-down` variant assigned to the bar before the chorus. Drums are just the common case.

UX: clicking a sub-range of an activation block in the song view opens a variant picker; the activation block shows colored sub-segments visualizing the schedule. The pattern editor has a variant-tabs strip at the top to switch which variant you're editing.

## Drum tracks

Tracks are typed:

```rust
enum TrackKind {
    Pitched { instrument: InstrumentRef, ... },
    Drum    { kit: DrumKitRef, instrument: InstrumentRef, ... },
}
```

A drum track accepts only drum patterns; a pitched track only pitched patterns. The drum track carries a kit reference, which in turn references the underlying instrument.

## Drum kits

Drum kits are TOML files. A kit declares its supported voices, its outputs, and the voice-to-(output, MIDI note, articulation map) mapping.

```toml
# example: vintage-808.kit.toml
name = "Vintage 808"
instrument = "rawdaw-sampler"   # which built-in instrument this kit drives
sample_pack = "samples/808/"

[outputs]
# Output name → channel layout (mono or stereo)
kick  = "stereo"
snare = "stereo"
hats  = "stereo"
perc  = "stereo"
mix   = "stereo"   # optional kit-internal mix (overhead/room blend)

[voices.kick]
output = "kick"
midi_note = 36
sample = "808-kick.wav"

[voices.snare]
output = "snare"
midi_note = 38
sample = "808-snare.wav"
[voices.snare.articulations]
ghost = { sample = "808-snare-ghost.wav" }
rim   = { sample = "808-rim.wav" }

[voices."hat.closed"]
output = "hats"
midi_note = 42
sample = "808-hat-closed.wav"

[voices."hat.open"]
output = "hats"
midi_note = 46
sample = "808-hat-open.wav"

# ... etc
```

The kit also declares an `instrument`, which tells rawdaw which built-in (or future plugin) plays this kit. The sampler reads the sample paths; a synth-based kit could declare a synth and have voices map to preset parameters instead of sample files.

## Multi-out: how the drum bus emerges

Because kits declare named outputs, the drum instrument is a **multi-output `AudioNode`**: its `outputs: &mut [&mut [f32]]` slice has one stereo pair per declared output, in declaration order.

When a kit is added to a track, the mixer auto-creates one channel per declared output. Adding the example kit above produces five mixer channels: `kick`, `snare`, `hats`, `perc`, `mix`. By default they're all routed to a "Drums" return track (auto-created on first multi-out instrument if one doesn't exist; reused if it does).

The user-visible flow:

1. Add a drum track with the `Vintage 808` kit.
2. Mixer shows the new drum channels grouped under the drum track (or however the UI presents it), each going to the `Drums` bus.
3. User puts a glue compressor and a touch of saturation on the `Drums` bus, sets the bus level, and that bus goes to master.

This is the standard pro drum-mixing workflow. No special "drum bus" concept in the data model — it's just a return track that the multi-out kit's channels happen to feed by default.

If a new kit is swapped in:
- Outputs that exist in both old and new kits keep their mixer channels and routing.
- New outputs that didn't exist before get auto-created channels routed to the same bus.
- Outputs that disappeared have their channels grayed/disabled (kept around in case the user swaps back; can be deleted explicitly).

## Humanization

Patterns stay clean (exact grid positions, exact velocities). Humanization (random velocity ±N, random timing ±N, swing/shuffle, accent rules) lives on the activation entry's `realization` block. The same pattern in two activation entries can humanize differently — the canonical pattern remains editable and legible.

## Engine implications summary

- `AudioNode` already takes `outputs: &mut [&mut [f32]]`; multi-out is the same API, just with N > 1.
- Node descriptors need to declare output names (and channel layout per output).
- Mixer model must support multi-out instruments: one source node, multiple downstream channel strips.
- No new RT-side mechanism beyond what the DAG already provides.

## Open questions

- **Articulation tag vocabulary.** Is articulation an open string namespace (kits declare what they support) or a fixed enum? Lean toward open, but a small fixed core (`ghost`, `accent`, `flam`, `roll`, `open`, `closed`, `half`, `mute`, `rim`, `bell`, `edge`, `bow`) for UI affordances.
- **Multi-channel layouts beyond stereo.** Mono outputs (for tight kick), 5.1/Atmos (not v1). Probably mono+stereo only.
- **Per-voice mixer pre-fader processing in the kit itself.** Some kits internally compress/EQ before output. Defer to the user putting plugins on the mixer channel; don't bake it into kits.

## Editor scope shipped in P3 (2026-05-21)

The drum step-grid editor shipped a deliberately minimal first
surface; a few items the doc anticipates remain deferred:

- **Click-to-toggle only; drag-to-set-velocity deferred.** Velocity
  lives on `DrumEvent` and is honored at realization, but P3 ships
  a binary on/off step grid. Velocity editing surface is a P3
  follow-up bite.
- **Grid resolution is hardcoded to `STRAIGHT_SIXTEENTH`.** The
  Signal-driven design for a per-pattern resolution selector is
  scoped in the pattern-editor plan's P2 § 6 but unimplemented.
- **`Extra(_)` voices have no creation UI.** The `DrumVoice::Extra`
  open namespace is decodable in the model but the editor can
  only edit voices that already exist; there's no `+ voice`
  affordance for naming a new `extras.*` slot.
