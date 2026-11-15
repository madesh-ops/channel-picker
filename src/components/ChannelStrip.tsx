import type { ChannelInfo, ChannelStrip as Strip } from "../lib/types";
import { fmtDb } from "../lib/api";
import { Meter } from "./Meter";

interface Props {
  info: ChannelInfo;
  strip: Strip;
  heard: boolean;
  peak: number;
  onChange: (patch: Partial<Strip>) => void;
}

export function ChannelStrip({ info, strip, heard, peak, onChange }: Props) {
  const dest =
    strip.toLeft > 0 && strip.toRight > 0 ? "L+R" : strip.toLeft > 0 ? "L" : strip.toRight > 0 ? "R" : "—";
  return (
    <div className={`strip ${heard ? "heard" : "silent"}`}>
      <div className="strip-head">
        <span className="chan-id">{info.id}</span>
        <span className="chan-label">{info.label}</span>
      </div>

      <button
        className={`toggle ${strip.enabled ? "on" : "off"}`}
        aria-pressed={strip.enabled}
        onClick={() => onChange({ enabled: !strip.enabled })}
        title={strip.enabled ? "Channel selected (click to mute)" : "Channel muted (click to select)"}
      >
        {strip.enabled ? "Enabled" : "Muted"}
      </button>

      <div className="strip-body">
        <Meter peak={peak} />
        <div className="strip-controls">
          <button
            className={`solo ${strip.solo ? "active" : ""}`}
            aria-pressed={strip.solo}
            onClick={() => onChange({ solo: !strip.solo })}
            title="Solo: hear only soloed channels"
          >
            S
          </button>
          <div className="dest" title="Stereo destination">
            → {heard ? dest : "—"}
          </div>
        </div>
      </div>

      <label className="gain">
        <span>Gain</span>
        <input
          type="range"
          min={-60}
          max={12}
          step={0.5}
          value={strip.gainDb}
          onChange={(e) => onChange({ gainDb: Number(e.target.value) })}
          onDoubleClick={() => onChange({ gainDb: 0 })}
          aria-label={`${info.label} gain`}
        />
        <output>{fmtDb(strip.gainDb)}</output>
      </label>
    </div>
  );
}

/** Placeholder for a 5.1 channel the source does not have (e.g. stereo sources). */
export function GhostStrip({ id, label }: { id: string; label: string }) {
  return (
    <div className="strip ghost" aria-disabled>
      <div className="strip-head">
        <span className="chan-id">{id}</span>
        <span className="chan-label">{label}</span>
      </div>
      <div className="ghost-note">Not in source</div>
    </div>
  );
}
