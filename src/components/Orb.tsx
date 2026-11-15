import { useEffect, useRef } from "react";
import type { Tick } from "../lib/types";

/**
 * Dot-matrix orb: a rotating sphere of dots pushed outward by the sound.
 * Height maps to frequency (bass at the bottom, treble at the top) and the
 * left/right halves of the screen follow the stereo Left/Right output levels.
 */

const DOTS = 1400;
const GOLDEN = Math.PI * (3 - Math.sqrt(5));
const TILT = 0.38;

// Fibonacci sphere: evenly spread unit vectors.
const POINTS = Array.from({ length: DOTS }, (_, i) => {
  const y = 1 - (2 * (i + 0.5)) / DOTS;
  const r = Math.sqrt(1 - y * y);
  const t = i * GOLDEN;
  return { x: Math.cos(t) * r, y, z: Math.sin(t) * r };
});

const LOW = [79, 140, 255]; // accent blue
const MID = [111, 211, 168]; // green
const HIGH = [242, 184, 75]; // warm

function color(d: number, alpha: number) {
  const [a, b, t] = d < 0.5 ? [LOW, MID, d * 2] : [MID, HIGH, (d - 0.5) * 2];
  const c = a.map((v, i) => Math.round(v + (b[i] - v) * t));
  return `rgba(${c[0]},${c[1]},${c[2]},${alpha.toFixed(3)})`;
}

const levelNorm = (peak: number) => (peak <= 1e-5 ? 0 : Math.max(0, Math.min(1, (20 * Math.log10(peak) + 54) / 54)));

export function Orb({ tick }: { tick: Tick | null }) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const target = useRef({ bands: [] as number[], l: 0, r: 0, playing: false });

  useEffect(() => {
    // Light blur across neighbouring bands so the surface flows instead of stepping.
    const raw = tick?.spectrum ?? [];
    const bands = raw.map((v, i) => Math.max(v, 0.6 * (raw[i - 1] ?? 0), 0.6 * (raw[i + 1] ?? 0)));
    target.current = {
      bands,
      l: levelNorm(tick?.outputPeaks?.[0] ?? 0),
      r: levelNorm(tick?.outputPeaks?.[1] ?? 0),
      playing: !!tick?.playing,
    };
  }, [tick]);

  useEffect(() => {
    const canvas = canvasRef.current!;
    const ctx = canvas.getContext("2d")!;
    const reduced = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    let w = 0;
    let h = 0;
    const resize = () => {
      const dpr = window.devicePixelRatio || 1;
      w = canvas.clientWidth;
      h = canvas.clientHeight;
      canvas.width = Math.round(w * dpr);
      canvas.height = Math.round(h * dpr);
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    };
    const ro = new ResizeObserver(resize);
    ro.observe(canvas);
    resize();

    // Smoothed state (fast attack, slow release, like a meter).
    let bands: number[] = [];
    let l = 0;
    let r = 0;
    let angle = 0;
    let last = performance.now();
    let raf = 0;
    const ease = (cur: number, tgt: number, dt: number) => {
      const k = tgt > cur ? 1 - Math.exp(-dt * 28) : 1 - Math.exp(-dt * 5);
      return cur + (tgt - cur) * k;
    };

    const frame = (now: number) => {
      const dt = Math.min(0.1, (now - last) / 1000);
      last = now;
      const tg = target.current;
      const nb = tg.bands.length || 32;
      if (bands.length !== nb) bands = new Array(nb).fill(0);
      for (let i = 0; i < nb; i++) bands[i] = ease(bands[i], tg.playing ? tg.bands[i] ?? 0 : 0, dt);
      l = ease(l, tg.playing ? tg.l : 0, dt);
      r = ease(r, tg.playing ? tg.r : 0, dt);

      const bass = (bands[0] + bands[1] + bands[2] + bands[3]) / 4;
      const energy = bands.reduce((a, b) => a + b, 0) / nb;
      const maxLR = Math.max(l, r, 1e-3);
      angle += dt * (reduced ? 0.05 : 0.18 + energy * 0.9);

      const cx = w / 2;
      const cy = h / 2;
      const R0 = Math.min(w, h) * 0.27;
      const time = now / 1000;
      const ca = Math.cos(angle);
      const sa = Math.sin(angle);
      const ct = Math.cos(TILT);
      const st = Math.sin(TILT);

      ctx.clearRect(0, 0, w, h);
      // Soft glow behind the orb that swells with energy.
      const glow = ctx.createRadialGradient(cx, cy, R0 * 0.2, cx, cy, R0 * (1.6 + energy));
      glow.addColorStop(0, `rgba(79,140,255,${(0.1 + energy * 0.25).toFixed(3)})`);
      glow.addColorStop(1, "rgba(79,140,255,0)");
      ctx.fillStyle = glow;
      ctx.fillRect(0, 0, w, h);

      const pulse = 1 + (reduced ? 0 : bass * 0.1);
      for (const p of POINTS) {
        // Rotate around Y, then tilt toward the viewer.
        const x1 = p.x * ca + p.z * sa;
        const z1 = -p.x * sa + p.z * ca;
        const y2 = p.y * ct - z1 * st;
        const z2 = p.y * st + z1 * ct;

        // Frequency from height (bass at the bottom), interpolated between bands.
        const fpos = ((p.y + 1) / 2) * (nb - 1);
        const b0 = Math.floor(fpos);
        const band = bands[b0] + ((bands[Math.min(nb - 1, b0 + 1)] ?? 0) - bands[b0]) * (fpos - b0);
        // Stereo side from on-screen x after rotation.
        const side = (l + (r - l) * ((x1 + 1) / 2)) / maxLR;
        const d = band * side;

        const idle = 0.018 * Math.sin(time * 1.4 + p.y * 5 + p.x * 3);
        const rad = R0 * pulse * (1 + idle + d * 0.5);
        const persp = 3.2 / (3.2 - z2);
        const sx = cx + x1 * rad * persp;
        const sy = cy - y2 * rad * persp;
        const front = (z2 + 1) / 2; // 0 back .. 1 front
        const size = (0.7 + d * 1.8) * persp * (0.75 + front * 0.6);
        ctx.fillStyle = color(Math.min(1, d * 1.3), 0.12 + front * 0.78);
        ctx.beginPath();
        ctx.arc(sx, sy, size, 0, Math.PI * 2);
        ctx.fill();
      }
      raf = requestAnimationFrame(frame);
    };
    raf = requestAnimationFrame(frame);
    return () => {
      cancelAnimationFrame(raf);
      ro.disconnect();
    };
  }, []);

  return (
    <section className="panel orb-panel">
      <header className="panel-head">
        <h2>Visualizer</h2>
        <span className="hint">{tick?.playing ? "Live output" : "Press play"}</span>
      </header>
      <div className="orb-wrap">
        <canvas ref={canvasRef} className="orb-canvas" aria-label="Sound-reactive dot orb" role="img" />
        <span className="orb-side left">L</span>
        <span className="orb-side right">R</span>
        <span className="orb-axis top">treble</span>
        <span className="orb-axis bottom">bass</span>
      </div>
    </section>
  );
}
