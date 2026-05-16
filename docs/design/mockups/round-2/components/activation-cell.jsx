// ActivationCell — the round-2 centerpiece. One cell per (section × track).
// Surfaces: pattern ref, state, voicing strategy, OctaveSpec, humanization
// (incl. seed), per-pattern variant schedule timeline, per-note override
// count. Inheritance comes from two sources: section variant ↑ section base,
// and realization params ↑ track role.

const ActivationCell = ({ section, track, currentVariant = 'base' }) => {
  const t = window.RD.tokens;
  const RD = window.RD;
  const base = section.activations[track.name];
  const ov = section.variantOverrides?.[currentVariant]?.activations?.[track.name];

  // Effective state for this variant.
  const variantSilent = ov === 'silent';
  const eff = variantSilent
    ? { ...base, state: 'silent', overridden: true }
    : { ...base, overridden: false };

  if (!base) return <CellInherit track={track} />;

  const pattern = RD.patterns[eff.pattern];
  const accentColor = pattern.color;

  return (
    <div style={{
      background: t.bg1, border: `1px solid ${t.line}`,
      borderRadius: 6, overflow: 'hidden',
      display: 'grid',
      gridTemplateColumns: '320px 1fr 520px',
      borderLeft: `3px solid ${accentColor}`,
      opacity: eff.state === 'silent' ? 0.78 : 1,
    }}>
      {/* COL 1 — identity, pattern, state */}
      <CellIdentity track={track} pattern={pattern} eff={eff}
        section={section} currentVariant={currentVariant} />

      {/* COL 2 — realization */}
      <CellRealization track={track} eff={eff} />

      {/* COL 3 — variant schedule + per-note overrides */}
      <CellSchedule track={track} pattern={pattern} eff={eff}
        section={section} currentVariant={currentVariant} />
    </div>
  );
};

// ─── Col 1: identity / pattern / state ────────────────────────────────────
const CellIdentity = ({ track, pattern, eff, section, currentVariant }) => {
  const t = window.RD.tokens;
  return (
    <div style={{
      padding: '12px 14px',
      borderRight: `1px solid ${t.line}`,
      display: 'flex', flexDirection: 'column', gap: 10,
      background: rgba(pattern.color, 0.04),
    }}>
      {/* Track row */}
      <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
        <Icon name={track.kind === 'Drum' ? 'pattern' : 'wave'} size={14} stroke={t.text1} />
        <span style={{ fontSize: 13.5, fontWeight: 600, color: t.text0, letterSpacing: -0.1 }}>
          {track.name}
        </span>
        <span style={{ fontSize: 10, color: t.text3, letterSpacing: 0.4, textTransform: 'uppercase' }}>
          {track.kind}
        </span>
        {track.role && (
          <Pill style={{ height: 16, padding: '0 5px' }}>
            role: {track.role}
          </Pill>
        )}
        <span style={{ flex: 1 }} />
        <StatePill state={eff.state} overridden={eff.overridden} />
      </div>

      {/* Pattern row */}
      <div style={{
        display: 'flex', alignItems: 'center', gap: 8,
        padding: '6px 8px', borderRadius: 4,
        background: t.bg0, border: `1px solid ${t.line}`,
      }}>
        <span style={{
          width: 9, height: 9, borderRadius: 2,
          background: pattern.color, flex: '0 0 auto',
          border: `1px solid ${rgba(pattern.color, 0.6)}`,
        }} />
        <span style={{ fontSize: 12.5, color: t.text0, fontWeight: 500 }}>{pattern.name}</span>
        <span style={{ fontSize: 11, color: t.text3 }}>· {pattern.kind}</span>
        <span style={{ flex: 1 }} />
        <button title="Open in pattern editor" style={iconBtn(t)}>
          <Icon name="chevron-r" size={13} stroke={t.text2} />
        </button>
      </div>

      {/* Footer hints */}
      <div style={{
        display: 'flex', gap: 12, marginTop: 'auto',
        fontSize: 11, color: t.text2,
        alignItems: 'center',
      }}>
        {(eff.perNoteOverrides ?? 0) > 0 && (
          <button style={{
            background: 'transparent', border: 0, padding: 0,
            color: t.text1, cursor: 'pointer',
            display: 'inline-flex', alignItems: 'center', gap: 4,
            fontSize: 11,
          }} title="open piano roll on pinned notes">
            <Icon name="dot" size={8} stroke={t.text1} />
            <span>{eff.perNoteOverrides} pinned</span>
            <Icon name="chevron-r" size={11} stroke={t.text2} />
          </button>
        )}
        {(eff.perNoteOverrides ?? 0) === 0 && (
          <span style={{ color: t.text3 }}>no per-note overrides</span>
        )}
      </div>
    </div>
  );
};

