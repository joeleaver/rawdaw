// Inspector (right pane). Shows what's selected; offers most-common edits.

const Inspector = ({ width = 320, selectedIdx = null }) => {
  const t = window.RD.tokens;
  const RD = window.RD;

  // Empty state
  if (selectedIdx == null) {
    return (
      <aside style={{
        width, flex: `0 0 ${width}px`,
        background: t.bg1, borderLeft: `1px solid ${t.line}`,
        display: 'flex', flexDirection: 'column', minHeight: 0,
      }}>
        <InspectorEmpty />
      </aside>
    );
  }

  const block   = RD.arrangement[selectedIdx];
  const section = RD.sections[block.sectionKey];
  const linkedRefs = RD.arrangement.filter(b => b.sectionKey === block.sectionKey).length;

  return (
    <aside style={{
      width, flex: `0 0 ${width}px`,
      background: t.bg1, borderLeft: `1px solid ${t.line}`,
      display: 'flex', flexDirection: 'column', minHeight: 0,
    }}>
      {/* Header — identity color + name + count of refs */}
      <InspectorHeader section={section} block={block} linkedRefs={linkedRefs} />

      {/* Variant tab strip */}
      <VariantTabs section={section} active={block.variant} />

      {/* Form */}
      <div style={{ flex: 1, overflowY: 'auto', padding: '12px 14px', minHeight: 0 }}>
        <Field label="Duration">
          <NumStepper value={section.baseDurationBars} unit="bars" />
          {/*
            Inheritance signal is unified on a single asterisk (see
            StatePill below). For the base variant nothing is inherited;
            for `stripped` or other non-default variants, any field that
            inherits from base would carry an asterisk next to its label
            with a "inherited from base" tooltip. Round 1 shows the base
            variant of `verse`, so no markers fire here.
          */}
        </Field>

        <Field label="Scale override">
          <Select value="Inherit project key (C major)" />
        </Field>

        <Field label="Chord loops">
          <ChordLoopRow
            barRange="0..4"
            loopName={section.chordLoops[0]}
            color={RD.chordLoops[section.chordLoops[0]].color}
          />
          <AddRow label="add chord loop" />
        </Field>

        <Field label="Activations">
          <ActivationTable section={section} block={block} />
        </Field>
      </div>

      {/* Footer */}
      <div style={{
        flex: '0 0 auto', padding: '10px 14px',
        borderTop: `1px solid ${t.line}`,
        display: 'flex', gap: 6,
      }}>
        <FooterBtn>Duplicate placement</FooterBtn>
        <FooterBtn primary>Open in editor</FooterBtn>
      </div>
    </aside>
  );
};

const InspectorEmpty = () => {
  const t = window.RD.tokens;
  return (
    <div style={{
      flex: 1, display: 'flex', flexDirection: 'column',
      alignItems: 'center', justifyContent: 'center',
      padding: 24, gap: 8, textAlign: 'center',
    }}>
      <div style={{ width: 28, height: 28, opacity: 0.4 }}>
        <Icon name="dot" size={28} stroke={t.text3} />
      </div>
      <div style={{ fontSize: 12.5, color: t.text1, lineHeight: 1.5, maxWidth: 240 }}>
        Select a section, pattern, or chord to inspect.
      </div>
    </div>
  );
};

const InspectorHeader = ({ section, block, linkedRefs }) => {
  const t = window.RD.tokens;
  return (
    <div style={{
      padding: '12px 14px', borderBottom: `1px solid ${t.line}`,
      display: 'flex', flexDirection: 'column', gap: 6,
      background: rgba(section.color, 0.06),
      borderLeft: `3px solid ${section.color}`,
    }}>
      <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
        <div style={{
          fontSize: 15, fontWeight: 600, color: t.text0, letterSpacing: -0.2,
        }}>{section.name}</div>
        {block.variant !== section.defaultVariant && (
          <Pill color={section.color}>variant: {block.variant}</Pill>
        )}
      </div>
      <div style={{
        fontSize: 11, color: t.text2,
        display: 'flex', alignItems: 'center', gap: 10,
      }}>
        <span>{linkedRefs} {linkedRefs === 1 ? 'instance' : 'instances'} in arrangement</span>
        <span style={{ color: t.text3 }}>·</span>
        <span style={{ fontFeatureSettings: '"tnum" 1' }}>
          bar {block.startBar + 1}–{block.startBar + block.bars}
        </span>
      </div>
    </div>
  );
};

