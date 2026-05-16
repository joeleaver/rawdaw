// rawdaw fixture data + design tokens for round-1 mockups.
// All component files read from `window.RD`.

(function () {
  // ─── Theme tokens ───────────────────────────────────────────────────────
  // Dark-default palette. The base is a near-black gray (not #000) so
  // elevated surfaces gain contrast through subtle lighter tints.
  const tokens = {
    // surfaces, low → high
    bg0: '#0F1115',        // window base
    bg1: '#15181E',        // arrangement / panes
    bg2: '#1B1F26',        // elevated cards, hover lift
    bg3: '#22262F',        // selected / pressed
    line: '#262B34',       // 1px borders
    lineSoft: '#1E222A',   // bar guides inside blocks

    text0: 'rgba(232,234,238,0.96)',
    text1: 'rgba(232,234,238,0.62)',
    text2: 'rgba(232,234,238,0.42)',
    text3: 'rgba(232,234,238,0.28)',

    accent: '#D5C8A6',     // a single warm neutral for focus rings / playhead
    danger: '#C76A6A',     // record dot, errors (status — separate from identity)
    ok: '#7FA88A',         // play arrow (status — separate from identity)

    // typography
    fontSans: '"Inter Tight","Inter Tight Fallback",ui-sans-serif,system-ui,sans-serif',
    fontNum:  '"Inter Tight","Inter Tight Fallback",ui-sans-serif,system-ui,sans-serif',

    // sizes
    h: { topbar: 48, ruler: 26, ribbon: 46, lane: 132, detail: 32 },
  };

  // ─── Identity palette (earthy / muted) ──────────────────────────────────
  // ~10 hues, moderate saturation. Assigned deterministically (by hand here
  // for fixture; production = name hash). Never used for status.
  const palette = {
    blue:    '#7C9EC2',
    terra:   '#B58A6B',
    sage:    '#8AA876',
    rose:    '#B5848F',
    olive:   '#A89A6B',
    plum:    '#9C84B5',
    teal:    '#6FA89E',
    sand:    '#C9A88E',
    slate:   '#8090A0',
    clay:    '#B07A6F',
  };

  // ─── Library: chord loops ───────────────────────────────────────────────
  // Roman is canonical; absolute is derived in C major for display only.
  // Case-by-quality convention: lowercase = minor, uppercase = major.
  // `quality` carries only non-default qualities (7, °, ø, sus4, etc.) —
  // the default quality for a degree's case is implicit.
  const chordLoops = {
    'verse-progression': {
      id: 'cl_verse', name: 'verse-progression',
      color: palette.terra, lengthBars: 4,
      events: [
        { bar: 0, roman: 'I',  quality: '', absolute: 'C'  },
        { bar: 1, roman: 'V',  quality: '', absolute: 'G'  },
        { bar: 2, roman: 'vi', quality: '', absolute: 'Am' },
        { bar: 3, roman: 'IV', quality: '', absolute: 'F'  },
      ],
    },
    'chorus-progression': {
      id: 'cl_chorus', name: 'chorus-progression',
      color: palette.olive, lengthBars: 4,
      events: [
        { bar: 0, roman: 'vi', quality: '', absolute: 'Am' },
        { bar: 1, roman: 'IV', quality: '', absolute: 'F'  },
        { bar: 2, roman: 'I',  quality: '', absolute: 'C'  },
        { bar: 3, roman: 'V',  quality: '', absolute: 'G'  },
      ],
    },
  };

  // ─── Library: patterns ──────────────────────────────────────────────────
  const patterns = {
    'bass-main':  { id: 'p_bass',  name: 'bass-main',  color: palette.teal,  kind: 'Pitched', variants: 2, meta: 'Pitched · 2 variants' },
    'lead-main':  { id: 'p_lead',  name: 'lead-main',  color: palette.plum,  kind: 'Pitched', variants: 1, meta: 'Pitched · 1 variant'  },
    'drums-main': { id: 'p_drums', name: 'drums-main', color: palette.sage,  kind: 'Drum',    variants: 2, meta: 'Drum · 2 variants'    },
    'pad-bed':    { id: 'p_pad',   name: 'pad-bed',    color: palette.slate, kind: 'Pitched', variants: 1, meta: 'Pitched · 1 variant'  },
  };

  // ─── Library: sections ──────────────────────────────────────────────────
  const sections = {
    intro: {
      id: 's_intro', name: 'intro', color: palette.rose,
      variants: [{ id: 'base', name: 'base' }],
      defaultVariant: 'base',
      baseDurationBars: 4,
      chordLoops: ['verse-progression'],
      activations: {
        bass:  { pattern: 'bass-main',  state: 'silent' },
        lead:  { pattern: 'lead-main',  state: 'silent' },
        drums: { pattern: 'drums-main', state: 'silent' },
        pad:   { pattern: 'pad-bed',    state: 'active' },
      },
    },
    verse: {
      id: 's_verse', name: 'verse', color: palette.blue,
      variants: [
        { id: 'base',     name: 'base'     },
        { id: 'stripped', name: 'stripped' },
      ],
      defaultVariant: 'base',
      baseDurationBars: 4,
      chordLoops: ['verse-progression'],
      activations: {
        bass:  { pattern: 'bass-main',  state: 'active' },
        lead:  { pattern: 'lead-main',  state: 'active' },
        drums: { pattern: 'drums-main', state: 'active' },
        pad:   { pattern: 'pad-bed',    state: 'inherit' },
      },
      // sparse override: stripped variant silences bass + drums
      variantOverrides: {
        stripped: {
          activations: { bass: 'silent', drums: 'silent' },
        },
      },
    },
    chorus: {
      id: 's_chorus', name: 'chorus', color: palette.sand,
      variants: [{ id: 'base', name: 'base' }],
      defaultVariant: 'base',
      baseDurationBars: 8,
      chordLoops: ['chorus-progression'],
      activations: {
        bass:  { pattern: 'bass-main',  state: 'active' },
        lead:  { pattern: 'lead-main',  state: 'active' },
        drums: { pattern: 'drums-main', state: 'active', subSchedule: 'fill@last' },
        pad:   { pattern: 'pad-bed',    state: 'active' },
      },
    },
  };

  // ─── Global tracks ──────────────────────────────────────────────────────
  const tracks = [
    { id: 't_bass',  name: 'bass',  kind: 'Pitched', role: 'bass'     },
    { id: 't_lead',  name: 'lead',  kind: 'Pitched', role: 'melodic'  },
    { id: 't_drums', name: 'drums', kind: 'Drum',    role: '—'        },
    { id: 't_pad',   name: 'pad',   kind: 'Pitched', role: 'pad'      },
  ];

  // ─── Arrangement (24 bars total) ────────────────────────────────────────
  // List of SectionRef-equivalents in time order.
  const arrangement = [
    { idx: 0, sectionKey: 'intro',  variant: 'base',     startBar: 0,  bars: 4 },
    { idx: 1, sectionKey: 'verse',  variant: 'base',     startBar: 4,  bars: 4 },
    { idx: 2, sectionKey: 'verse',  variant: 'stripped', startBar: 8,  bars: 4 },
    { idx: 3, sectionKey: 'verse',  variant: 'base',     startBar: 12, bars: 4 },
    { idx: 4, sectionKey: 'chorus', variant: 'base',     startBar: 16, bars: 8 },
  ];

  const totalBars = 24;

  // ─── Project meta ───────────────────────────────────────────────────────
  const project = {
    name: 'untitled-1',
    key: 'C major',
    timeSig: '4/4',
    tempo: 96,
    playhead: { bar: 5, beat: 2 }, // for the readout in the top bar
  };

  window.RD = {
    tokens, palette,
    chordLoops, patterns, sections, tracks,
    arrangement, totalBars, project,
  };
})();