const CellInherit = ({ track }) => {
  const t = window.RD.tokens;
  return (
    <div style={{
      background: t.bg1, border: `1px dashed ${t.line}`,
      borderRadius: 6, padding: '14px 16px',
      display: 'flex', alignItems: 'center', gap: 10,
      color: t.text2, fontSize: 12.5,
    }}>
      <Icon name="dot" size={8} stroke={t.text3} />
      <strong style={{ fontWeight: 600, color: t.text1 }}>{track.name}</strong>
      <span>inherits from base · no entry in this variant</span>
    </div>
  );
};

// ─── Col 2: realization ──────────────────────────────────────────────────
const CellRealization = ({ track, eff }) => {
  const t = window.RD.tokens;
  const r = eff.realization || {};
  const isDrum = track.kind === 'Drum';

  return (
    <div style={{
      padding: '12px 14px',
      borderRight: `1px solid ${t.line}`,
      display: 'flex', flexDirection: 'column', gap: 8,
    }}>
      <SectionHeader>Realization</SectionHeader>

      {!isDrum && (
        <DropdownRow
          label="Voicing"
          value={window.RD.voicingLabels[r.voicing] || '—'}
          inheritedFrom={r.voicingFromRole ? `role: ${track.role}` : null}
          overridden={r.voicingFromRole === false}
        />
      )}

      {!isDrum && (
        <DropdownRow
          label="Octave"
          value={window.RD.octaveLabels[r.octave] || '—'}
          inheritedFrom={r.octaveFromRole ? `role: ${track.role}` : null}
          overridden={r.octaveFromRole === false}
        />
      )}

      {isDrum && (
        <div style={{ fontSize: 11, color: t.text3, marginTop: 2, marginBottom: 2 }}>
          drums are pitch-symbolic — voicing &amp; octave do not apply
        </div>
      )}

      <HumanizeRow h={r.humanization} />
    </div>
  );
};

const SectionHeader = ({ children, action }) => {
  const t = window.RD.tokens;
  return (
    <div style={{
      display: 'flex', alignItems: 'center',
      fontSize: 10, letterSpacing: 0.6, textTransform: 'uppercase',
      color: t.text2, fontWeight: 600, marginBottom: 2,
    }}>
      <span style={{ flex: 1 }}>{children}</span>
      {action}
    </div>
  );
};

