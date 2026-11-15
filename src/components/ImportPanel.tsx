import type { AppStatus } from "../lib/types";

interface Props {
  status: AppStatus | null;
  dragging: boolean;
  loading: boolean;
  error: string | null;
  onOpen: () => void;
  onLocateFfmpeg: () => void;
}

export function ImportPanel({ status, dragging, loading, error, onOpen, onLocateFfmpeg }: Props) {
  const ready = !!status?.ffmpeg;
  return (
    <div className={`import ${dragging ? "dragging" : ""}`}>
      <div className="import-card">
        <svg viewBox="0 0 64 64" width="56" height="56" aria-hidden className="import-icon">
          <rect x="6" y="22" width="6" height="20" rx="2" />
          <rect x="18" y="14" width="6" height="36" rx="2" />
          <rect x="30" y="8" width="6" height="48" rx="2" className="accent" />
          <rect x="42" y="14" width="6" height="36" rx="2" className="accent" />
          <rect x="54" y="22" width="6" height="20" rx="2" />
        </svg>
        <h1>Pick channels from a 5.1 soundtrack</h1>
        <p>
          Open a movie or audio file. Choose which of FL, FR, Center, LFE, SL and SR to keep, preview the mix,
          and export a standard stereo file for headphones or 2.0 speakers.
        </p>
        <button className="primary-btn big" onClick={onOpen} disabled={!ready || loading}>
          {loading ? "Reading file…" : "Open video or audio file…"}
        </button>
        <p className="hint">or drop a file anywhere in this window</p>
        {error && <p className="error">{error}</p>}

        <div className="support">
          {status?.error ? (
            <div className="error">
              {status.error}.{" "}
              <button className="link-btn" onClick={onLocateFfmpeg}>
                Locate the ffmpeg folder…
              </button>
            </div>
          ) : (
            <>
              <div className="support-row">
                <span className="support-head">Decodes</span>
                {status?.decoders.map((d) => (
                  <span key={d.codec} className={`chip ${d.available ? "" : "missing"}`} title={d.codec}>
                    {d.label}
                  </span>
                ))}
              </div>
              <div className="support-row">
                <span className="support-head">Exports</span>
                {status?.encoders.map((d) => (
                  <span key={d.codec} className={`chip ${d.available ? "" : "missing"}`} title={d.codec}>
                    {d.label}
                  </span>
                ))}
              </div>
              {status?.ffmpeg && <div className="muted small">{status.ffmpeg.version}</div>}
            </>
          )}
        </div>
      </div>
    </div>
  );
}
