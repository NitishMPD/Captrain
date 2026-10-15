const HEX = 'M507 469Q560 439 612 469l122 71q56 33 56 92v143q0 58-50 86l-123 72q-55 36-111 4l-122-70q-51-28-51-85V633q0-59 52-89Z';
const C = 'M574 721q0-20 21-31l118-69q25-14 39 9 6 20 0 43-4 15-19 26l-62 36q-23 13-23 34-1 21 16 28 10 3 20-3l27-15q25-17 39 4 14 26-6 47l-13 10-122 61q-35 18-35-16Z';

// The hexagon settles in from a soft blur, the C traces its outline and fills, and a light sweep crosses the logo.
export function Splash() {
  return (
    <div className="splash">
      <svg className="splash-logo" viewBox="280 422 560 560" fill="none">
        <clipPath id="splash-clip"><path d={HEX} /></clipPath>
        <linearGradient id="splash-shine" x1="0" x2="1" y1="0" y2="0">
          <stop offset="0" stopColor="#fff" stopOpacity="0" />
          <stop offset="0.5" stopColor="#fff" stopOpacity="0.35" />
          <stop offset="1" stopColor="#fff" stopOpacity="0" />
        </linearGradient>
        <path className="splash-hex" d={HEX} fill="#3D5F94" />
        <path className="splash-c" d={C} pathLength={1} fill="#FCFAF7" stroke="#FCFAF7" strokeWidth="6" strokeLinejoin="round" />
        <g clipPath="url(#splash-clip)">
          <rect className="splash-sweep" x="180" y="380" width="160" height="660" fill="url(#splash-shine)" />
        </g>
      </svg>
    </div>
  );
}