const DropdownRow = ({ label, value, inheritedFrom, overridden }) => {
  const t = window.RD.tokens;
  return (
    <div style={{ display: 'grid', gridTemplateColumns: '64px 1fr auto', gap: 8, alignItems: 'center' }}>
      <span style={{ fontSize: 11.5, color: t.text2 }}>{label}</span>
      <div style={{
        display: 'flex', alignItems: 'center', gap: 6,
        padding: '4px 8px', borderRadius: 4,
        background: t.bg0, border: `1px solid ${t.line}`,
        fontSize: 12, color: t.text0,
      }}>
        <span style={{ flex: 1, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
          {value}
        </span>
        <Icon name="chevron-d" size={11} stroke={t.text2} />
      </div>
      <div style={{ minWidth: 90, fontSize: 10.5 }}>
        {inheritedFrom && (
          <span title={`inherited from ${inheritedFrom}`} style={{
            display: 'inline-flex', alignItems: 'center', gap: 3,
            color: t.text3, cursor: 'help',
            padding: '1px 5px', borderRadius: 2,
            border: `1px solid ${t.lineSoft}`,
            letterSpacing: 0.2,
          }}>
            ↳ {inheritedFrom}
          </span>
        )}
        {overridden && <InheritMark kind="overridden" />}
      </div>
    </div>
  );
};

const HumanizeRow = ({ h }) => {
  const t = window.RD.tokens;
  if (!h) return null;
  return (
    <div style={{
      display: 'grid', gridTemplateColumns: '64px 1fr', gap: 8,
      alignItems: 'start', marginTop: 4,
    }}>
      <span style={{ fontSize: 11.5, color: t.text2, paddingTop: 5 }}>Humanize</span>
      <div style={{ display: 'flex', flexWrap: 'wrap', gap: 10, alignItems: 'center' }}>
        <MicroSlider label="vel"   value={`${Math.round(h.velocity * 100)}%`} fill={h.velocity / 0.20} />
        <MicroSlider label="tim"   value={`${h.timing}t`}                     fill={h.timing / 16} />
        <MicroSlider label="swing" value={h.swing === 0 ? 'straight' : `${Math.round(h.swing*100)}%`}  fill={h.swing / 0.5} />
        <SeedField seed={h.seed} />
      </div>
    </div>
  );
};

const MicroSlider = ({ label, value, fill }) => {
  const t = window.RD.tokens;
  const w = 64;
  return (
    <div style={{ display: 'inline-flex', flexDirection: 'column', gap: 2 }}>
      <div style={{
        display: 'flex', alignItems: 'baseline', justifyContent: 'space-between',
        fontSize: 10, color: t.text2, gap: 6, lineHeight: 1,
      }}>
        <span style={{ letterSpacing: 0.3 }}>{label}</span>
        <span style={{
          color: t.text0, fontFeatureSettings: '"tnum" 1',
          fontVariantNumeric: 'tabular-nums',
        }}>{value}</span>
      </div>
      <div style={{
        width: w, height: 4, borderRadius: 2,
        background: t.bg0, border: `1px solid ${t.line}`,
        position: 'relative', overflow: 'hidden',
      }}>
        <div style={{
          position: 'absolute', inset: 0,
          width: `${Math.max(2, Math.min(100, fill * 100))}%`,
          background: 'rgba(213,200,166,0.55)',
        }} />
      </div>
    </div>
  );
};

const SeedField = ({ seed }) => {
  const t = window.RD.tokens;
  return (
    <div style={{ display: 'inline-flex', flexDirection: 'column', gap: 2 }}>
      <div style={{ fontSize: 10, color: t.text2, letterSpacing: 0.3, lineHeight: 1 }}>seed</div>
      <div style={{
        display: 'inline-flex', alignItems: 'center', gap: 4,
        padding: '2px 4px 2px 6px', borderRadius: 3,
        background: t.bg0, border: `1px solid ${t.line}`,
        fontSize: 11, color: t.text0,
        fontFeatureSettings: '"tnum" 1', fontVariantNumeric: 'tabular-nums',
      }}>
        <span style={{ minWidth: 30 }}>{seed}</span>
        <button title="re-roll humanization seed" style={{
          ...iconBtn(t), width: 16, height: 16, borderRadius: 2,
        }}>
          <svg width="11" height="11" viewBox="0 0 24 24" fill="none"
            stroke={t.text2} strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
            <path d="M20 11a8 8 0 1 0 -3 6.3" />
            <polyline points="20 4 20 11 13 11" />
          </svg>
        </button>
      </div>
    </div>
  );
};

// ─── Col 3: pattern-variant schedule + per-note overrides ────────────────
const CellSchedule = ({ track, pattern, eff, section, currentVariant }) => {
  const t = window.RD.tokens;
  const totalBars = (section.variantOverrides?.[currentVariant]?.durationBars) ?? section.baseDurationBars;
  const schedule = eff.variantSchedule;
  const hasSchedule = !!(schedule && schedule.length);

  // Build segments: if no schedule, the whole range is the pattern's default
  // variant. Otherwise tile the schedule and fill gaps with the default.
  const segments = (() => {
    if (!hasSchedule) {
      return [{ range: [0, totalBars], variant: pattern.variants && pattern.variants > 1 ? 'main' : 'main', isDefault: true }];
    }
    return schedule.map(s => ({ ...s, isDefault: false }));
  })();

  // Note: pattern variants are conceptually distinct from "main" — show fill
  // / build / etc as accent strokes within the pattern's identity color.
  return (
    <div style={{ padding: '12px 14px', display: 'flex', flexDirection: 'column', gap: 10 }}>
      <SectionHeader
        action={hasSchedule
          ? <span style={{ fontSize: 10, color: t.text3, letterSpacing: 0.4 }}>
              {segments.length} segment{segments.length === 1 ? '' : 's'}
            </span>
          : <span style={{ fontSize: 10, color: t.text3, letterSpacing: 0.4 }}>
              default variant only
            </span>}>
        Variant schedule
      </SectionHeader>

      {/* Timeline */}
      <ScheduleTimeline
        totalBars={totalBars}
        segments={segments}
        color={pattern.color}
        silent={eff.state === 'silent'}
      />

      {/* Variant legend */}
      <div style={{
        display: 'flex', flexWrap: 'wrap', gap: 8,
        fontSize: 10.5, color: t.text2, alignItems: 'center',
      }}>
        <LegendChip color={pattern.color} variant="main"  isDefault />
        {hasSchedule && segments.filter(s => s.variant !== 'main').map((s, i) => (
          <LegendChip key={i} color={pattern.color} variant={s.variant}
            range={`bar ${s.range[0] + 1}–${s.range[1]}`} />
        ))}
        <span style={{ marginLeft: 'auto', color: t.text3, fontSize: 10 }}>
          click a sub-range to assign a pattern variant
        </span>
      </div>
    </div>
  );
};

const ScheduleTimeline = ({ totalBars, segments, color, silent }) => {
  const t = window.RD.tokens;
  const H = 28;
  return (
    <div style={{
      position: 'relative', height: H,
      background: t.bg0, border: `1px solid ${t.line}`, borderRadius: 4,
      overflow: 'hidden',
    }}>
      {/* per-bar grid */}
      <svg width="100%" height={H} style={{ position: 'absolute', inset: 0 }}>
        {Array.from({ length: totalBars + 1 }, (_, i) => (
          <line key={i}
            x1={`${(i / totalBars) * 100}%`} y1={0}
            x2={`${(i / totalBars) * 100}%`} y2={H}
            stroke={t.lineSoft} strokeWidth={1}
          />
        ))}
      </svg>

      {/* segments */}
      {segments.map((s, i) => {
        const x = (s.range[0] / totalBars) * 100;
        const w = ((s.range[1] - s.range[0]) / totalBars) * 100;
        const isFill = !s.isDefault;
        return (
          <div key={i} style={{
            position: 'absolute',
            left: `${x}%`, top: 3, bottom: 3,
            width: `${w}%`,
            background: silent
              ? 'rgba(232,234,238,0.05)'
              : rgba(color, isFill ? 0.34 : 0.16),
            border: `1px solid ${rgba(color, isFill ? 0.7 : 0.35)}`,
            borderRadius: 2,
            display: 'flex', alignItems: 'center', justifyContent: 'center',
            fontSize: 10.5, color: 'rgba(232,234,238,0.86)',
            fontWeight: isFill ? 600 : 500,
            letterSpacing: 0.2,
            backgroundImage: isFill
              ? `repeating-linear-gradient(135deg, transparent 0 4px, ${rgba(color, 0.18)} 4px 6px)`
              : 'none',
            cursor: 'pointer',
          }}>
            {s.variant}
          </div>
        );
      })}

      {/* bar number ticks at bottom */}
      <svg width="100%" height={H} style={{ position: 'absolute', inset: 0, pointerEvents: 'none' }}>
        {Array.from({ length: totalBars }, (_, i) => (
          <text key={i}
            x={`${((i + 0.5) / totalBars) * 100}%`} y={H - 2}
            textAnchor="middle"
            fontSize="9" fill={t.text3}
            fontFamily={t.fontNum}
            style={{ fontFeatureSettings: '"tnum" 1' }}
          >{i + 1}</text>
        ))}
      </svg>
    </div>
  );
};

const LegendChip = ({ color, variant, range, isDefault }) => {
  const t = window.RD.tokens;
  return (
    <span style={{
      display: 'inline-flex', alignItems: 'center', gap: 5,
      padding: '1px 6px 1px 4px', borderRadius: 2,
      border: `1px solid ${rgba(color, 0.4)}`,
      background: rgba(color, isDefault ? 0.10 : 0.20),
      fontSize: 10.5, color: t.text1,
    }}>
      <span style={{ width: 7, height: 7, borderRadius: 1.5, background: color, flex: '0 0 auto' }} />
      <span>{variant}</span>
      {isDefault && <span style={{ color: t.text3, fontSize: 9.5, letterSpacing: 0.3, textTransform: 'uppercase' }}>default</span>}
      {range && <span style={{ color: t.text3, fontSize: 9.5 }}>{range}</span>}
    </span>
  );
};

const iconBtn = (t) => ({
  width: 22, height: 22, borderRadius: 3, border: 0,
  background: 'transparent', color: t.text2, cursor: 'pointer',
  display: 'inline-flex', alignItems: 'center', justifyContent: 'center',
  padding: 0,
});

Object.assign(window, { ActivationCell });
