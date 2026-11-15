import type { GainReport, MixSettings } from "../lib/types";
import { fmtDb } from "../lib/api";
import { Meter } from "./Meter";
import { Disclosure, SectionTitle, Segmented, Slider } from "./ui";

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
    <section className="card">
      <SectionTitle aside={<span className="badge blue" title="Exports are always plain two-channel stereo">Stereo 2.0</span>}>
        Output
      </SectionTitle>

      <div className="out-meters">
        <div className="meter-row">
          <span className="ch">L</span>
          <Meter peak={outputPeaks[0]} horizontal label="Left output" />
          <button
            className={`clip ${clipLatched ? "lit" : ""}`}
            onClick={onResetClip}
            title={clipLatched ? "Clipping detected. Click to reset." : "No clipping"}
            aria-label={clipLatched ? "Clipping detected, reset" : "No clipping"}
          >
            CLIP
          </button>
        </div>
        <div className="meter-row">
          <span className="ch">R</span>
          <Meter peak={outputPeaks[1]} horizontal label="Right output" />
          <span style={{ width: 44 }} />
        </div>
      </div>

      <Segmented
        label="Gain staging"
        value={mix.gainMode}
        onChange={(gainMode) => onChange({ gainMode })}
        options={[
          { value: "safe", label: "Safe Gain", title: "Scale down so nothing can clip" },
          { value: "manual", label: "Manual" },
        ]}
      />
      <p className="t-caption" style={{ margin: "8px 2px 12px" }}>
        {mix.gainMode === "safe"
          ? "Turns the mix down just enough that every selected channel at full volume still can't clip."
          : "No automatic headroom. Watch the meters, or normalize when exporting."}
      </p>

      <div className="chan-gain">
        <div className="row">
          <span>Master</span>
          <output>{fmtDb(mix.masterGainDb)}</output>
        </div>
        <Slider
          min={-24}
          max={12}
          step={0.5}
          value={mix.masterGainDb}
          resetTo={0}
          onChange={(masterGainDb) => onChange({ masterGainDb })}
          label="Master gain"
        />
      </div>

      {report && (
        <>
          <div className="headroom">
            {report.canClip ? <span className="warn">Can clip at full volume</span> : <span className="ok">Clip-safe</span>}
            <span>·</span>
            <span>{report.routes.length ? report.routes.join(", ") : "Nothing selected"}</span>
          </div>
          <Disclosure title="Gain details">
            <div className="list stats">
              <div className="list-row">
                <span className="k">Worst-case sum L / R</span>
                <span className="v num">
                  {report.worstCaseLeft.toFixed(2)} / {report.worstCaseRight.toFixed(2)}
                </span>
              </div>
              <div className="list-row">
                <span className="k">Safe-gain trim</span>
                <span className="v num">{mix.gainMode === "safe" ? fmtDb(report.safeGainDb) : "Off"}</span>
              </div>
              <div className="list-row">
                <span className="k">Total output gain</span>
                <span className="v num">{fmtDb(report.outputGainDb)}</span>
              </div>
            </div>
          </Disclosure>
        </>
      )}
    </section>
  );
}
