import type { AppStatus } from "../lib/types";
import { Icon } from "./ui";

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
        <div className="import-art" aria-hidden>
          <Icon name="waveform" size={40} />
        </div>
        <h1 className="t-display">Pick channels from a 5.1&nbsp;soundtrack</h1>
        <p className="lede">
          Choose which of Front, Center, LFE and Surround to keep, hear the mix as you go, and save it as a standard
          stereo file for headphones or two speakers.
        </p>

        <div className="drop-zone">
          <button className="btn primary large" onClick={onOpen} disabled={!ready || loading}>
            <Icon name="folder" size={17} />
            {loading ? "Reading file…" : "Open Video or Audio…"}
          </button>
          <span className="t-caption">{dragging ? "Drop to open" : "or drop a file anywhere in this window"}</span>
        </div>

        {error && (
          <div className="alert" role="alert" style={{ width: "100%", textAlign: "left" }}>
            <Icon name="exclamation" className="icon" size={16} />
            <div className="grow">{error}</div>
          </div>
        )}

        <div className="support">
          {status?.error ? (
            <div className="alert" style={{ textAlign: "left" }}>
              <Icon name="exclamation" className="icon" size={16} />
              <div className="grow">
                {status.error}.{" "}
                <button className="link" onClick={onLocateFfmpeg}>
                  Locate ffmpeg folder…
                </button>
              </div>
            </div>
          ) : (
            <>
              <div className="chips">
                <span className="chips-label">Decodes</span>
                {status?.decoders.map((d) => (
                  <span key={d.codec} className={`chip ${d.available ? "ok" : "missing"}`} title={d.codec}>
                    {d.label}
                  </span>
                ))}
              </div>
              <div className="chips">
                <span className="chips-label">Exports</span>
                {status?.encoders.map((d) => (
                  <span key={d.codec} className={`chip ${d.available ? "ok" : "missing"}`} title={d.codec}>
                    {d.label}
                  </span>
                ))}
              </div>
            </>
          )}
        </div>
      </div>
    </div>
  );
}