const VariantTabs = ({ section, active }) => {
  const t = window.RD.tokens;
  return (
    <div style={{
      display: 'flex', padding: '8px 10px 0', gap: 2,
      background: t.bg1, borderBottom: `1px solid ${t.line}`,
    }}>
      {section.variants.map(v => {
        const isActive = v.id === active;
        return (
          <button key={v.id} style={{
            padding: '6px 10px 7px', borderRadius: '4px 4px 0 0',
            border: 0,
            borderBottom: isActive ? `2px solid ${section.color}` : '2px solid transparent',
            background: isActive ? t.bg2 : 'transparent',
            color: isActive ? t.text0 : t.text1,
            fontSize: 11.5, fontWeight: isActive ? 600 : 500,
            cursor: 'pointer', position: 'relative', top: 1,
          }}>
            {v.name}
            {v.id === section.defaultVariant && (
              <span style={{ marginLeft: 6, fontSize: 9.5, color: t.text3, letterSpacing: 0.4, textTransform: 'uppercase' }}>
                default
              </span>
            )}
          </button>
        );
      })}
      <button style={{
        padding: '6px 8px', borderRadius: 3, border: 0,
        background: 'transparent', color: t.text2, cursor: 'pointer',
        display: 'inline-flex', alignItems: 'center', gap: 4, marginLeft: 'auto',
        fontSize: 11,
      }}>
        <Icon name="plus" size={11} stroke={t.text2} /> variant
      </button>
    </div>
  );
};

const Field = ({ label, children, inherited }) => {
  const t = window.RD.tokens;
  return (
    <div style={{ marginBottom: 14 }}>
      <div style={{
        fontSize: 10.5, letterSpacing: 0.6, textTransform: 'uppercase',
        color: t.text2, fontWeight: 600, marginBottom: 6,
        display: 'flex', alignItems: 'center', gap: 4,
      }}>
        <span>{label}</span>
        {inherited && <InheritMark />}
      </div>
      <div style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
        {children}
      </div>
    </div>
  );
};

// Unified inheritance / override mark. Used by both inspector form fields
// and activation table rows. Asterisk + native tooltip is the cheapest
// signal that survives compaction.
const InheritMark = ({ kind = 'inherited' }) => {
  const t = window.RD.tokens;
  const title = kind === 'overridden'
    ? 'overridden in this variant from base'
    : 'inherited from base';
  return (
    <span title={title} style={{
      color: t.text2, fontSize: 11, lineHeight: 1,
      cursor: 'help', userSelect: 'none',
    }}>*</span>
  );
};

const NumStepper = ({ value, unit }) => {
  const t = window.RD.tokens;
  return (
    <div style={{
      display: 'inline-flex', alignItems: 'stretch',
      background: t.bg0, border: `1px solid ${t.line}`, borderRadius: 4,
      width: 'fit-content',
    }}>
      <button style={btnSegStyle(t, 'left')}><Icon name="minus" size={12} stroke={t.text1} /></button>
      <div style={{
        padding: '4px 10px', minWidth: 30, textAlign: 'center',
        fontSize: 12.5, color: t.text0,
        fontFeatureSettings: '"tnum" 1', fontVariantNumeric: 'tabular-nums',
        borderLeft: `1px solid ${t.line}`, borderRight: `1px solid ${t.line}`,
      }}>{value}</div>
      <button style={btnSegStyle(t, 'right')}><Icon name="plus" size={12} stroke={t.text1} /></button>
      <div style={{
        padding: '4px 10px', fontSize: 11, color: t.text2, alignSelf: 'center',
        borderLeft: `1px solid ${t.line}`,
      }}>{unit}</div>
    </div>
  );
};

const btnSegStyle = (t, side) => ({
  background: 'transparent', border: 0, padding: '0 8px',
  color: t.text1, cursor: 'pointer',
  display: 'flex', alignItems: 'center', justifyContent: 'center',
});

const Select = ({ value }) => {
  const t = window.RD.tokens;
  return (
    <div style={{
      display: 'flex', alignItems: 'center',
      padding: '5px 10px', borderRadius: 4,
      background: t.bg0, border: `1px solid ${t.line}`,
      fontSize: 12, color: t.text0,
    }}>
      <span style={{ flex: 1 }}>{value}</span>
      <Icon name="chevron-d" size={12} stroke={t.text2} />
    </div>
  );
};

const ChordLoopRow = ({ barRange, loopName, color }) => {
  const t = window.RD.tokens;
  const RD = window.RD;
  const loop = RD.chordLoops[loopName];
  return (
    <div style={{
      display: 'flex', alignItems: 'center', gap: 8,
      padding: '6px 8px', borderRadius: 4,
      background: t.bg0, border: `1px solid ${t.line}`,
    }}>
      <span style={{
        width: 8, height: 8, borderRadius: 2,
        background: color, flex: '0 0 auto',
      }} />
      <span style={{ fontSize: 11, color: t.text2,
        fontFeatureSettings: '"tnum" 1', minWidth: 30,
      }}>{barRange}</span>
      <span style={{ flex: 1, fontSize: 12, color: t.text0 }}>{loopName}</span>
      <span style={{ fontSize: 11, color: t.text2, letterSpacing: 0.4,
        fontFeatureSettings: '"tnum" 1',
      }}>{loop.events.map(e => e.roman).join(' ')}</span>
    </div>
  );
};

