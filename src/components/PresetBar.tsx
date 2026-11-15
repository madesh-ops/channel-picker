import { useState } from "react";
import type { Preset } from "../lib/types";
import { Icon } from "./ui";

interface Props {
  presets: Preset[];
  current: string | null;
  disabled: boolean;
  onApply: (name: string) => void;
  onSave: (name: string) => Promise<void>;
  onDelete: (name: string) => void;
}

/** Presets as a selectable source list: one click applies. */
export function PresetBar({ presets, current, disabled, onApply, onSave, onDelete }: Props) {
  const [saving, setSaving] = useState(false);
  const [name, setName] = useState("");
  const builtins = presets.filter((p) => p.builtin);
  const user = presets.filter((p) => !p.builtin);

  const item = (p: Preset) => {
    const selected = p.name === current;
    return (
      <div key={p.name} className={`preset-wrap ${selected ? "selected" : ""}`}>
        <button
          className={`preset-item ${selected ? "selected" : ""}`}
          aria-pressed={selected}
          disabled={disabled}
          onClick={() => onApply(p.name)}
        >
          <span className="p-check">{selected && <Icon name="check" size={14} />}</span>
          <span className="p-text">
            <span className="p-name">{p.name}</span>
            {p.description && <span className="p-desc" style={{ display: "block" }}>{p.description}</span>}
          </span>
        </button>
        {!p.builtin && (
          <button
            className="icon-btn p-del"
            onClick={() => onDelete(p.name)}
            aria-label={`Delete preset ${p.name}`}
            title="Delete preset"
          >
            <Icon name="xmark" size={13} />
          </button>
        )}
      </div>
    );
  };

  return (
    <section>
      <div className="group-label">Presets</div>
      <div className="preset-list">{builtins.map(item)}</div>
      {user.length > 0 && (
        <>
          <div className="group-label" style={{ marginTop: 14 }}>
            My Presets
          </div>
          <div className="preset-list">{user.map(item)}</div>
        </>
      )}
      {current === null && (
        <p className="t-caption" style={{ margin: "8px 10px 0" }}>
          Current selection is custom.
        </p>
      )}

      {saving ? (
        <form
          className="preset-save"
          onSubmit={async (e) => {
            e.preventDefault();
            if (!name.trim()) return;
            try {
              await onSave(name.trim());
              setSaving(false);
              setName("");
            } catch {
              /* error shown by parent */
            }
          }}
        >
          <input
            className="field-input"
            autoFocus
            placeholder="Preset name"
            value={name}
            onChange={(e) => setName(e.target.value)}
            onKeyDown={(e) => e.key === "Escape" && setSaving(false)}
            aria-label="New preset name"
          />
          <button type="submit" className="btn primary" disabled={!name.trim()}>
            Save
          </button>
        </form>
      ) : (
        <button className="btn plain" style={{ marginTop: 6 }} disabled={disabled} onClick={() => setSaving(true)}>
          <Icon name="plus" size={15} /> Save as Preset…
        </button>
      )}
    </section>
  );
}
