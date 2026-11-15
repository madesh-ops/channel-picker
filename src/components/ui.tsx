import { useId, useState, type CSSProperties, type ReactNode } from "react";

/* ------------------------------------------------------------------ */
/* Icons: simple stroked glyphs in the spirit of SF Symbols.            */
/* ------------------------------------------------------------------ */

type IconName =
  | "play" | "pause" | "back10" | "fwd10" | "folder" | "expand" | "eyeSlash" | "eye"
  | "plus" | "xmark" | "chevron" | "check" | "waveform" | "speaker" | "film" | "exclamation";

const PATHS: Record<IconName, ReactNode> = {
  play: <path d="M8 5.5v13l10.5-6.5z" fill="currentColor" stroke="none" />,
  pause: (
    <g fill="currentColor" stroke="none">
      <rect x="6.5" y="5" width="4" height="14" rx="1.2" />
      <rect x="13.5" y="5" width="4" height="14" rx="1.2" />
    </g>
  ),
  back10: (
    <>
      <path d="M12 5a7 7 0 1 1-7 7" />
      <path d="M12 2.5 9 5l3 2.5" />
      <text x="12" y="15.2" fontSize="6.5" textAnchor="middle" fill="currentColor" stroke="none" fontWeight="700">10</text>
    </>
  ),
  fwd10: (
    <>
      <path d="M12 5a7 7 0 1 0 7 7" />
      <path d="M12 2.5 15 5l-3 2.5" />
      <text x="12" y="15.2" fontSize="6.5" textAnchor="middle" fill="currentColor" stroke="none" fontWeight="700">10</text>
    </>
  ),
  folder: <path d="M3.5 7.5a2 2 0 0 1 2-2h4l2 2h7a2 2 0 0 1 2 2v7a2 2 0 0 1-2 2h-13a2 2 0 0 1-2-2z" />,
  expand: <path d="M14 4h6v6M10 20H4v-6M20 4l-6.5 6.5M4 20l6.5-6.5" />,
  eyeSlash: <path d="M3 3l18 18M10.6 6.1A9.8 9.8 0 0 1 12 6c5 0 8.5 4.5 9.5 6a15 15 0 0 1-3 3.4M6.3 7.6C4.3 9 3 10.9 2.5 12c1 1.5 4.5 6 9.5 6 1.4 0 2.7-.3 3.8-.9M9.9 9.9a3 3 0 0 0 4.2 4.2" />,
  eye: (
    <>
      <path d="M2.5 12C3.5 10.5 7 6 12 6s8.5 4.5 9.5 6c-1 1.5-4.5 6-9.5 6s-8.5-4.5-9.5-6z" />
      <circle cx="12" cy="12" r="3" />
    </>
  ),
  plus: <path d="M12 5v14M5 12h14" />,
  xmark: <path d="M6.5 6.5l11 11M17.5 6.5l-11 11" />,
  chevron: <path d="M9 5.5l6.5 6.5L9 18.5" />,
  check: <path d="M5 12.5l4.5 4.5L19 7.5" />,
  waveform: <path d="M4 10v4M8 7v10M12 4v16M16 7v10M20 10v4" />,
  speaker: <path d="M4 9.5h3.5L12 5.5v13l-4.5-4H4zM15.5 9a4 4 0 0 1 0 6M18 6.5a7.5 7.5 0 0 1 0 11" />,
  film: <path d="M4 5.5h16v13H4zM8 5.5v13M16 5.5v13M4 9.5h4M4 14.5h4M16 9.5h4M16 14.5h4" />,
  exclamation: <path d="M12 3.5 2.5 20h19zM12 10v4.5M12 17.2v.3" />,
};

export function Icon({ name, size = 18, className }: { name: IconName; size?: number; className?: string }) {
  return (
    <svg
      viewBox="0 0 24 24"
      width={size}
      height={size}
      fill="none"
      stroke="currentColor"
      strokeWidth={1.8}
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden
      className={className}
    >
      {PATHS[name]}
    </svg>
  );
}

