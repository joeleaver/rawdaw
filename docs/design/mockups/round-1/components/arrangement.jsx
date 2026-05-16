// Arrangement — the centerpiece. Ruler · chord-loop ribbon · section lane.
// All content geometry rendered as SVG-friendly rectangles & lines so the
// translation to Rinch/Vello is direct.

const Arrangement = ({ selectedIdx = null, onSelect = () => {} }) => {
  const t = window.RD.tokens;
  const RD = window.RD;
  const totalBars = RD.totalBars;

  // Layout: a left gutter for track-label spacer (matches inspector/library
  // gutters) then the bar grid. Keep right padding small.
  const containerRef = React.useRef(null);
  const [w, setW] = React.useState(1020);
  React.useEffect(() => {
    const el = containerRef.current;
    if (!el) return;
    const ro = new ResizeObserver(() => setW(el.clientWidth));
    ro.observe(el);
    setW(el.clientWidth);
    return () => ro.disconnect();
  }, []);

  const leftGutter = 0;
  const rightPad = 12;
  const gridW = Math.max(200, w - leftGutter - rightPad);
  const barW = gridW / totalBars;

  // Playhead position in pixels (project.playhead).
  const playheadBars = (RD.project.playhead.bar - 1) + (RD.project.playhead.beat - 1) / 4;
  const playheadX = leftGutter + playheadBars * barW;

  // Selected block + its section id (for linked highlight).
  const selectedBlock = selectedIdx != null ? RD.arrangement[selectedIdx] : null;
  const selectedSectionKey = selectedBlock?.sectionKey ?? null;

  return (
    <section
      ref={containerRef}
      style={{
        flex: 1, minWidth: 0, display: 'flex', flexDirection: 'column',
        background: t.bg0,
      }}>

      {/* ── Timeline ruler ─────────────────────────────────────────────── */}
      <Ruler
        totalBars={totalBars} barW={barW} leftGutter={leftGutter}
        height={t.h.ruler} playheadX={playheadX} />

      {/* ── Chord-loop ribbon ──────────────────────────────────────────── */}
      <ChordRibbon
        height={t.h.ribbon} barW={barW} leftGutter={leftGutter}
        selectedIdx={selectedIdx} selectedSectionKey={selectedSectionKey} />

      {/* ── Section lane ───────────────────────────────────────────────── */}
      <SectionLane
        height={t.h.lane} barW={barW} leftGutter={leftGutter}
        totalBars={totalBars}
        selectedIdx={selectedIdx} selectedSectionKey={selectedSectionKey}
        onSelect={onSelect} playheadX={playheadX} />

      {/* ── Empty future-multi-lane area ───────────────────────────────── */}
      <div style={{
        flex: 1, minHeight: 0,
        background: t.bg0,
        backgroundImage: `linear-gradient(${t.lineSoft} 1px, transparent 1px)`,
        backgroundSize: `100% 28px`,
        borderTop: `1px solid ${t.line}`,
        position: 'relative',
      }}>
        {/* faint per-bar vertical guides */}
        <svg width="100%" height="100%" style={{ position: 'absolute', inset: 0, pointerEvents: 'none' }}>
          {Array.from({ length: totalBars + 1 }, (_, i) => {
            const x = leftGutter + i * barW;
            const major = i % 4 === 0;
            return <line key={i} x1={x} y1={0} x2={x} y2="100%"
              stroke={major ? t.line : t.lineSoft} strokeWidth={1} />;
          })}
          <line x1={playheadX} y1={0} x2={playheadX} y2="100%"
            stroke={t.accent} strokeWidth={1} opacity={0.7} />
        </svg>
        <div style={{
          position: 'absolute', left: 14, bottom: 10,
          fontSize: 10.5, color: t.text3, letterSpacing: 0.4,
          fontStyle: 'italic',
        }}>
          additional lanes (sub-tracks) — round 2
        </div>
      </div>
    </section>
  );
};

