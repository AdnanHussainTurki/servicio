import { useId } from "react";

const W = 240;
const H = 56;
const PAD = 3;

/**
 * A small instrument-style line chart. Renders an SVG polyline scaled to the
 * data's min/max across a fixed viewBox, with a soft area fill beneath and a
 * glowing marker at the latest sample. Degrades gracefully for empty/one-point
 * series (flat baseline).
 */
export function Sparkline({ data, stroke = "#f97316" }: { data: number[]; stroke?: string }) {
  const gid = useId();
  const n = data.length;

  // vertical scale — guard against a flat series (min === max) to avoid div/0
  const min = n ? Math.min(...data) : 0;
  const max = n ? Math.max(...data) : 1;
  const span = max - min || 1;

  const innerW = W - PAD * 2;
  const innerH = H - PAD * 2;

  const x = (i: number) => (n <= 1 ? W / 2 : PAD + (i / (n - 1)) * innerW);
  const y = (v: number) => PAD + innerH - ((v - min) / span) * innerH;

  // a single point becomes a short flat segment so the line is visible
  const pts = n === 0 ? [] : n === 1 ? [[PAD, y(data[0])], [W - PAD, y(data[0])]] : data.map((v, i) => [x(i), y(v)]);

  const line = pts.map(([px, py]) => `${px.toFixed(1)},${py.toFixed(1)}`).join(" ");
  const area = pts.length
    ? `${pts[0][0].toFixed(1)},${(H - PAD).toFixed(1)} ${line} ${pts[pts.length - 1][0].toFixed(1)},${(H - PAD).toFixed(1)}`
    : "";

  const last = pts.length ? pts[pts.length - 1] : null;

  return (
    <svg
      viewBox={`0 0 ${W} ${H}`}
      preserveAspectRatio="none"
      className="h-14 w-full"
      role="img"
      aria-hidden
    >
      <defs>
        <linearGradient id={`fill-${gid}`} x1="0" y1="0" x2="0" y2="1">
          <stop offset="0%" stopColor={stroke} stopOpacity="0.22" />
          <stop offset="100%" stopColor={stroke} stopOpacity="0" />
        </linearGradient>
      </defs>

      {/* baseline / mid gridlines — the instrument grid */}
      <line x1="0" y1={H - PAD} x2={W} y2={H - PAD} stroke="currentColor" strokeOpacity="0.10" strokeWidth="1" />
      <line
        x1="0"
        y1={H / 2}
        x2={W}
        y2={H / 2}
        stroke="currentColor"
        strokeOpacity="0.06"
        strokeWidth="1"
        strokeDasharray="2 4"
      />

      {area && <polygon points={area} fill={`url(#fill-${gid})`} stroke="none" />}
      {line && (
        <polyline
          points={line}
          fill="none"
          stroke={stroke}
          strokeWidth="1.75"
          strokeLinejoin="round"
          strokeLinecap="round"
          vectorEffect="non-scaling-stroke"
        />
      )}
      {last && (
        <>
          <circle cx={last[0]} cy={last[1]} r="4.5" fill={stroke} fillOpacity="0.18" />
          <circle cx={last[0]} cy={last[1]} r="2" fill={stroke} />
        </>
      )}
    </svg>
  );
}
