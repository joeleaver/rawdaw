// Shared bits — color helpers + Tabler line-icon glyphs (inline SVG).

// Convert a 6-char hex to rgba() with given alpha. Used for soft tints.
function rgba(hex, a) {
  const n = parseInt(hex.replace('#',''), 16);
  return `rgba(${(n>>16)&255},${(n>>8)&255},${n&255},${a})`;
}

// Single-stroke 16px Tabler-style icons. Pass `size` to override.
const Icon = ({ name, size = 16, stroke = 'currentColor', strokeWidth = 1.6 }) => {
  const common = {
    width: size, height: size, viewBox: '0 0 24 24',
    fill: 'none', stroke, strokeWidth,
    strokeLinecap: 'round', strokeLinejoin: 'round',
    style: { flex: '0 0 auto', display: 'block' },
  };
  switch (name) {
    case 'play':       return <svg {...common}><path d="M7 4v16l13 -8z" /></svg>;
    case 'stop':       return <svg {...common}><rect x="6" y="6" width="12" height="12" rx="1" /></svg>;
    case 'rewind':     return <svg {...common}><path d="M21 5v14l-10 -7zM4 5v14" /></svg>;
    case 'record':     return <svg {...common}><circle cx="12" cy="12" r="6" /></svg>;
    case 'search':     return <svg {...common}><circle cx="10" cy="10" r="6"/><path d="M21 21l-6 -6"/></svg>;
    case 'chevron-d':  return <svg {...common}><path d="M6 9l6 6l6 -6"/></svg>;
    case 'chevron-r':  return <svg {...common}><path d="M9 6l6 6l-6 6"/></svg>;
    case 'chevron-u':  return <svg {...common}><path d="M6 15l6 -6l6 6"/></svg>;
    case 'plus':       return <svg {...common}><path d="M12 5v14M5 12h14"/></svg>;
    case 'grip':       return <svg {...common}><circle cx="9" cy="6" r="0.6" fill={stroke}/><circle cx="15" cy="6" r="0.6" fill={stroke}/><circle cx="9" cy="12" r="0.6" fill={stroke}/><circle cx="15" cy="12" r="0.6" fill={stroke}/><circle cx="9" cy="18" r="0.6" fill={stroke}/><circle cx="15" cy="18" r="0.6" fill={stroke}/></svg>;
    case 'zoom-out':   return <svg {...common}><circle cx="10" cy="10" r="6"/><path d="M7 10h6M21 21l-6 -6"/></svg>;
    case 'zoom-in':    return <svg {...common}><circle cx="10" cy="10" r="6"/><path d="M7 10h6M10 7v6M21 21l-6 -6"/></svg>;
    case 'gear':       return <svg {...common}><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06 .06a2 2 0 1 1 -2.83 2.83l-.06 -.06a1.65 1.65 0 0 0 -1.82 -.33a1.65 1.65 0 0 0 -1 1.51v.17a2 2 0 1 1 -4 0v-.09a1.65 1.65 0 0 0 -1.08 -1.51a1.65 1.65 0 0 0 -1.82 .33l-.06 .06a2 2 0 1 1 -2.83 -2.83l.06 -.06a1.65 1.65 0 0 0 .33 -1.82a1.65 1.65 0 0 0 -1.51 -1h-.17a2 2 0 1 1 0 -4h.09a1.65 1.65 0 0 0 1.51 -1.08a1.65 1.65 0 0 0 -.33 -1.82l-.06 -.06a2 2 0 1 1 2.83 -2.83l.06 .06a1.65 1.65 0 0 0 1.82 .33h0a1.65 1.65 0 0 0 1 -1.51v-.17a2 2 0 1 1 4 0v.09a1.65 1.65 0 0 0 1 1.51h0a1.65 1.65 0 0 0 1.82 -.33l.06 -.06a2 2 0 1 1 2.83 2.83l-.06 .06a1.65 1.65 0 0 0 -.33 1.82v0a1.65 1.65 0 0 0 1.51 1h.17a2 2 0 1 1 0 4h-.09a1.65 1.65 0 0 0 -1.51 1z"/></svg>;
    case 'wave':       return <svg {...common}><path d="M3 12h2l2 -7l4 14l3 -10l2 5h5"/></svg>;
    case 'pattern':    return <svg {...common}><rect x="3" y="6" width="4" height="4" rx="0.5"/><rect x="10" y="6" width="4" height="4" rx="0.5"/><rect x="17" y="6" width="4" height="4" rx="0.5"/><rect x="3" y="14" width="4" height="4" rx="0.5"/><rect x="10" y="14" width="4" height="4" rx="0.5"/><rect x="17" y="14" width="4" height="4" rx="0.5"/></svg>;
    case 'chord':      return <svg {...common}><path d="M5 4v12.5a3 3 0 1 1 -2 -2.83V7l12 -3v10.5a3 3 0 1 1 -2 -2.83V4z"/></svg>;
    case 'section':    return <svg {...common}><rect x="3" y="8" width="6" height="8" rx="1"/><rect x="11" y="8" width="4" height="8" rx="1"/><rect x="17" y="8" width="4" height="8" rx="1"/></svg>;
    case 'dot':        return <svg {...common}><circle cx="12" cy="12" r="3" fill={stroke}/></svg>;
    case 'minus':      return <svg {...common}><path d="M5 12h14"/></svg>;
    case 'eye-off':    return <svg {...common}><path d="M3 3l18 18M10.7 5.1A9.6 9.6 0 0 1 12 5c5 0 9 4 10 7a13 13 0 0 1 -2.6 3.8M6.6 6.6C4.6 7.9 3 9.9 2 12c1 3 5 7 10 7c1.4 0 2.7 -.3 3.9 -.8"/></svg>;
    default: return null;
  }
};

// Compact pill — used for tags, counts, variant chips.
const Pill = ({ children, color, dim, style }) => (
  <span style={{
    display: 'inline-flex', alignItems: 'center', gap: 4,
    height: 17, padding: '0 6px', borderRadius: 3,
    fontSize: 10.5, fontWeight: 500, letterSpacing: 0.2,
    background: color ? rgba(color, 0.16) : 'rgba(232,234,238,0.06)',
    color: dim ? 'rgba(232,234,238,0.55)' : 'rgba(232,234,238,0.88)',
    border: color ? `1px solid ${rgba(color, 0.30)}` : '1px solid rgba(232,234,238,0.10)',
    ...style,
  }}>{children}</span>
);

// Roman numeral renderer. Case is load-bearing: lowercase = minor, uppercase
// = major (the case-by-quality convention from composition-model.md). We
// MUST preserve case — no small-caps, no upper-casing. The chord-context
// treatment is a slightly heavier weight + tracked letter-spacing only.
const Roman = ({ children, size = 13, color = 'rgba(232,234,238,0.92)', weight = 600 }) => (
  <span style={{
    fontFamily: 'inherit',
    fontFeatureSettings: '"tnum" 1',
    fontWeight: weight, fontSize: size,
    letterSpacing: 0.6, color, lineHeight: 1,
  }}>{children}</span>
);

Object.assign(window, { Icon, Pill, Roman, rgba });
