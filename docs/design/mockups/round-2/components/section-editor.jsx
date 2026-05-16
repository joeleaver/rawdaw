// SectionEditor — round-2 surface. Edits one Section: variant tabs,
// duration / scale / chord-loop schedule, then a list of activation cells.

const SectionEditor = ({ sectionKey, currentVariant = 'base' }) => {
  const t = window.RD.tokens;
  const RD = window.RD;
  const section = RD.sections[sectionKey];

  return (
    <div style={{
      flex: 1, minHeight: 0, display: 'flex', flexDirection: 'column',
      background: t.bg0, color: t.text0,
      fontFamily: t.fontSans, fontSize: 13,
    }}>
      <SectionEditorHeader section={section} currentVariant={currentVariant} />
      <SectionMetaBar    section={section} currentVariant={currentVariant} />

      {/* Activations */}
      <div style={{ flex: 1, overflowY: 'auto', padding: '8px 20px 18px' }}>
        <div style={{
          display: 'flex', alignItems: 'center', gap: 8,
          padding: '6px 0 8px',
        }}>
          <span style={{
            fontSize: 10.5, letterSpacing: 0.6, textTransform: 'uppercase',
            color: t.text2, fontWeight: 600,
          }}>Activations</span>
          <span style={{ fontSize: 11, color: t.text3 }}>· {RD.tracks.length} tracks</span>
          <span style={{ flex: 1 }} />
          <button style={{
            display: 'inline-flex', alignItems: 'center', gap: 4,
            padding: '4px 8px', borderRadius: 3,
            background: 'transparent', border: `1px solid ${t.line}`,
            color: t.text1, cursor: 'pointer', fontSize: 11,
          }}>
            <Icon name="plus" size={11} stroke={t.text1} />
            <span>add track</span>
          </button>
        </div>

        <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
          {RD.tracks.map(track => (
            <ActivationCell key={track.id}
              section={section} track={track}
              currentVariant={currentVariant} />
          ))}
        </div>
      </div>
    </div>
  );
};

// ─── Header: name · variant tabs · "open in arrangement" ─────────────────
const SectionEditorHeader = ({ section, currentVariant }) => {
  const t = window.RD.tokens;
  return (
    <div style={{
      flex: '0 0 auto',
      borderBottom: `1px solid ${t.line}`,
      background: t.bg1,
      padding: '10px 20px 0',
      display: 'flex', flexDirection: 'column', gap: 8,
    }}>
      <div style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
        <button style={{
          ...iconBtn(t), width: 26, height: 26,
          border: `1px solid ${t.line}`, borderRadius: 4,
        }} title="Back to arrangement">
          <Icon name="chevron-r" size={13} stroke={t.text1} />
        </button>
        <span style={{ fontSize: 11, color: t.text3 }}>
          arrangement · sections ·
        </span>
        <span style={{
          width: 4, height: 18, borderRadius: 2,
          background: section.color, flex: '0 0 auto',
        }} />
        <h1 style={{
          margin: 0, fontSize: 16, fontWeight: 700,
          color: t.text0, letterSpacing: -0.3,
        }}>{section.name}</h1>
        <span style={{ fontSize: 11, color: t.text2 }}>
          section editor
        </span>

        <span style={{ flex: 1 }} />

        <button style={{
          padding: '5px 10px', borderRadius: 4,
          background: t.bg0, border: `1px solid ${t.line}`,
          color: t.text1, cursor: 'pointer', fontSize: 11.5,
        }}>Duplicate section</button>
        <button style={{
          padding: '5px 10px', borderRadius: 4,
          background: t.bg2, border: `1px solid ${t.line}`,
          color: t.text0, cursor: 'pointer', fontSize: 11.5, fontWeight: 500,
        }}>Done</button>
      </div>

      {/* Variant tabs */}
      <div style={{
        display: 'flex', gap: 2, marginTop: 2,
      }}>
        {section.variants.map(v => {
          const isActive = v.id === currentVariant;
          return (
            <button key={v.id} style={{
              padding: '7px 14px 8px',
              borderRadius: '4px 4px 0 0',
              border: 0,
              borderBottom: isActive
                ? `2px solid ${section.color}`
                : '2px solid transparent',
              background: isActive ? t.bg0 : 'transparent',
              color: isActive ? t.text0 : t.text1,
              fontSize: 12.5, fontWeight: isActive ? 600 : 500,
              cursor: 'pointer', position: 'relative', top: 1,
              letterSpacing: -0.05,
            }}>
              {v.name}
              {v.id === section.defaultVariant && (
                <span style={{ marginLeft: 8, fontSize: 9.5, color: t.text3, letterSpacing: 0.4, textTransform: 'uppercase' }}>
                  default
                </span>
              )}
            </button>
          );
        })}
        <button style={{
          padding: '6px 8px', borderRadius: 3, border: 0,
          background: 'transparent', color: t.text2, cursor: 'pointer',
          display: 'inline-flex', alignItems: 'center', gap: 4,
          marginLeft: 4, fontSize: 11.5, alignSelf: 'center',
        }}>
          <Icon name="plus" size={11} stroke={t.text2} /> variant
        </button>
      </div>
    </div>
  );
};

