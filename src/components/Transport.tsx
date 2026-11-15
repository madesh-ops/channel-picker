import { useEffect, useRef, useState } from "react";
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
  // While dragging, the bar shows the drag position instead of playback.
  const [scrub, setScrub] = useState<number | null>(null);
  const pending = useRef<number | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const onSeekRef = useRef(onSeek);
  onSeekRef.current = onSeek;

  // Finishing a drag must never depend on the mouse being released over the
  // bar itself: a release over the title bar or outside the window used to
  // leave the bar frozen in drag mode. Any of these ends the drag.
  useEffect(() => {
    const commit = () => {
      const v = pending.current;
      if (v === null) return;
      pending.current = null;
      setScrub(null);
      onSeekRef.current(v);
    };
    const onMove = (e: PointerEvent) => {
      if (e.buttons === 0) commit(); // button already released somewhere we didn't see
    };
    const el = inputRef.current;
    el?.addEventListener("change", commit); // native: fires once when the value is committed
    window.addEventListener("pointerup", commit);
    window.addEventListener("pointermove", onMove);
    window.addEventListener("blur", commit);
    return () => {
      el?.removeEventListener("change", commit);
      window.removeEventListener("pointerup", commit);
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("blur", commit);
    };
  }, []);

  const shown = scrub ?? position;
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
        ref={inputRef}
        className="seek"
        type="range"
        min={0}
        max={Math.max(duration, 0.001)}
        step={0.1}
        value={Math.min(shown, duration)}
        disabled={disabled}
        onChange={(e) => {
          const v = Number(e.target.value);
          pending.current = v;
          setScrub(v);
        }}
        aria-label="Seek"
      />
      <span className="time">{formatTime(duration)}</span>
    </div>
  );
}