/* ------------------------------------------------------------------ */
/* Switch: on/off for a single setting (familiar from iOS/macOS).      */
/* ------------------------------------------------------------------ */

export function Switch({
  checked,
  onChange,
  label,
  disabled,
}: {
  checked: boolean;
  onChange: (v: boolean) => void;
  label: string;
  disabled?: boolean;
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      disabled={disabled}
      className={`switch ${checked ? "on" : ""}`}
      onClick={() => onChange(!checked)}
    >
      <span className="switch-knob" />
    </button>
  );
}

/* ------------------------------------------------------------------ */
/* Segmented control with a sliding selection thumb.                    */
/* ------------------------------------------------------------------ */

export function Segmented<T extends string>({
  options,
  value,
  onChange,
  label,
  size = "md",
  disabled,
}: {
  options: { value: T; label: string; title?: string }[];
  value: T;
  onChange: (v: T) => void;
  label: string;
  size?: "sm" | "md";
  disabled?: boolean;
}) {
  const found = options.findIndex((o) => o.value === value);
  const style = { "--n": options.length, "--i": Math.max(0, found) } as CSSProperties;
  return (
    <div className={`seg seg-${size}`} role="radiogroup" aria-label={label} style={style}>
      <span className="seg-thumb" aria-hidden style={found < 0 ? { opacity: 0 } : undefined} />
      {options.map((o) => (
        <button
          key={o.value}
          type="button"
          role="radio"
          aria-checked={o.value === value}
          title={o.title}
          disabled={disabled}
          className={o.value === value ? "active" : ""}
          onClick={() => onChange(o.value)}
        >
          {o.label}
        </button>
      ))}
    </div>
  );
}

/* ------------------------------------------------------------------ */
/* Slider: native range input, styled; the filled part tracks value.    */
/* ------------------------------------------------------------------ */

export function Slider({
  min,
  max,
  step,
  value,
  onChange,
  resetTo,
  label,
  disabled,
}: {
  min: number;
  max: number;
  step: number;
  value: number;
  onChange: (v: number) => void;
  resetTo?: number;
  label: string;
  disabled?: boolean;
}) {
  const p = ((value - min) / (max - min)) * 100;
  return (
    <input
      className="slider"
      type="range"
      min={min}
      max={max}
      step={step}
      value={value}
      disabled={disabled}
      aria-label={label}
      title={resetTo !== undefined ? "Double-click to reset" : undefined}
      style={{ "--p": `${p}%` } as CSSProperties}
      onChange={(e) => onChange(Number(e.target.value))}
      onDoubleClick={() => resetTo !== undefined && onChange(resetTo)}
    />
  );
}

/* ------------------------------------------------------------------ */
/* Disclosure: advanced content one level deeper.                       */
/* ------------------------------------------------------------------ */

export function Disclosure({
  title,
  summary,
  defaultOpen = false,
  children,
}: {
  title: string;
  summary?: ReactNode;
  defaultOpen?: boolean;
  children: ReactNode;
}) {
  const [open, setOpen] = useState(defaultOpen);
  const id = useId();
  return (
    <section className={`disclosure ${open ? "open" : ""}`}>
      <button type="button" className="disclosure-head" aria-expanded={open} aria-controls={id} onClick={() => setOpen(!open)}>
        <Icon name="chevron" size={14} className="disclosure-chevron" />
        <span className="disclosure-title">{title}</span>
        {summary && <span className="disclosure-summary">{summary}</span>}
      </button>
      <div className="disclosure-body" id={id} inert={!open}>
        <div className="disclosure-inner">{children}</div>
      </div>
    </section>
  );
}

/** Section heading used inside cards and the sidebar. */
export function SectionTitle({ children, aside }: { children: ReactNode; aside?: ReactNode }) {
  return (
    <div className="section-title">
      <h2>{children}</h2>
      {aside && <div className="section-aside">{aside}</div>}
    </div>
  );
}
