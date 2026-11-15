import { toDb } from "../lib/api";

const FLOOR = -60;

/**
 * Peak meter on a dBFS scale (−60 … 0). The colour gradient is fixed to the
 * scale (green → yellow near −6 dB → red at 0); the fill is clipped to the
 * current level, so the unlit part shows only the plain track.
 */
export function Meter({ peak, horizontal = false, label }: { peak: number; horizontal?: boolean; label?: string }) {
  const db = Math.max(FLOOR, Math.min(0, toDb(peak)));
  const unlit = `${100 - ((db - FLOOR) / -FLOOR) * 100}%`;
  return (
    <div className={`meter ${horizontal ? "meter-h" : "meter-v"}`} role="img" aria-label={label ? `${label} level` : "Level"}>
      <div className="meter-fill" style={{ clipPath: horizontal ? `inset(0 ${unlit} 0 0)` : `inset(${unlit} 0 0 0)` }} />
    </div>
  );
}
