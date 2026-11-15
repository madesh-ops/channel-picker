import type { AudioStream, MediaInfo } from "../lib/types";
import { formatTime } from "../lib/api";
import { Icon } from "./ui";

interface Props {
  media: MediaInfo;
  stream: AudioStream;
  onSelectStream: (index: number) => void;
  onOpen: () => void;
}

function trackName(s: AudioStream) {
  const bits = [`Track ${s.track}`, s.codecLabel, s.channelLayout];
  if (s.language) bits.push(s.language.toUpperCase());
  if (s.title) bits.push(`“${s.title}”`);
  if (!s.decoderAvailable) bits.push("(can't decode)");
  return bits.join(" · ");
}

export function SourceInfo({ media, stream, onSelectStream, onOpen }: Props) {
  const file = media.path.split(/[\\/]/).pop();
  const native = stream.kind === "multichannel";
  return (
    <section>
      <div className="row between" style={{ alignItems: "flex-start", gap: 8 }}>
        <div style={{ minWidth: 0 }}>
          <div className="t-caption" style={{ display: "flex", alignItems: "center", gap: 5 }}>
            <Icon name={media.hasVideo ? "film" : "speaker"} size={13} />
            {media.hasVideo ? "Video" : "Audio"}
          </div>
          <h1 className="source-name" title={media.path}>
            {file}
          </h1>
        </div>
        <button className="icon-btn" onClick={onOpen} aria-label="Open another file" title="Open another file">
          <Icon name="folder" />
        </button>
      </div>

      <div className="source-badges">
        <span className={`badge ${native ? "green" : "orange"}`} title={stream.kindLabel}>
          <span className="badge-dot" />
          {native ? `Native ${stream.channelLayout}` : stream.kind === "stereo" ? "Stereo source" : "Mono source"}
        </span>
        {stream.layoutAssumed && <span className="badge orange">Layout assumed</span>}
      </div>

      {media.audioStreams.length > 1 && (
        <label className="field" style={{ marginBottom: 12 }}>
          <span className="lbl">Audio track</span>
          <select className="select" value={stream.index} onChange={(e) => onSelectStream(Number(e.target.value))}>
            {media.audioStreams.map((s) => (
              <option key={s.index} value={s.index} disabled={!s.decoderAvailable}>
                {trackName(s)}
              </option>
            ))}
          </select>
        </label>
      )}

      <div className="list">
        <div className="list-row">
          <span className="k">Codec</span>
          <span className="v" title={stream.codecName}>{stream.codecLabel}</span>
        </div>
        <div className="list-row">
          <span className="k">Channels</span>
          <span className="v">
            {stream.channels} · {stream.channelLayout}
          </span>
        </div>
        <div className="list-row">
          <span className="k">Sample rate</span>
          <span className="v num">{(stream.sampleRate / 1000).toFixed(1)} kHz</span>
        </div>
        <div className="list-row">
          <span className="k">Duration</span>
          <span className="v num">{formatTime(stream.duration)}</span>
        </div>
        {stream.bitRate && (
          <div className="list-row">
            <span className="k">Bit rate</span>
            <span className="v num">{Math.round(stream.bitRate / 1000)} kbps</span>
          </div>
        )}
        <div className="list-row">
          <span className="k">Container</span>
          <span className="v" title={media.formatLongName}>{media.formatLongName || media.formatName}</span>
        </div>
      </div>
      {!native && (
        <p className="t-caption" style={{ margin: "8px 4px 0" }}>
          {stream.kindLabel}.
        </p>
      )}
      {!stream.decoderAvailable && (
        <p className="error-text">This ffmpeg build can't decode {stream.codecName}. Choose another track.</p>
      )}
    </section>
  );
}