// ─── Timeline ruler ───────────────────────────────────────────────────────
const Ruler = ({ totalBars, barW, leftGutter, height, playheadX }) => {
  const t = window.RD.tokens;
  return (
    <div style={{
      height, flex: `0 0 ${height}px`,
      background: t.bg1, borderBottom: `1px solid ${t.line}`,
      position: 'relative',
    }}>
      <svg width="100%" height={height} style={{ display: 'block' }}>
        {/* Per-bar tick marks */}
        {Array.from({ length: totalBars + 1 }, (_, i) => {
          const x = leftGutter + i * barW;
          const major = i % 4 === 0;
          return (
            <line key={i} x1={x} y1={major ? height - 8 : height - 4}
                  x2={x} y2={height}
                  stroke={major ? t.text2 : t.text3} strokeWidth={1} />
          );
        })}
        {/* Per-beat minor ticks (every barW/4) */}
        {Array.from({ length: totalBars * 4 }, (_, i) => {
          if (i % 4 === 0) return null;
          const x = leftGutter + (i / 4) * barW;
          return (
            <line key={`b${i}`} x1={x} y1={height - 2} x2={x} y2={height}
                  stroke={t.text3} strokeWidth={1} opacity={0.5} />
          );
        })}
        {/* Bar number labels every 4 bars */}
        {Array.from({ length: Math.ceil(totalBars / 4) }, (_, i) => {
          const bar = i * 4 + 1;
          const x = leftGutter + (bar - 1) * barW + 4;
          return (
            <text key={`l${i}`} x={x} y={12}
              fontSize={10} fill={t.text2}
              fontFamily={t.fontNum}
              style={{ fontFeatureSettings: '"tnum" 1', fontVariantNumeric: 'tabular-nums' }}>
              {bar}
            </text>
          );
        })}
        {/* Playhead */}
        <line x1={playheadX} y1={0} x2={playheadX} y2={height}
          stroke={t.accent} strokeWidth={1} />
        <polygon
          points={`${playheadX - 4},0 ${playheadX + 4},0 ${playheadX},5`}
          fill={t.accent} />
      </svg>
    </div>
  );
};

// ─── Chord-loop ribbon ────────────────────────────────────────────────────
// Sits between the ruler and the section lane. For each arrangement block,
// we render its scheduled chord loop (or loops) tiled across the block's
// bar range. Roman primary; absolute on a smaller line beneath (always
// visible at this density). Each cell carries a 2px top stripe in the
// chord loop's identity color.
const ChordRibbon = ({ height, barW, leftGutter, selectedIdx, selectedSectionKey }) => {
  const t = window.RD.tokens;
  const RD = window.RD;

  // Build flat list of cells: one per chord event per loop-iteration per block.
  const cells = [];
  RD.arrangement.forEach((block, idx) => {
    const section = RD.sections[block.sectionKey];
    // For round 1: a section has exactly one chord loop covering its range.
    const loopName = section.chordLoops[0];
    const loop = RD.chordLoops[loopName];

    // Tile the loop across the block.
    let bar = block.startBar;
    while (bar < block.startBar + block.bars) {
      loop.events.forEach((ev, ei) => {
        if (bar + 1 > block.startBar + block.bars) return;
        cells.push({
          bar, len: 1, // each chord = 1 bar in this fixture
          roman: ev.roman, quality: ev.quality, absolute: ev.absolute,
          color: loop.color,
          blockIdx: idx, sectionKey: block.sectionKey,
          loopName, isFirstOfLoop: ei === 0,
        });
        bar += 1;
      });
    }
  });

  return (
    <div style={{
      height, flex: `0 0 ${height}px`,
      background: t.bg1, borderBottom: `1px solid ${t.line}`,
      position: 'relative',
    }}>
      {cells.map((c, i) => {
        const x = leftGutter + c.bar * barW;
        const cellW = c.len * barW;
        const isSelectedBlock = selectedIdx === c.blockIdx;
        const isLinked = selectedSectionKey != null && c.sectionKey === selectedSectionKey;
        const emphasize = isSelectedBlock || isLinked;

        return (
          <div key={i} style={{
            position: 'absolute', left: x, top: 0,
            width: cellW, height,
            borderRight: `1px solid ${t.lineSoft}`,
            display: 'flex', flexDirection: 'column',
            justifyContent: 'center', alignItems: 'center',
            paddingTop: 3, gap: 1,
            background: emphasize ? rgba(c.color, isSelectedBlock ? 0.10 : 0.05) : 'transparent',
            cursor: 'pointer',
          }}>
            {/* Loop identity stripe at the top */}
            {c.isFirstOfLoop && (
              <div style={{
                position: 'absolute', left: 0, top: 0,
                width: 2, height: '100%',
                background: c.color, opacity: emphasize ? 1 : 0.7,
              }} />
            )}
            <Roman size={13.5} weight={600}
              color={emphasize ? t.text0 : 'rgba(232,234,238,0.82)'}>
              {c.roman}{c.quality}
            </Roman>
            <div style={{
              fontSize: 9.5, color: emphasize ? t.text1 : t.text2,
              fontFeatureSettings: '"tnum" 1',
              letterSpacing: 0.2, lineHeight: 1,
            }}>{c.absolute}</div>
          </div>
        );
      })}
    </div>
  );
};

