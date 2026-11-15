import { useState } from "react";
import type { Preset } from "../lib/types";

interface Props {
  presets: Preset[];
  current: string | null;
  disabled: boolean;
  onApply: (name: string) => void;
  onSave: (name: string) => Promise<void>;
  onDelete: (name: string) => void;
}

export function PresetBar({ presets, current, disabled, onApply, onSave, onDelete }: Props) {
  const [saving, setSaving] = useState(false);
  const [name, setName] = useState("");
  const builtins = presets.filter((p) => p.builtin);
  const user = presets.filter((p) => !p.builtin);
  const currentPreset = presets.find((p) => p.name === current);

  return (
    <section className="panel presets">
      <header className="panel-head">
        <h2>Presets</h2>
      </header>
      <div className="preset-row">
        <select
          value={current ?? ""}
          disabled={disabled}
          onChange={(e) => e.target.value && onApply(e.target.value)}
          aria-label="Channel-selection preset"
        >
          <option value="" disabled>
            Custom (unsaved)
          </option>
          <optgroup label="Built-in">
            {builtins.map((p) => (
              <option key={p.name} value={p.name}>
                {p.name}
              </option>
            ))}
          </optgroup>
          {user.length > 0 && (
            <optgroup label="My presets">
              {user.map((p) => (
                <option key={p.name} value={p.name}>
                  {p.name}
                </option>
              ))}
            </optgroup>
          )}
        </select>
        {currentPreset && !currentPreset.builtin && (
          <button className="ghost-btn" onClick={() => onDelete(currentPreset.name)} title="Delete this preset">
            Delete
          </button>
        )}
      </div>
      {currentPreset?.description && <p className="hint">{currentPreset.description}</p>}

      {saving ? (
        <form
          className="preset-row"
          onSubmit={async (e) => {
            e.preventDefault();
            if (!name.trim()) return;
            await onSave(name.trim());
            setSaving(false);
            setName("");
          }}
        >
          <input
            autoFocus
            placeholder="Preset name"
            value={name}
            onChange={(e) => setName(e.target.value)}
            onKeyDown={(e) => e.key === "Escape" && setSaving(false)}
            aria-label="New preset name"
          />
          <button type="submit" className="primary-btn" disabled={!name.trim()}>
            Save
          </button>
          <button type="button" className="ghost-btn" onClick={() => setSaving(false)}>
            Cancel
          </button>
        </form>
      ) : (
        <button className="ghost-btn wide" disabled={disabled} onClick={() => setSaving(true)}>
          Save current selection as preset…
        </button>
      )}
    </section>
  );
}
