import { useEffect, useRef, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { VideoInfo } from "../lib/types";

/** Serves the latest decoded frame (see `vframe` protocol in lib.rs). */
const FRAME_URL = convertFileSrc("frame", "vframe");

interface Props {
  video: VideoInfo;
  enabled: boolean;
  fullQuality: boolean;
  onToggle: (enabled: boolean) => void;
  onQuality: (full: boolean) => void;
}

/**
 * Silent picture synced to the preview audio. Frames are decoded by ffmpeg in
 * the backend and timed to the audio clock, so any container/codec ffmpeg can
 * read works (MKV, M2TS, AVI, ...), not just what the webview can play.
 */
export function VideoPanel({ video, enabled, fullQuality, onToggle, onQuality }: Props) {
  const frameRef = useRef<HTMLDivElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [hasFrame, setHasFrame] = useState(false);
  const [frameSize, setFrameSize] = useState<[number, number] | null>(null);
  const [isFull, setIsFull] = useState(false);

  useEffect(() => {
    if (!enabled) return;
    let loading = false;
    let pending = false;
    let alive = true;
    const load = async () => {
      if (loading) {
        pending = true; // only the newest frame matters
        return;
      }
      loading = true;
      try {
        const res = await fetch(`${FRAME_URL}?t=${performance.now()}`, { cache: "no-store" });
        if (res.ok) {
          // Decoded off the main thread, so large frames don't stall the UI.
          const bmp = await createImageBitmap(await res.blob());
          const c = canvasRef.current;
          if (alive && c) {
            if (c.width !== bmp.width || c.height !== bmp.height) {
              c.width = bmp.width;
              c.height = bmp.height;
              setFrameSize([bmp.width, bmp.height]);
            }
            c.getContext("2d")!.drawImage(bmp, 0, 0);
            setHasFrame(true);
          }
          bmp.close();
        }
      } catch {
        /* a dropped frame is fine; the next one replaces it */
      }
      loading = false;
      if (pending && alive) {
        pending = false;
        load();
      }
    };
    load(); // poster frame may already be there
    const un = listen<number>("video:frame", () => load());
    return () => {
      alive = false;
      un.then((f) => f());
    };
  }, [enabled, video.index]);

  useEffect(() => {
    setHasFrame(false);
    setFrameSize(null);
  }, [video.index]);

  useEffect(() => {
    const onChange = () => setIsFull(document.fullscreenElement === frameRef.current);
    document.addEventListener("fullscreenchange", onChange);
    return () => document.removeEventListener("fullscreenchange", onChange);
  }, []);

  const toggleFullscreen = () => {
    if (document.fullscreenElement) document.exitFullscreen();
    else frameRef.current?.requestFullscreen().catch(() => {});
  };

  const fps = video.fps.toFixed(video.fps % 1 ? 3 : 0);
  return (
    <section className="panel video-panel">
      <header className="panel-head">
        <h2>Video</h2>
        <span className="hint">
          {video.width}×{video.height} · {video.codecName} · {fps} fps
          {enabled && frameSize && ` · showing ${frameSize[0]}×${frameSize[1]}`}
        </span>
        <div className="head-actions">
          {enabled && (
            <>
              <div className="segmented compact" role="radiogroup" aria-label="Preview quality">
                <button
                  role="radio"
                  aria-checked={fullQuality}
                  className={fullQuality ? "active" : ""}
                  onClick={() => onQuality(true)}
                  title="Source resolution and frame rate"
                >
                  Full quality
                </button>
                <button
                  role="radio"
                  aria-checked={!fullQuality}
                  className={!fullQuality ? "active" : ""}
                  onClick={() => onQuality(false)}
                  title="960 px, 30 fps: lighter on slower PCs"
                >
                  Fast
                </button>
              </div>
              <button className="ghost-btn" onClick={toggleFullscreen} title="Full screen (Esc to exit)">
                Full screen
              </button>
            </>
          )}
          <button className="ghost-btn" onClick={() => onToggle(!enabled)} aria-pressed={enabled}>
            {enabled ? "Hide" : "Show video"}
          </button>
        </div>
      </header>
      {enabled && (
        <div
          ref={frameRef}
          className={`video-frame ${isFull ? "is-full" : ""}`}
          style={isFull ? undefined : { aspectRatio: String(video.aspect || 16 / 9) }}
          onDoubleClick={toggleFullscreen}
        >
          <canvas ref={canvasRef} aria-label="Video preview" />
          {!hasFrame && <div className="video-wait">Loading picture…</div>}
        </div>
      )}
    </section>
  );
}
