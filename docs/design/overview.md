# rawdaw — Overview

## What it is

rawdaw is a native desktop Digital Audio Workstation written in Rust, focused on **MIDI composition**. Its defining feature is a structural composition model — chord progressions, sections, and patterns are first-class reusable objects, not just bags of notes.

## Primary user

A composer who works top-down: picks a key, sketches song form, lays out chord loops, then realizes melodies on top. Frustrated that mainstream DAWs make this workflow into per-note busywork across many tracks. Reference points: Ableton, Cakewalk, GarageBand.

## In scope (v1)

- MIDI composition on a linear arrangement timeline.
- A structural composition model: key, sections, chord loops, patterns, tracks, clips (see `composition-model.md`).
- A handful of built-in instruments (a sampler and at least one synth — enough to actually hear what you're writing).
- Built-in audio effects: gain/pan, EQ, basic reverb/delay. Whatever's needed to mix the built-in instruments.
- A piano-roll-style detail editor for inspecting and tweaking clip realizations.
- A song-form / arrangement view as the centerpiece UI.
- MIDI I/O via `midir` for external keyboards and gear.
- Audio I/O via `cpal`. Linux is the primary target initially; cross-platform follows from cpal naturally.

## Out of scope (v1)

- **Plugin hosting (VST3 / CLAP / AU).** Deferred. When eventually built, it lives in a separate crate consumed by the main app. CLAP-first.
- **Audio editing / recording workflows.** No comping, no take management, no time-stretching, no audio clip editing beyond placing a sample. The DAW is not for tracking audio.
- **Live performance / clip launching / session view.** Not a goal.
- **Notation / score view.** Engraving is a separate discipline and isn't valuable for this user.
- **Web / WASM build.** Native desktop only — RT audio, plugin hosting (future), and latency floors all rule out the browser.
- **Mobile / tablet UI.** Desktop only.

## Non-goals (longer term)

- Becoming a generalist DAW. We will turn down features that pull rawdaw toward the "everything DAW" shape (audio engineering, mastering, live performance, broad genre coverage) if they conflict with composition focus.
