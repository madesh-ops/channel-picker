import type { AudioStream, MediaInfo } from "../lib/types";
import { formatTime } from "../lib/api";

interface Props {
  media: MediaInfo;
  stream: AudioStream;
  onSelectStream: (index: number) => void;
  onOpen: () => void;
}

function trackName(s: AudioStream) {
  const bits = [`Track ${s.track}`, s.codecLabel, s.channelLayout];
  if (s.language) bits.push(s.language.toUpperCase());
  if (s.title) bits.push(`"${s.title}"`);
  if (!s.decoderAvailable) bits.push("(cannot decode)");
  return bits.join(" · ");
}

export function SourceInfo({ media, stream, onSelectStream, onOpen }: Props) {
  const file = media.path.split(/[\\/]/).pop();
  const native = stream.kind === "multichannel";
  return (
    <section className="panel source">
      <div className="source-top">
        <div className="source-file">
          <span className="file-kind">{media.hasVideo ? "Video file" : "Audio file"}</span>
          <h1 title={media.path}>{file}</h1>
        </div>
        <button className="ghost-btn" onClick={onOpen}>
          Open another…
        </button>
      </div>

      {media.audioStreams.length > 1 && (
        <label className="field">
          <span>Audio track</span>
          <select value={stream.index} onChange={(e) => onSelectStream(Number(e.target.value))}>
            {media.audioStreams.map((s) => (
              <option key={s.index} value={s.index} disabled={!s.decoderAvailable}>
                {trackName(s)}
              </option>
            ))}
          </select>
        </label>
      )}

      <div className={`kind-badge ${native ? "native" : "stereo"}`}>
        <span className="kind-dot" />
        {stream.kindLabel}
      </div>

      <dl className="facts">
        <dt>Codec</dt>
        <dd>
          {stream.codecLabel}
          <span className="muted"> ({stream.codecName})</span>
        </dd>
        <dt>Channels</dt>
        <dd>
          {stream.channels} · {stream.channelLayout}
          {stream.layoutAssumed && <span className="warn-text"> (layout not stated by file, assumed)</span>}
        </dd>
        <dt>Sample rate</dt>
        <dd>{(stream.sampleRate / 1000).toFixed(1)} kHz</dd>
        <dt>Duration</dt>
        <dd>{formatTime(stream.duration)}</dd>
        {stream.bitRate && (
          <>
            <dt>Bit rate</dt>
            <dd>{Math.round(stream.bitRate / 1000)} kbps</dd>
          </>
        )}
        <dt>Container</dt>
        <dd>{media.formatLongName || media.formatName}</dd>
      </dl>
      {!stream.decoderAvailable && (
        <p className="error">This ffmpeg build has no decoder for {stream.codecName}. Pick another track.</p>
      )}
    </section>
  );
}