// ─── Section lane ────────────────────────────────────────────────────────
const SectionLane = ({
  height, barW, leftGutter, totalBars,
  selectedIdx, selectedSectionKey, onSelect, playheadX,
}) => {
  const t = window.RD.tokens;
  const RD = window.RD;

  return (
    <div style={{
      height, flex: `0 0 ${height}px`,
      background: t.bg0, position: 'relative',
      borderBottom: `1px solid ${t.line}`,
    }}>
      {/* Background per-bar guides */}
      <svg width="100%" height={height}
        style={{ position: 'absolute', inset: 0, pointerEvents: 'none' }}>
        {Array.from({ length: totalBars + 1 }, (_, i) => {
          const x = leftGutter + i * barW;
          const major = i % 4 === 0;
          return <line key={i} x1={x} y1={0} x2={x} y2={height}
            stroke={major ? t.line : t.lineSoft} strokeWidth={1} />;
        })}
      </svg>

      {/* Section blocks */}
      {RD.arrangement.map((block, idx) => {
        const section = RD.sections[block.sectionKey];
        const isSelected = selectedIdx === idx;
        const isLinked = !isSelected && selectedSectionKey != null && block.sectionKey === selectedSectionKey;
        const showVariant = block.variant !== section.defaultVariant;
        return (
          <SectionBlock
            key={idx}
            x={leftGutter + block.startBar * barW + 1}
            width={block.bars * barW - 2}
            height={height - 12}
            top={6}
            section={section}
            block={block}
            barW={barW}
            isSelected={isSelected}
            isLinked={isLinked}
            showVariant={showVariant}
            onClick={() => onSelect(idx)}
          />
        );
      })}

      {/* Playhead */}
      <div style={{
        position: 'absolute', left: playheadX - 0.5, top: 0, bottom: 0,
        width: 1, background: t.accent, opacity: 0.85, pointerEvents: 'none',
      }} />
    </div>
  );
};

// ─── Section block (the most important visual primitive) ─────────────────
const SectionBlock = ({
  x, width, height, top,
  section, block, barW,
  isSelected, isLinked, showVariant, onClick,
}) => {
  const t = window.RD.tokens;
  const color = section.color;

  // Internal bar boundaries (subtle vertical lines inside the block).
  const internalBars = block.bars - 1;

  return (
    <div
      onClick={onClick}
      style={{
        position: 'absolute', left: x, top, width, height,
        background: rgba(color, isSelected ? 0.20 : isLinked ? 0.14 : 0.10),
        border: isSelected
          ? `1px solid ${color}`
          : isLinked
          ? `1px solid ${rgba(color, 0.55)}`
          : `1px solid ${rgba(color, 0.28)}`,
        borderLeft: `3px solid ${color}`,
        borderRadius: 3,
        cursor: 'pointer',
        boxShadow: isSelected ? `0 0 0 1px ${rgba(color, 0.30)}` : 'none',
        overflow: 'hidden',
      }}
    >
      {/* Internal bar boundary lines */}
      <svg width={width} height={height}
        style={{ position: 'absolute', inset: 0, pointerEvents: 'none' }}>
        {Array.from({ length: internalBars }, (_, i) => {
          const lx = (i + 1) * barW - 0;
          return <line key={i} x1={lx} y1={6} x2={lx} y2={height - 6}
            stroke={rgba(color, 0.22)} strokeWidth={1} />;
        })}
      </svg>

      {/* Name (top-left) */}
      <div style={{
        position: 'absolute', left: 8, top: 6,
        fontSize: 12.5, fontWeight: 600,
        color: t.text0, letterSpacing: -0.1,
      }}>{section.name}</div>

      {/* Variant chip (top-right) — only if non-default */}
      {showVariant && (
        <div style={{
          position: 'absolute', right: 6, top: 5,
          display: 'inline-flex', alignItems: 'center',
          height: 16, padding: '0 5px', borderRadius: 2,
          fontSize: 10, fontWeight: 500, letterSpacing: 0.2,
          background: rgba(color, 0.32),
          color: 'rgba(232,234,238,0.92)',
          border: `1px solid ${rgba(color, 0.55)}`,
        }}>
          <span style={{ opacity: 0.7, marginRight: 4 }}>variant:</span>
          {block.variant}
        </div>
      )}

      {/* Length readout (bottom-right) */}
      <div style={{
        position: 'absolute', right: 6, bottom: 4,
        fontSize: 10, color: 'rgba(232,234,238,0.62)',
        fontFeatureSettings: '"tnum" 1', fontVariantNumeric: 'tabular-nums',
        letterSpacing: 0.2,
      }}>
        {block.bars} {block.bars === 1 ? 'bar' : 'bars'}
      </div>
    </div>
  );
};

Object.assign(window, { Arrangement });
