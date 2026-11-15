import type { ReactNode } from "react";
import type { ChannelInfo, ChannelStrip } from "../lib/types";
import { fmtDb, toDb } from "../lib/api";
import { Disclosure, Segmented } from "./ui";

interface Props {
  channels: ChannelInfo[];
  strips: ChannelStrip[];
  isHeard: (s: ChannelStrip) => boolean;
  onChange: (index: number, patch: Partial<ChannelStrip>) => void;
}

type Route = "L" | "R" | "LR" | "custom";
const ROUTES: Record<Exclude<Route, "custom">, [number, number]> = {
  L: [1, 0],
  R: [0, 1],
  LR: [Math.SQRT1_2, Math.SQRT1_2],
};
const routeOf = (s: ChannelStrip): Route =>
  (Object.entries(ROUTES).find(([, [l, r]]) => Math.abs(s.toLeft - l) < 1e-3 && Math.abs(s.toRight - r) < 1e-3)?.[0] as Route) ??
  "custom";

/** Advanced: the 2×N downmix coefficients, one level deeper than the channel cards. */
export function RoutingMatrix({ channels, strips, isHeard, onChange }: Props) {
  const custom = strips.filter((s) => routeOf(s) === "custom").length;
  return (
    <Disclosure
      title="Downmix coefficients"
      summary={custom ? `${custom} custom` : "Standard routing"}
    >
      <p className="t-caption" style={{ margin: "0 4px 4px" }}>
        How much of each source channel feeds stereo Left and Right. 0.707 is −3 dB.
      </p>
      <div className="card" style={{ padding: 12 }}>
        <div className="matrix" style={{ gridTemplateColumns: `64px repeat(${channels.length}, minmax(76px, 1fr))` }}>
          <div />
          {channels.map((c, i) => (
            <div key={c.id} className={`mx-col ${isHeard(strips[i]) ? "" : "dim"}`} title={c.label}>
              {c.id}
            </div>
          ))}
          {(["toLeft", "toRight"] as const).map((key) => (
            <Row key={key} name={key === "toLeft" ? "To Left" : "To Right"}>
              {strips.map((s, i) => (
                <div key={s.id} className={`mx-cell ${isHeard(s) ? "" : "dim"}`}>
                  <input
                    className="field-input"
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
          <div className="mx-row">Route</div>
          {strips.map((s, i) => (
            <Segmented
              key={s.id}
              size="sm"
              label={`${channels[i].label} route`}
              value={routeOf(s) === "custom" ? ("" as Route) : routeOf(s)}
              onChange={(r) => {
                const [l, rr] = ROUTES[r as Exclude<Route, "custom">];
                onChange(i, { toLeft: l, toRight: rr });
              }}
              options={[
                { value: "L" as Route, label: "L" },
                { value: "LR" as Route, label: "L+R" },
                { value: "R" as Route, label: "R" },
              ]}
            />
          ))}
        </div>
      </div>
    </Disclosure>
  );
}

function Row({ name, children }: { name: string; children: ReactNode }) {
  return (
    <>
      <div className="mx-row">{name}</div>
      {children}
    </>
  );
}
