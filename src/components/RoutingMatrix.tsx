import type { ReactNode } from "react";
import type { ChannelInfo, ChannelStrip } from "../lib/types";
import { fmtDb, toDb } from "../lib/api";

interface Props {
  channels: ChannelInfo[];
  strips: ChannelStrip[];
  isHeard: (s: ChannelStrip) => boolean;
  onChange: (index: number, patch: Partial<ChannelStrip>) => void;
}

const QUICK: { label: string; l: number; r: number }[] = [
  { label: "L", l: 1, r: 0 },
  { label: "R", l: 0, r: 1 },
  { label: "L+R", l: Math.SQRT1_2, r: Math.SQRT1_2 },
];

/** 2×N downmix coefficient grid: how much of each source channel feeds stereo L and R. */
export function RoutingMatrix({ channels, strips, isHeard, onChange }: Props) {
  return (
    <section className="panel routing">
      <header className="panel-head">
        <h2>Downmix coefficients</h2>
        <p className="hint">Linear gain from each source channel into stereo Left / Right. 0.707 = −3 dB.</p>
      </header>
      <div className="matrix" style={{ gridTemplateColumns: `72px repeat(${channels.length}, minmax(84px, 1fr))` }}>
        <div className="mx-corner" />
        {channels.map((c, i) => (
          <div key={c.id} className={`mx-col ${isHeard(strips[i]) ? "" : "dim"}`} title={c.label}>
            {c.id}
          </div>
        ))}
        {(["toLeft", "toRight"] as const).map((key) => (
          <Row key={key} name={key === "toLeft" ? "→ Left" : "→ Right"}>
            {strips.map((s, i) => (
              <div key={s.id} className={`mx-cell ${isHeard(s) ? "" : "dim"}`}>
                <input
                  type="number"
                  min={0}
                  max={2}
                  step={0.05}
                  value={Number(s[key].toFixed(3))}
                  onChange={(e) => {
                    const v = Math.max(0, Math.min(2, Number(e.target.value) || 0));
                    onChange(i, { [key]: v } as Partial<ChannelStrip>);
                  }}
                  aria-label={`${channels[i].label} ${key === "toLeft" ? "to Left" : "to Right"}`}
                />
                <span className="mx-db">{s[key] > 0 ? fmtDb(toDb(s[key])) : "off"}</span>
              </div>
            ))}
          </Row>
        ))}
        <div className="mx-row-name">Assign</div>
        {strips.map((s, i) => (
          <div key={s.id} className="mx-quick">
            {QUICK.map((q) => {
              const active = Math.abs(s.toLeft - q.l) < 1e-3 && Math.abs(s.toRight - q.r) < 1e-3;
              return (
                <button
                  key={q.label}
                  className={active ? "active" : ""}
                  onClick={() => onChange(i, { toLeft: q.l, toRight: q.r })}
                  title={`Route ${channels[i].id} to ${q.label}`}
                >
                  {q.label}
                </button>
              );
            })}
          </div>
        ))}
      </div>
    </section>
  );
}

function Row({ name, children }: { name: string; children: ReactNode }) {
  return (
    <>
      <div className="mx-row-name">{name}</div>
      {children}
    </>
  );
}
