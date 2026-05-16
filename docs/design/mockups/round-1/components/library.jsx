// Library panel (left) — Patterns, Chord Loops, Sections. Always visible,
// drag-droppable. Each row carries a color swatch (identity color).

const LibraryRow = ({ color, name, meta, expandable, expanded, indent, highlighted, hoverable = true }) => {
  const t = window.RD.tokens;
  return (
    <div style={{
      display: 'flex', alignItems: 'center', gap: 8,
      padding: `4px 8px 4px ${8 + (indent ? 14 : 0)}px`,
      background: highlighted ? rgba(color, 0.10) : 'transparent',
      borderLeft: highlighted
        ? `2px solid ${color}`
        : '2px solid transparent',
      cursor: 'grab',
      minHeight: 28,
    }}>
      {/* swatch (or chevron for variant rows) */}
      {expandable
        ? <Icon name={expanded ? 'chevron-d' : 'chevron-r'} size={12} stroke={t.text2} />
        : <span style={{
            width: 10, height: 10, borderRadius: 2, flex: '0 0 auto',
            background: color, border: `1px solid ${rgba(color, 0.6)}`,
          }} />}

      <div style={{ display: 'flex', flexDirection: 'column', minWidth: 0, gap: 0, flex: 1 }}>
        <div style={{
          fontSize: 12.5, color: t.text0, fontWeight: 500,
          overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap',
          lineHeight: 1.25,
        }}>{name}</div>
        {meta && (
          <div style={{ fontSize: 10.5, color: t.text2, lineHeight: 1.25,
                        fontFeatureSettings: '"tnum" 1' }}>{meta}</div>
        )}
      </div>
    </div>
  );
};

const Group = ({ title, count, icon, children, defaultOpen = true }) => {
  const t = window.RD.tokens;
  const [open, setOpen] = React.useState(defaultOpen);
  return (
    <div style={{ borderBottom: `1px solid ${t.line}` }}>
      <button
        onClick={() => setOpen(o => !o)}
        style={{
          display: 'flex', alignItems: 'center', gap: 6,
          width: '100%', padding: '8px 10px',
          background: 'transparent', border: 0,
          color: t.text1, cursor: 'pointer',
          textAlign: 'left',
        }}>
        <Icon name={open ? 'chevron-d' : 'chevron-r'} size={12} stroke={t.text2} />
        <Icon name={icon} size={13} stroke={t.text1} />
        <span style={{
          fontSize: 11, letterSpacing: 0.6, textTransform: 'uppercase',
          fontWeight: 600, color: t.text1, flex: 1,
        }}>{title}</span>
        <span style={{ fontSize: 11, color: t.text3, fontFeatureSettings: '"tnum" 1' }}>{count}</span>
      </button>
      {open && (
        <div style={{ paddingBottom: 4 }}>
          {children}
          <button style={{
            display: 'flex', alignItems: 'center', gap: 6,
            margin: '4px 10px 8px', padding: '4px 6px',
            background: 'transparent', border: 0,
            color: t.text2, cursor: 'pointer', borderRadius: 3,
            fontSize: 11,
          }}>
            <Icon name="plus" size={12} stroke={t.text2} />
            <span>new {title.toLowerCase().replace(/s$/, '')}</span>
          </button>
        </div>
      )}
    </div>
  );
};

const Library = ({ width = 260, highlightSectionKey = null }) => {
  const t = window.RD.tokens;
  const RD = window.RD;
  const patternList   = Object.values(RD.patterns);
  const chordLoopList = Object.values(RD.chordLoops);
  const sectionList   = Object.values(RD.sections);

  return (
    <aside style={{
      width, flex: `0 0 ${width}px`,
      background: t.bg1, borderRight: `1px solid ${t.line}`,
      display: 'flex', flexDirection: 'column', minHeight: 0,
    }}>
      {/* Search */}
      <div style={{ padding: 8, borderBottom: `1px solid ${t.line}` }}>
        <div style={{
          display: 'flex', alignItems: 'center', gap: 6,
          padding: '5px 8px', borderRadius: 4,
          background: t.bg0, border: `1px solid ${t.line}`,
        }}>
          <Icon name="search" size={12} stroke={t.text2} />
          <span style={{ fontSize: 12, color: t.text3, flex: 1 }}>Search library</span>
          <span style={{
            fontSize: 10, color: t.text3,
            padding: '1px 4px', border: `1px solid ${t.line}`, borderRadius: 3,
            fontFamily: 'ui-monospace, SFMono-Regular, monospace',
          }}>⌘K</span>
        </div>
      </div>

      <div style={{ flex: 1, overflowY: 'auto', minHeight: 0 }}>
        <Group title="Patterns"    count={patternList.length}   icon="pattern">
          {patternList.map(p => (
            <LibraryRow key={p.id} color={p.color} name={p.name} meta={p.meta} />
          ))}
        </Group>

        <Group title="Chord Loops" count={chordLoopList.length} icon="chord">
          {chordLoopList.map(cl => (
            <LibraryRow key={cl.id} color={cl.color} name={cl.name}
              meta={`${cl.lengthBars} bars · ${cl.events.map(e => e.roman).join(' ')}`} />
          ))}
        </Group>

        <Group title="Sections"    count={sectionList.length}   icon="section">
          {sectionList.map(s => {
            const hl = highlightSectionKey === s.name;
            // Per ui-principles.md §4 (Variants are tabs, not tree nodes):
            // the library lists sections only. Variants live in the section
            // editor / inspector tab strip — never as tree children here.
            // Variant count is surfaced on the meta line.
            return (
              <LibraryRow
                key={s.id}
                color={s.color} name={s.name}
                meta={`${s.baseDurationBars} bars${s.variants.length > 1 ? ` · ${s.variants.length} variants` : ''}`}
                highlighted={hl}
              />
            );
          })}
        </Group>
      </div>
    </aside>
  );
};

Object.assign(window, { Library });
