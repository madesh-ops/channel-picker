import { useState } from "react";
import type { ExportProgress, ExportResult, OutputFormat } from "../lib/types";
import { fmtDb } from "../lib/api";

export interface ExportOptions {
  format: OutputFormat;
  sampleRate: number | null;
  normalize: boolean;
  normalizeDbfs: number;
  align: boolean;
}

const FORMATS: { key: string; label: string; format: OutputFormat }[] = [
  { key: "wav24", label: "WAV · 24-bit PCM", format: { kind: "wav", bits: 24 } },
  { key: "wav16", label: "WAV · 16-bit PCM (dithered)", format: { kind: "wav", bits: 16 } },
  { key: "wav32", label: "WAV · 32-bit float", format: { kind: "wav", bits: 32 } },
  { key: "flac24", label: "FLAC · 24-bit lossless", format: { kind: "flac", bits: 24 } },
  { key: "flac16", label: "FLAC · 16-bit lossless", format: { kind: "flac", bits: 16 } },
  { key: "mp3cbr", label: "MP3 · 320 kbps CBR", format: { kind: "mp3", vbr: false } },
  { key: "mp3vbr", label: "MP3 · V0 VBR", format: { kind: "mp3", vbr: true } },
  { key: "aac256", label: "AAC · 256 kbps (.m4a)", format: { kind: "aac", kbps: 256 } },
  { key: "aac192", label: "AAC · 192 kbps (.m4a)", format: { kind: "aac", kbps: 192 } },
];

interface Props {
  opts: ExportOptions;
  setOpts: (o: ExportOptions) => void;
  sourceRate: number;
  hasVideo: boolean;
  startOffset: number;
  disabled: boolean;
  busy: boolean;
  progress: ExportProgress | null;
  result: ExportResult | null;
  error: string | null;
  onExport: () => void;
  onCancel: () => void;
}

export function ExportPanel(p: Props) {
  const key = FORMATS.find((f) => JSON.stringify(f.format) === JSON.stringify(p.opts.format))?.key ?? "wav24";
  const [showDetails, setShowDetails] = useState(false);
  const pct = p.progress ? Math.round(p.progress.fraction * 100) : 0;
  return (
    <section className="panel export">
      <header className="panel-head">
        <h2>Export stereo file</h2>
      </header>

      <label className="field">
        <span>Format</span>
        <select
          value={key}
          disabled={p.busy}
          onChange={(e) => p.setOpts({ ...p.opts, format: FORMATS.find((f) => f.key === e.target.value)!.format })}
        >
          {FORMATS.map((f) => (
            <option key={f.key} value={f.key}>
              {f.label}
            </option>
          ))}
        </select>
      </label>

      <label className="field">
        <span>Sample rate</span>
        <select
          value={p.opts.sampleRate ?? ""}
          disabled={p.busy}
          onChange={(e) => p.setOpts({ ...p.opts, sampleRate: e.target.value ? Number(e.target.value) : null })}
        >
          <option value="">Same as source ({(p.sourceRate / 1000).toFixed(1)} kHz)</option>
          <option value="44100">44.1 kHz</option>
          <option value="48000">48 kHz</option>
          <option value="96000">96 kHz</option>
        </select>
      </label>

      <label className="check">
        <input
          type="checkbox"
          checked={p.opts.normalize}
          disabled={p.busy}
          onChange={(e) => p.setOpts({ ...p.opts, normalize: e.target.checked })}
        />
        <span>Peak-normalize to</span>
        <input
          className="small-num"
          type="number"
          step={0.5}
          max={0}
          min={-24}
          value={p.opts.normalizeDbfs}
          disabled={!p.opts.normalize || p.busy}
          onChange={(e) => p.setOpts({ ...p.opts, normalizeDbfs: Math.min(0, Number(e.target.value)) })}
          aria-label="Normalization target in dBFS"
        />
        <span>dBFS</span>
      </label>

      <label className="check check-long">
        <input
          type="checkbox"
          checked={p.opts.align}
          disabled={p.busy}
          onChange={(e) => p.setOpts({ ...p.opts, align: e.target.checked })}
        />
        <span>
          Keep video sync
          {p.startOffset > 0.0005
            ? ` (adds ${(p.startOffset * 1000).toFixed(0)} ms of lead-in silence)`
            : p.hasVideo
              ? " (no offset needed for this track)"
              : ""}
        </span>
      </label>

      {p.busy ? (
        <div className="progress-wrap">
          <div className="progress" role="progressbar" aria-valuenow={pct} aria-valuemin={0} aria-valuemax={100}>
            <div style={{ width: `${pct}%` }} />
          </div>
          <div className="progress-row">
            <span>
              {p.progress?.phase === "analyzing" ? "Measuring peak level" : "Rendering"} · {pct}%
            </span>
            <button className="ghost-btn" onClick={p.onCancel}>
              Cancel
            </button>
          </div>
        </div>
      ) : (
        <button className="primary-btn wide" disabled={p.disabled} onClick={p.onExport}>
          Export…
        </button>
      )}

      {p.error && <p className="error">{p.error}</p>}

      {p.result && !p.busy && (
        <div className={`result ${p.result.verification.ok ? "ok" : "warn"}`}>
          <div className="result-title">
            {p.result.verification.ok ? "Exported and verified" : "Exported with warnings"} · {p.result.formatLabel}
          </div>
          <div className="result-path" title={p.result.output}>
            {p.result.output}
          </div>
          <ul className="result-facts">
            <li>
              {p.result.verification.channels} channels ({p.result.verification.channelLayout || "stereo"}) ·{" "}
              {p.result.verification.codec} · {(p.result.verification.sampleRate / 1000).toFixed(1)} kHz
            </li>
            <li>
              Duration {p.result.verification.duration.toFixed(3)} s (mixed {p.result.verification.expectedDuration.toFixed(3)} s,{" "}
              {p.result.verification.durationDiffMs >= 0 ? "+" : ""}
              {p.result.verification.durationDiffMs.toFixed(1)} ms)
            </li>
            <li>
              Peak {fmtDb(p.result.peakDbfs)} · gain {fmtDb(p.result.appliedGainDb)}
              {p.result.clippedSamples > 0 ? ` · ${p.result.clippedSamples} samples over 0 dBFS` : " · no clipping"}
            </li>
          </ul>
          {p.result.verification.notes.length > 0 && (
            <ul className="result-notes">
              {p.result.verification.notes.map((n) => (
                <li key={n}>{n}</li>
              ))}
            </ul>
          )}
          <button className="link-btn" onClick={() => setShowDetails(!showDetails)}>
            {showDetails ? "Hide" : "Show"} file description
          </button>
          {showDetails && <p className="hint">{p.result.description}</p>}
        </div>
      )}
    </section>
  );
}