// ─── Meta bar: duration · scale · chord-loop schedule ────────────────────
const SectionMetaBar = ({ section, currentVariant }) => {
  const t = window.RD.tokens;
  const RD = window.RD;

  // For variant != base, fields that aren't overridden are inherited.
  const variantOv = section.variantOverrides?.[currentVariant];
  const isBase = currentVariant === section.defaultVariant;
  const durationInherited  = !isBase && (variantOv?.durationBars == null);
  const scaleInherited     = !isBase && (variantOv?.scaleOverride == null);
  const chordLoopInherited = !isBase && (variantOv?.chordLoops == null);

  const dur = section.baseDurationBars;
  const loopName = section.chordLoops[0];
  const loop = RD.chordLoops[loopName];

  return (
    <div style={{
      flex: '0 0 auto',
      padding: '12px 20px',
      borderBottom: `1px solid ${t.line}`,
      display: 'grid',
      gridTemplateColumns: '200px 240px 1fr',
      gap: 18,
      alignItems: 'stretch',
    }}>
      {/* Duration */}
      <MetaField label="Duration" inherited={durationInherited}>
        <NumStepperMini value={dur} unit="bars" />
      </MetaField>

      {/* Scale override */}
      <MetaField label="Scale override" inherited={scaleInherited}>
        <PseudoSelect value="Inherit project key (C major)" />
      </MetaField>

      {/* Chord loops */}
      <MetaField label="Chord loops" inherited={chordLoopInherited}>
        <ChordLoopBar dur={dur} loop={loop} />
      </MetaField>
    </div>
  );
};

const MetaField = ({ label, inherited, children }) => {
  const t = window.RD.tokens;
  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 5 }}>
      <div style={{
        fontSize: 10, letterSpacing: 0.6, textTransform: 'uppercase',
        color: t.text2, fontWeight: 600,
        display: 'flex', alignItems: 'center', gap: 5,
      }}>
        <span>{label}</span>
        {inherited && (
          <span title="inherited from base" style={{
            color: t.text3, fontSize: 10, cursor: 'help',
            padding: '0 4px', border: `1px solid ${t.lineSoft}`,
            borderRadius: 2, letterSpacing: 0.2, textTransform: 'none',
          }}>↳ base</span>
        )}
      </div>
      {children}
    </div>
  );
};

const NumStepperMini = ({ value, unit }) => {
  const t = window.RD.tokens;
  return (
    <div style={{
      display: 'inline-flex', alignItems: 'stretch',
      background: t.bg0, border: `1px solid ${t.line}`, borderRadius: 4,
      width: 'fit-content',
    }}>
      <button style={{ ...iconBtn(t), width: 24, height: 24 }}>
        <Icon name="minus" size={12} stroke={t.text1} />
      </button>
      <div style={{
        padding: '3px 12px', minWidth: 28, textAlign: 'center',
        fontSize: 12.5, color: t.text0,
        fontFeatureSettings: '"tnum" 1', fontVariantNumeric: 'tabular-nums',
        borderLeft: `1px solid ${t.line}`, borderRight: `1px solid ${t.line}`,
      }}>{value}</div>
      <button style={{ ...iconBtn(t), width: 24, height: 24 }}>
        <Icon name="plus" size={12} stroke={t.text1} />
      </button>
      <div style={{
        padding: '3px 10px', fontSize: 11, color: t.text2, alignSelf: 'center',
        borderLeft: `1px solid ${t.line}`,
      }}>{unit}</div>
    </div>
  );
};

const PseudoSelect = ({ value }) => {
  const t = window.RD.tokens;
  return (
    <div style={{
      display: 'flex', alignItems: 'center',
      padding: '4px 10px', borderRadius: 4,
      background: t.bg0, border: `1px solid ${t.line}`,
      fontSize: 12, color: t.text0,
    }}>
      <span style={{ flex: 1 }}>{value}</span>
      <Icon name="chevron-d" size={12} stroke={t.text2} />
    </div>
  );
};

// Mini chord-loop strip: a horizontal block per chord cell, scaled to the
// section duration. Shows the schedule (only one loop in this fixture) and
// the chord cells' Roman + absolute labels.
const ChordLoopBar = ({ dur, loop }) => {
  const t = window.RD.tokens;
  // Tile the loop across the section.
  const cells = [];
  let bar = 0;
  while (bar < dur) {
    loop.events.forEach((ev, ei) => {
      if (bar >= dur) return;
      cells.push({ ...ev, bar, isFirstOfLoop: ei === 0 });
      bar += 1;
    });
  }

  return (
    <div style={{
      display: 'flex',
      gap: 2,
      height: 38,
      borderRadius: 4,
      overflow: 'hidden',
      background: t.bg0,
      border: `1px solid ${t.line}`,
      padding: 2,
    }}>
      {cells.map((c, i) => (
        <div key={i} style={{
          flex: 1, minWidth: 0,
          background: rgba(loop.color, 0.10),
          border: `1px solid ${rgba(loop.color, 0.30)}`,
          borderLeft: c.isFirstOfLoop ? `2px solid ${loop.color}` : `1px solid ${rgba(loop.color, 0.30)}`,
          borderRadius: 2,
          display: 'flex', flexDirection: 'column',
          alignItems: 'center', justifyContent: 'center',
          gap: 1, padding: '2px 4px',
        }}>
          <Roman size={12} color={t.text0}>{c.roman}{c.quality}</Roman>
          <span style={{
            fontSize: 9, color: t.text2,
            fontFeatureSettings: '"tnum" 1', lineHeight: 1,
          }}>{c.absolute}</span>
        </div>
      ))}
      <button title="Add chord-loop range" style={{
        ...iconBtn(t),
        width: 22, height: '100%',
        borderRadius: 2, alignSelf: 'stretch',
        border: `1px dashed ${t.line}`,
        marginLeft: 2,
      }}>
        <Icon name="plus" size={11} stroke={t.text2} />
      </button>
    </div>
  );
};

Object.assign(window, { SectionEditor });
