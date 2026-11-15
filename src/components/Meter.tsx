import { toDb } from "../lib/api";

const FLOOR = -60;

/**
 * Peak meter on a dBFS scale (−60 … 0). The colour gradient is fixed to the
 * scale (green → yellow near −6 dB → red at 0), and a mask hides the unlit part.
 */
export function Meter({ peak, horizontal = false, label }: { peak: number; horizontal?: boolean; label?: string }) {
  const db = Math.max(FLOOR, Math.min(0, toDb(peak)));
  const lit = ((db - FLOOR) / -FLOOR) * 100;
  const unlit = `${100 - lit}%`;
  return (
    <div className={`meter ${horizontal ? "meter-h" : "meter-v"}`} role="img" aria-label={label ? `${label} level` : "Level"}>
      <div className="meter-mask" style={horizontal ? { width: unlit } : { height: unlit }} />
    </div>
  );
}