const AddRow = ({ label }) => {
  const t = window.RD.tokens;
  return (
    <button style={{
      display: 'flex', alignItems: 'center', gap: 6,
      padding: '4px 8px', borderRadius: 3,
      background: 'transparent',
      border: `1px dashed ${t.line}`,
      color: t.text2, cursor: 'pointer',
      fontSize: 11.5,
    }}>
      <Icon name="plus" size={11} stroke={t.text2} /> {label}
    </button>
  );
};

const ActivationTable = ({ section, block }) => {
  const t = window.RD.tokens;
  const RD = window.RD;

  // Determine effective state per track for this variant.
  const effective = (track) => {
    const baseA = section.activations[track.name];
    if (!baseA) return { pattern: null, state: 'inherit' };
    // sparse variant override
    const ov = section.variantOverrides?.[block.variant]?.activations?.[track.name];
    if (ov === 'silent') return { pattern: baseA.pattern, state: 'silent', overridden: true };
    return { ...baseA, overridden: false };
  };

  return (
    <div style={{
      borderRadius: 4, overflow: 'hidden',
      border: `1px solid ${t.line}`, background: t.bg0,
    }}>
      <div style={{
        display: 'grid', gridTemplateColumns: '64px 1fr 60px',
        padding: '5px 8px', gap: 8,
        fontSize: 10, color: t.text2, letterSpacing: 0.5, textTransform: 'uppercase',
        borderBottom: `1px solid ${t.line}`, background: t.bg1,
      }}>
        <div>Track</div><div>Pattern</div><div style={{ textAlign: 'right' }}>State</div>
      </div>
      {RD.tracks.map(track => {
        const eff = effective(track);
        const pat = eff.pattern ? RD.patterns[eff.pattern] : null;
        return (
          <div key={track.id} style={{
            display: 'grid', gridTemplateColumns: '64px 1fr 60px',
            padding: '6px 8px', gap: 8, alignItems: 'center',
            borderBottom: `1px solid ${t.lineSoft}`,
            fontSize: 12,
          }}>
            <div style={{ color: t.text0, display: 'flex', alignItems: 'center', gap: 5 }}>
              <span style={{
                width: 6, height: 6, borderRadius: '50%',
                background: track.kind === 'Drum' ? t.text2 : t.text1, opacity: 0.6,
              }} />
              {track.name}
            </div>
            <div style={{
              color: pat ? t.text1 : t.text3,
              display: 'flex', alignItems: 'center', gap: 5,
            }}>
              {pat && <span style={{
                width: 7, height: 7, borderRadius: 1.5,
                background: pat.color, flex: '0 0 auto',
              }} />}
              <span style={{
                overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap',
                fontStyle: eff.state === 'inherit' ? 'italic' : 'normal',
              }}>{pat ? pat.name : '—'}</span>
            </div>
            <div style={{ textAlign: 'right' }}>
              <StatePill state={eff.state} overridden={eff.overridden} />
            </div>
          </div>
        );
      })}
    </div>
  );
};

const StatePill = ({ state, overridden }) => {
  const t = window.RD.tokens;
  const styles = {
    active:  { bg: 'rgba(127,168,138,0.18)',  fg: '#A8C9B0',         border: 'rgba(127,168,138,0.40)' },
    silent:  { bg: 'rgba(232,234,238,0.06)',  fg: t.text2,           border: 'rgba(232,234,238,0.14)' },
    inherit: { bg: 'transparent',             fg: t.text3,           border: 'rgba(232,234,238,0.14)' },
  }[state];
  return (
    <span style={{
      display: 'inline-flex', alignItems: 'center', gap: 4,
      padding: '1px 6px', borderRadius: 3,
      fontSize: 10, fontWeight: 500, letterSpacing: 0.3,
      background: styles.bg, color: styles.fg,
      border: `1px solid ${styles.border}`,
    }}>
      {state}
      {overridden && <InheritMark kind="overridden" />}
    </span>
  );
};

const FooterBtn = ({ children, primary }) => {
  const t = window.RD.tokens;
  return (
    <button style={{
      flex: 1, padding: '6px 10px', borderRadius: 4,
      background: primary ? t.bg3 : t.bg2,
      border: `1px solid ${primary ? t.line : t.line}`,
      color: primary ? t.text0 : t.text1,
      fontSize: 12, fontWeight: 500, cursor: 'pointer',
    }}>{children}</button>
  );
};

Object.assign(window, { Inspector });
