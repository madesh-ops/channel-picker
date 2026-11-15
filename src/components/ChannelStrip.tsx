import type { ChannelInfo, ChannelStrip as Strip } from "../lib/types";
import { fmtDb } from "../lib/api";
import { Meter } from "./Meter";
import { Slider, Switch } from "./ui";

interface Props {
  info: ChannelInfo;
  strip: Strip;
  heard: boolean;
  peak: number;
  area?: string;
  onChange: (patch: Partial<Strip>) => void;
}

export function ChannelStrip({ info, strip, heard, peak, area, onChange }: Props) {
  const dest =
    strip.toLeft > 0 && strip.toRight > 0 ? "L + R" : strip.toLeft > 0 ? "Left" : strip.toRight > 0 ? "Right" : "None";
  return (
    <div
      className={`chan ${heard ? "is-heard" : ""} ${strip.solo ? "is-solo" : ""}`}
      style={area ? { gridArea: area } : undefined}
    >
      <div className="chan-head">
        <div className="chan-titles">
          <span className="chan-id">{info.id}</span>
          <span className="chan-name">{info.label}</span>
        </div>
        <Switch
          checked={strip.enabled}
          onChange={(enabled) => onChange({ enabled })}
          label={`${info.label} in the mix`}
        />
      </div>

      <div className="chan-body">
        <Meter peak={peak} label={info.label} />
        <div className="chan-aside">
          <span className={`dest ${heard ? "" : "off"}`} title="Where this channel goes in the stereo mix">
            {heard ? `→ ${dest}` : "Not in mix"}
          </span>
          <button
            className={`solo ${strip.solo ? "on" : ""}`}
            aria-pressed={strip.solo}
            onClick={() => onChange({ solo: !strip.solo })}
            title="Solo: hear only soloed channels"
          >
            Solo
          </button>
        </div>
      </div>

      <div className="chan-gain">
        <div className="row">
          <span>Gain</span>
          <output>{fmtDb(strip.gainDb)}</output>
        </div>
        <Slider
          min={-60}
          max={12}
          step={0.5}
          value={strip.gainDb}
          resetTo={0}
          onChange={(gainDb) => onChange({ gainDb })}
          label={`${info.label} gain`}
        />
      </div>
    </div>
  );
}

/** Placeholder for a 5.1 position the source doesn't have (e.g. a stereo source). */
export function GhostStrip({ id, label, area }: { id: string; label: string; area?: string }) {
  return (
    <div className="chan ghost" aria-disabled style={area ? { gridArea: area } : undefined}>
      <div className="chan-titles">
        <span className="chan-id">{id}</span>
        <span className="chan-name">{label}</span>
      </div>
      <div className="ghost-note">Not in this source</div>
    </div>
  );
}
