import { useState } from "react";
import { formatTime } from "../lib/api";

interface Props {
  position: number;
  duration: number;
  playing: boolean;
  disabled: boolean;
  onToggle: () => void;
  onSeek: (pos: number) => void;
}

export function Transport({ position, duration, playing, disabled, onToggle, onSeek }: Props) {
  const [scrub, setScrub] = useState<number | null>(null);
  const shown = scrub ?? position;
  const commit = () => {
    if (scrub !== null) onSeek(scrub);
    setScrub(null);
  };
  return (
    <div className="transport">
      <button className="play" onClick={onToggle} disabled={disabled} aria-label={playing ? "Pause" : "Play"}>
        {playing ? (
          <svg viewBox="0 0 24 24" width="18" height="18" aria-hidden>
            <rect x="6" y="5" width="4" height="14" rx="1" fill="currentColor" />
            <rect x="14" y="5" width="4" height="14" rx="1" fill="currentColor" />
          </svg>
        ) : (
          <svg viewBox="0 0 24 24" width="18" height="18" aria-hidden>
            <path d="M7 5l12 7-12 7z" fill="currentColor" />
          </svg>
        )}
      </button>
      <span className="time">{formatTime(shown)}</span>
      <input
        className="seek"
        type="range"
        min={0}
        max={Math.max(duration, 0.001)}
        step={0.1}
        value={Math.min(shown, duration)}
        disabled={disabled}
        onChange={(e) => setScrub(Number(e.target.value))}
        onPointerUp={commit}
        onKeyUp={commit}
        aria-label="Seek"
      />
      <span className="time">{formatTime(duration)}</span>
    </div>
  );
}
