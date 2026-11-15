import { toDb } from "../lib/api";

const FLOOR = -60;

/** Peak meter on a dBFS scale (−60 … 0). `peak` is linear. */
export function Meter({ peak, label, horizontal = false }: { peak: number; label?: string; horizontal?: boolean }) {
  const db = Math.max(FLOOR, Math.min(0, toDb(peak)));
  const pct = ((db - FLOOR) / -FLOOR) * 100;
  const hot = peak >= 0.891; // −1 dBFS
  const over = peak > 1;
  return (
    <div className={`meter ${horizontal ? "meter-h" : "meter-v"}`} title={label ? `${label} peak` : "peak"}>
      <div
        className={`meter-fill ${over ? "over" : hot ? "hot" : ""}`}
        style={horizontal ? { width: `${pct}%` } : { height: `${pct}%` }}
      />
      {label && <span className="meter-label">{label}</span>}
    </div>
  );
}
