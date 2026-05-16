// MainWindow — composes the four regions into a 1600x900 artboard.

const MainWindow = ({ selectedIdx = null }) => {
  const t = window.RD.tokens;
  const [sel, setSel] = React.useState(selectedIdx);

  // Linked highlighting key: which sectionKey are we highlighting in the
  // library + arrangement when something is selected?
  const linkedSectionKey = sel != null ? window.RD.arrangement[sel].sectionKey : null;

  return (
    <div style={{
      width: 1600, height: 900,
      background: t.bg0, color: t.text0,
      fontFamily: t.fontSans,
      fontSize: 13, lineHeight: 1.4,
      display: 'flex', flexDirection: 'column',
      overflow: 'hidden',
      boxShadow: '0 0 0 1px rgba(0,0,0,0.5)',
      borderRadius: 6,
    }}>
      <TopBar />

      <div style={{ flex: 1, display: 'flex', minHeight: 0 }}>
        <Library width={260} highlightSectionKey={linkedSectionKey} />
        <Arrangement
          selectedIdx={sel}
          onSelect={(idx) => setSel(idx === sel ? null : idx)}
        />
        <Inspector width={320} selectedIdx={sel} />
      </div>

      <BottomDetailStrip />
    </div>
  );
};

const BottomDetailStrip = () => {
  const t = window.RD.tokens;
  return (
    <div style={{
      height: t.h.detail, flex: `0 0 ${t.h.detail}px`,
      background: t.bg1, borderTop: `1px solid ${t.line}`,
      display: 'flex', alignItems: 'center', gap: 8, padding: '0 12px',
    }}>
      <Icon name="chevron-u" size={13} stroke={t.text2} />
      <span style={{ fontSize: 11.5, color: t.text2 }}>No detail view open</span>
      <span style={{ flex: 1 }} />
      <span style={{ fontSize: 10.5, color: t.text3 }}>
        piano roll · pattern editor open here on drill-down
      </span>
    </div>
  );
};

Object.assign(window, { MainWindow });
