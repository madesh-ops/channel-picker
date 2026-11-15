import type { GainReport, MixSettings } from "../lib/types";
import { fmtDb } from "../lib/api";
import { Meter } from "./Meter";

interface Props {
  mix: MixSettings;
  report: GainReport | null;
  outputPeaks: [number, number];
  clipLatched: boolean;
  onResetClip: () => void;
  onChange: (patch: Partial<MixSettings>) => void;
}

export function MasterSection({ mix, report, outputPeaks, clipLatched, onResetClip, onChange }: Props) {
  return (
    <section className="panel master">
      <header className="panel-head">
        <h2>Stereo output</h2>
        <span className="out-badge" title="Exports are always plain two-channel stereo">
          Stereo 2.0 · L/R
        </span>
      </header>

      <div className="master-meters">
        <Meter peak={outputPeaks[0]} label="L" horizontal />
        <Meter peak={outputPeaks[1]} label="R" horizontal />
        <button
          className={`clip-led ${clipLatched ? "lit" : ""}`}
          onClick={onResetClip}
          title={clipLatched ? "Clipping detected: click to reset" : "No clipping"}
        >
          CLIP
        </button>
      </div>

      <div className="segmented" role="radiogroup" aria-label="Gain staging">
        <button
          role="radio"
          aria-checked={mix.gainMode === "safe"}
          className={mix.gainMode === "safe" ? "active" : ""}
          onClick={() => onChange({ gainMode: "safe" })}
        >
          Safe gain
        </button>
        <button
          role="radio"
          aria-checked={mix.gainMode === "manual"}
          className={mix.gainMode === "manual" ? "active" : ""}
          onClick={() => onChange({ gainMode: "manual" })}
        >
          Manual
        </button>
      </div>
      <p className="hint">
        {mix.gainMode === "safe"
          ? "Scales the mix down so full-scale audio on every selected channel cannot clip."
          : "No automatic headroom. Watch the meters, or turn on normalization when exporting."}
      </p>

      <label className="gain">
        <span>Master</span>
        <input
          type="range"
          min={-24}
          max={12}
          step={0.5}
          value={mix.masterGainDb}
          onChange={(e) => onChange({ masterGainDb: Number(e.target.value) })}
          onDoubleClick={() => onChange({ masterGainDb: 0 })}
          aria-label="Master gain"
        />
        <output>{fmtDb(mix.masterGainDb)}</output>
      </label>

      {report && (
        <dl className="stats">
          <dt>Worst-case sum L / R</dt>
          <dd>
            {report.worstCaseLeft.toFixed(2)} / {report.worstCaseRight.toFixed(2)}
          </dd>
          <dt>Safe-gain trim</dt>
          <dd>{mix.gainMode === "safe" ? fmtDb(report.safeGainDb) : "off"}</dd>
          <dt>Total output gain</dt>
          <dd>{fmtDb(report.outputGainDb)}</dd>
          <dt>Headroom</dt>
          <dd className={report.canClip ? "warn" : "ok"}>{report.canClip ? "Can clip at full scale" : "Clip-safe"}</dd>
          <dt>Routing</dt>
          <dd>{report.routes.length ? report.routes.join(", ") : "Nothing selected (silence)"}</dd>
        </dl>
      )}
    </section>
  );
}
