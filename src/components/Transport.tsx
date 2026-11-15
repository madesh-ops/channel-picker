import { useRef, useState, type KeyboardEvent, type PointerEvent } from "react";
import { formatTime } from "../lib/api";
import { Icon } from "./ui";

interface Props {
  position: number;
  duration: number;
  playing: boolean;
  disabled: boolean;
  onToggle: () => void;
  onSeek: (pos: number) => void;
}

export function Transport({ position, duration, playing, disabled, onToggle, onSeek }: Props) {
  const [drag, setDrag] = useState<number | null>(null);
  const shown = drag ?? position;
  const skip = (d: number) => onSeek(Math.max(0, Math.min(duration, position + d)));
  return (
    <div className="transport">
      <button className="icon-btn" onClick={() => skip(-10)} disabled={disabled} aria-label="Back 10 seconds" title="Back 10 s">
        <Icon name="back10" size={20} />
      </button>
      <button className="play-btn" onClick={onToggle} disabled={disabled} aria-label={playing ? "Pause" : "Play"} title="Play/Pause (Space)">
        <Icon name={playing ? "pause" : "play"} size={18} />
      </button>
      <button className="icon-btn" onClick={() => skip(10)} disabled={disabled} aria-label="Forward 10 seconds" title="Forward 10 s">
        <Icon name="fwd10" size={20} />
      </button>
      <span className="time" aria-hidden>{formatTime(shown)}</span>
      <Scrubber value={position} max={duration} disabled={disabled} onDrag={setDrag} onCommit={onSeek} />
      <span className="time" aria-hidden>-{formatTime(Math.max(0, duration - shown))}</span>
    </div>
  );
}

/**
 * Seek bar tracked 1:1 with pointer capture: the drag follows the pointer
 * even outside the bar, and always ends (pointerup or lost capture), so it
 * can never be left stuck in drag mode.
 */
function Scrubber({
  value,
  max,
  disabled,
  onDrag,
  onCommit,
}: {
  value: number;
  max: number;
  disabled: boolean;
  onDrag: (v: number | null) => void;
  onCommit: (v: number) => void;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const dragRef = useRef<number | null>(null);
  const [dragging, setDragging] = useState<number | null>(null);

  const at = (clientX: number) => {
    const r = ref.current!.getBoundingClientRect();
    return Math.max(0, Math.min(1, (clientX - r.left) / r.width)) * max;
  };
  const update = (v: number | null) => {
    dragRef.current = v;
    setDragging(v);
    onDrag(v);
  };
  const finish = () => {
    const v = dragRef.current;
    if (v === null) return;
    update(null);
    onCommit(v);
  };

  const onPointerDown = (e: PointerEvent<HTMLDivElement>) => {
    if (disabled || e.button !== 0) return;
    ref.current!.setPointerCapture(e.pointerId);
    update(at(e.clientX)); // respond on press
  };
  const onPointerMove = (e: PointerEvent<HTMLDivElement>) => {
    if (dragRef.current !== null) update(at(e.clientX));
  };
  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    if (disabled) return;
    const step = { ArrowLeft: -5, ArrowRight: 5, PageDown: -30, PageUp: 30 }[e.key];
    if (step !== undefined) onCommit(Math.max(0, Math.min(max, value + step)));
    else if (e.key === "Home") onCommit(0);
    else if (e.key === "End") onCommit(max);
    else return;
    e.preventDefault();
  };

  const shown = dragging ?? value;
  const pct = max > 0 ? (Math.min(shown, max) / max) * 100 : 0;
  return (
    <div
      ref={ref}
      className={`scrubber ${dragging !== null ? "dragging" : ""}`}
      role="slider"
      tabIndex={disabled ? -1 : 0}
      aria-label="Playback position"
      aria-valuemin={0}
      aria-valuemax={Math.round(max)}
      aria-valuenow={Math.round(shown)}
      aria-valuetext={`${formatTime(shown)} of ${formatTime(max)}`}
      aria-disabled={disabled}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={finish}
      onPointerCancel={finish}
      onLostPointerCapture={finish}
      onKeyDown={onKeyDown}
    >
      <div className="scrubber-track">
        <div className="scrubber-fill" style={{ width: `${pct}%` }} />
      </div>
    </div>
  );
}
