// Top bar — project identity (left) · transport (center) · tempo/zoom (right).

const TopBar = () => {
  const t = window.RD.tokens;
  const p = window.RD.project;
  return (
    <div style={{
      height: t.h.topbar, flex: `0 0 ${t.h.topbar}px`,
      background: t.bg1, borderBottom: `1px solid ${t.line}`,
      display: 'grid', gridTemplateColumns: '1fr auto 1fr',
      alignItems: 'center', padding: '0 12px', gap: 12,
    }}>
      {/* Left: project identity */}
      <div style={{ display: 'flex', flexDirection: 'column', justifyContent: 'center', gap: 1, minWidth: 0 }}>
        <div style={{
          fontSize: 13, fontWeight: 600, color: t.text0, letterSpacing: -0.1,
          overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap',
        }}>{p.name}</div>
        <div style={{ fontSize: 11, color: t.text2, fontFeatureSettings: '"tnum" 1' }}>
          {p.key} · {p.timeSig}
        </div>
      </div>

      {/* Center: transport */}
      <div style={{ display: 'flex', alignItems: 'center', gap: 4 }}>
        <TransportBtn icon="rewind"  title="Return to zero" />
        <TransportBtn icon="play"    title="Play"  primary />
        <TransportBtn icon="stop"    title="Stop" />
        <TransportBtn icon="record"  title="Record (disabled — MIDI v1 has no audio recording)" disabled dotColor={t.danger} />
        <div style={{ width: 1, height: 18, background: t.line, margin: '0 6px' }} />
        <div style={{
          display: 'flex', alignItems: 'baseline', gap: 6,
          padding: '3px 9px', borderRadius: 4, background: t.bg0,
          border: `1px solid ${t.line}`,
          fontSize: 12, color: t.text0,
          fontFeatureSettings: '"tnum" 1', fontVariantNumeric: 'tabular-nums',
        }}>
          <span style={{ color: t.text2, fontSize: 10, textTransform: 'uppercase', letterSpacing: 0.6 }}>Bar</span>
          <span style={{ minWidth: 14, textAlign: 'right' }}>{p.playhead.bar}</span>
          <span style={{ color: t.text3 }}>·</span>
          <span style={{ color: t.text2, fontSize: 10, textTransform: 'uppercase', letterSpacing: 0.6 }}>Beat</span>
          <span style={{ minWidth: 8 }}>{p.playhead.beat}</span>
        </div>
      </div>

      {/* Right: tempo · zoom · gear */}
      <div style={{ display: 'flex', alignItems: 'center', gap: 6, justifyContent: 'flex-end' }}>
        <div style={{
          display: 'flex', alignItems: 'baseline', gap: 6,
          padding: '3px 9px', borderRadius: 4, background: t.bg0,
          border: `1px solid ${t.line}`, fontSize: 12,
          color: t.text0, fontFeatureSettings: '"tnum" 1',
        }}>
          <span style={{ minWidth: 22, textAlign: 'right', fontVariantNumeric: 'tabular-nums' }}>{p.tempo}.00</span>
          <span style={{ color: t.text2, fontSize: 10, letterSpacing: 0.6, textTransform: 'uppercase' }}>BPM</span>
        </div>
        <div style={{ display: 'flex', gap: 1, marginLeft: 4 }}>
          <IconBtn icon="zoom-out" title="Zoom out" />
          <IconBtn icon="zoom-in"  title="Zoom in"  />
        </div>
        <IconBtn icon="gear" title="Project settings" />
      </div>
    </div>
  );
};

const TransportBtn = ({ icon, title, primary, disabled, dotColor }) => {
  const t = window.RD.tokens;
  const color = disabled ? t.text3
              : primary  ? t.ok
              : t.text0;
  return (
    <button title={title} disabled={disabled} style={{
      height: 28, width: 28, borderRadius: 4,
      background: 'transparent',
      border: '1px solid transparent',
      color, cursor: disabled ? 'not-allowed' : 'pointer',
      display: 'inline-flex', alignItems: 'center', justifyContent: 'center',
      padding: 0,
    }}>
      {icon === 'record'
        ? <span style={{
            width: 9, height: 9, borderRadius: '50%',
            background: dotColor, opacity: disabled ? 0.45 : 1,
          }} />
        : <Icon name={icon} size={icon === 'play' ? 14 : 13} stroke={color} strokeWidth={icon === 'play' ? 1.8 : 1.6} />}
    </button>
  );
};

const IconBtn = ({ icon, title }) => {
  const t = window.RD.tokens;
  return (
    <button title={title} style={{
      height: 26, width: 26, borderRadius: 4,
      background: 'transparent',
      border: '1px solid transparent',
      color: t.text1, cursor: 'pointer',
      display: 'inline-flex', alignItems: 'center', justifyContent: 'center',
      padding: 0,
    }}><Icon name={icon} size={14} stroke={t.text1} /></button>
  );
};

Object.assign(window, { TopBar });
