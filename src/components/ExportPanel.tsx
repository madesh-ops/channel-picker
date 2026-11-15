import type { ExportProgress, ExportResult, OutputFormat } from "../lib/types";
import { fmtDb } from "../lib/api";
import { Disclosure, Icon, SectionTitle, Segmented, Switch } from "./ui";

export interface ExportOptions {
  format: OutputFormat;
  sampleRate: number | null;
  normalize: boolean;
  normalizeDbfs: number;
  align: boolean;
}

type Family = OutputFormat["kind"];

/** Quality choices per format, most common first. */
const QUALITIES: Record<Family, { key: string; label: string; format: OutputFormat }[]> = {
  wav: [
    { key: "wav24", label: "24-bit", format: { kind: "wav", bits: 24 } },
    { key: "wav16", label: "16-bit (dithered)", format: { kind: "wav", bits: 16 } },
    { key: "wav32", label: "32-bit float", format: { kind: "wav", bits: 32 } },
  ],
  flac: [
    { key: "flac24", label: "24-bit lossless", format: { kind: "flac", bits: 24 } },
    { key: "flac16", label: "16-bit lossless", format: { kind: "flac", bits: 16 } },
  ],
  mp3: [
    { key: "mp3cbr", label: "320 kbps", format: { kind: "mp3", vbr: false } },
    { key: "mp3vbr", label: "V0 (variable)", format: { kind: "mp3", vbr: true } },
  ],
  aac: [
    { key: "aac256", label: "256 kbps", format: { kind: "aac", kbps: 256 } },
    { key: "aac192", label: "192 kbps", format: { kind: "aac", kbps: 192 } },
  ],
};
const keyOf = (f: OutputFormat) => QUALITIES[f.kind].find((q) => JSON.stringify(q.format) === JSON.stringify(f))?.key;

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
  const family = p.opts.format.kind;
  const pct = p.progress ? Math.round(p.progress.fraction * 100) : 0;
  const set = (patch: Partial<ExportOptions>) => p.setOpts({ ...p.opts, ...patch });
  const v = p.result?.verification;

  return (
    <section className="card">
      <SectionTitle>Export</SectionTitle>

      <Segmented
        label="File format"
        value={family}
        disabled={p.busy}
        onChange={(f) => set({ format: QUALITIES[f][0].format })}
        options={[
          { value: "wav", label: "WAV" },
          { value: "flac", label: "FLAC" },
          { value: "mp3", label: "MP3" },
          { value: "aac", label: "AAC" },
        ]}
      />

      <div style={{ marginTop: 12 }}>
        <label className="field">
          <span className="lbl">Quality</span>
          <select
            className="select"
            value={keyOf(p.opts.format)}
            disabled={p.busy}
            onChange={(e) => set({ format: QUALITIES[family].find((q) => q.key === e.target.value)!.format })}
          >
            {QUALITIES[family].map((q) => (
              <option key={q.key} value={q.key}>
                {q.label}
              </option>
            ))}
          </select>
        </label>
        <label className="field">
          <span className="lbl">Sample rate</span>
          <select
            className="select"
            value={p.opts.sampleRate ?? ""}
            disabled={p.busy}
            onChange={(e) => set({ sampleRate: e.target.value ? Number(e.target.value) : null })}
          >
            <option value="">Same as source ({(p.sourceRate / 1000).toFixed(1)} kHz)</option>
            <option value="44100">44.1 kHz</option>
            <option value="48000">48 kHz</option>
            <option value="96000">96 kHz</option>
          </select>
        </label>
      </div>

      <div style={{ marginTop: 10 }}>
        <div className="toggle-row">
          <div className="grow">
            <div>Keep video sync</div>
            <div className="t-caption">
              {p.startOffset > 0.0005
                ? `Adds ${(p.startOffset * 1000).toFixed(0)} ms of silence at the start`
                : p.hasVideo
                  ? "This track already starts with the video"
                  : "Only matters for audio from video files"}
            </div>
          </div>
          <Switch checked={p.opts.align} disabled={p.busy} onChange={(align) => set({ align })} label="Keep video sync" />
        </div>
        <div className="toggle-row">
          <div className="grow">
            <div>Normalize peaks</div>
            <div className="t-caption">Scan first, then set the loudest peak to a target</div>
          </div>
          <Switch
            checked={p.opts.normalize}
            disabled={p.busy}
            onChange={(normalize) => set({ normalize })}
            label="Normalize peaks"
          />
        </div>
        {p.opts.normalize && (
          <div className="row" style={{ margin: "2px 0 4px", fontSize: 12.5 }}>
            <span className="secondary">Target peak</span>
            <input
              className="field-input small-num num"
              type="number"
              step={0.5}
              max={0}
              min={-24}
              value={p.opts.normalizeDbfs}
              disabled={p.busy}
              onChange={(e) => set({ normalizeDbfs: Math.min(0, Number(e.target.value)) })}
              aria-label="Normalization target in dBFS"
            />
            <span className="secondary">dBFS</span>
          </div>
        )}
      </div>

      <div style={{ marginTop: 12 }}>
        {p.busy ? (
          <div>
            <div className="progress" role="progressbar" aria-valuenow={pct} aria-valuemin={0} aria-valuemax={100}>
              <div style={{ width: `${pct}%` }} />
            </div>
            <div className="row between" style={{ marginTop: 8, fontSize: 12 }}>
              <span className="secondary">
                {p.progress?.phase === "analyzing" ? "Measuring peak level…" : "Exporting…"} {pct}%
              </span>
              <button className="btn plain" onClick={p.onCancel}>
                Cancel
              </button>
            </div>
          </div>
        ) : (
          <button className="btn primary block large" disabled={p.disabled} onClick={p.onExport}>
            Export Stereo File…
          </button>
        )}
      </div>

      {p.error && (
        <div className="alert" role="alert" style={{ marginTop: 12 }}>
          <Icon name="exclamation" className="icon" size={16} />
          <div className="grow">{p.error}</div>
        </div>
      )}

      {p.result && v && !p.busy && (
        <div className={`result ${v.ok ? "ok" : "warn"}`} role="status">
          <div className="result-title">
            <Icon name={v.ok ? "check" : "exclamation"} size={15} />
            {v.ok ? "Exported and checked" : "Exported with warnings"}
          </div>
          <div className="result-path" title={p.result.output}>
            <bdi>{p.result.output}</bdi>
          </div>
          <ul>
            <li>
              {p.result.formatLabel} · {v.codec} · {(v.sampleRate / 1000).toFixed(1)} kHz
            </li>
            <li className="num">
              {v.duration.toFixed(3)} s ({v.durationDiffMs >= 0 ? "+" : ""}
              {v.durationDiffMs.toFixed(1)} ms vs mix)
            </li>
            <li className="num">
              Peak {fmtDb(p.result.peakDbfs)}
              {p.result.clippedSamples > 0 ? ` · ${p.result.clippedSamples} samples clipped` : " · no clipping"}
            </li>
          </ul>
          {v.notes.length > 0 && (
            <ul className="notes" style={{ marginTop: 4 }}>
              {v.notes.map((n) => (
                <li key={n}>{n}</li>
              ))}
            </ul>
          )}
          <Disclosure title="File description">
            <p className="t-caption" style={{ margin: "0 4px" }}>
              {p.result.description}
            </p>
          </Disclosure>
        </div>
      )}
    </section>
  );
}
