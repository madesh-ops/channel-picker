//! Channel-selection presets: read-only built-ins and user presets saved as JSON.
//! Presets are keyed by channel *role* (see `layout::role`), so a preset made
//! on a "5.1(side)" track works on a "5.1" (back-surround) track too.

use crate::layout;
use crate::mix::{default_routing, ChannelStrip, GainMode, MixSettings, MINUS_3DB};
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PresetChannel {
    pub enabled: bool,
    pub gain_db: f32,
    pub to_left: f32,
    pub to_right: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Preset {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub builtin: bool,
    /// Whether channels not listed in `channels` keep their default routing (true) or are muted.
    pub unlisted_enabled: bool,
    pub channels: BTreeMap<String, PresetChannel>,
    pub master_gain_db: f32,
    pub gain_mode: GainMode,
}

fn pc(enabled: bool, to_left: f32, to_right: f32) -> PresetChannel {
    PresetChannel { enabled, gain_db: 0.0, to_left, to_right }
}

fn builtin(name: &str, description: &str, unlisted_enabled: bool, channels: &[(&str, PresetChannel)]) -> Preset {
    Preset {
        name: name.into(),
        description: description.into(),
        builtin: true,
        unlisted_enabled,
        channels: channels.iter().map(|(k, v)| (k.to_string(), v.clone())).collect(),
        master_gain_db: 0.0,
        gain_mode: GainMode::Safe,
    }
}

pub fn builtins() -> Vec<Preset> {
    let off = |id: &str| {
        let (_, l, r) = default_routing(id);
        pc(false, l, r)
    };
    vec![
        builtin("ITU Downmix", "Standard Lo/Ro fold-down: fronts, centre and surrounds at −3 dB, LFE off.", true, &[]),
        builtin(
            "Surround to Stereo",
            "Only the surround channels: Surround Left → Left, Surround Right → Right.",
            false,
            &[
                ("FL", off("FL")),
                ("FR", off("FR")),
                ("FC", off("FC")),
                ("LFE", off("LFE")),
                ("SL", pc(true, 1.0, 0.0)),
                ("SR", pc(true, 0.0, 1.0)),
            ],
        ),
        builtin("Dialogue Only", "Centre channel only, to both sides.", false, &[("FC", pc(true, MINUS_3DB, MINUS_3DB))]),
        builtin("Fronts Only", "Front Left/Right only, no centre, LFE or surrounds.", false, &[("FL", pc(true, 1.0, 0.0)), ("FR", pc(true, 0.0, 1.0))]),
        builtin("LFE Check", "Subwoofer channel only, to both sides.", false, &[("LFE", pc(true, MINUS_3DB, MINUS_3DB))]),
    ]
}

/// Builds mix settings for a stream's channels from a preset.
pub fn apply(preset: &Preset, ids: &[String]) -> MixSettings {
    let channels = ids
        .iter()
        .map(|id| {
            let role = layout::role(id, ids);
            match preset.channels.get(&role) {
                Some(p) => ChannelStrip {
                    id: id.clone(),
                    enabled: p.enabled,
                    solo: false,
                    gain_db: p.gain_db,
                    to_left: p.to_left,
                    to_right: p.to_right,
                },
                None => {
                    let (enabled, l, r) = default_routing(id);
                    ChannelStrip { id: id.clone(), enabled: enabled && preset.unlisted_enabled, solo: false, gain_db: 0.0, to_left: l, to_right: r }
                }
            }
        })
        .collect();
    MixSettings { channels, master_gain_db: preset.master_gain_db, gain_mode: preset.gain_mode }
}

pub fn from_mix(name: &str, description: &str, mix: &MixSettings) -> Preset {
    let ids: Vec<String> = mix.channels.iter().map(|c| c.id.clone()).collect();
    Preset {
        name: name.trim().into(),
        description: description.trim().into(),
        builtin: false,
        unlisted_enabled: false,
        channels: mix
            .channels
            .iter()
            .map(|c| {
                (layout::role(&c.id, &ids), PresetChannel { enabled: c.enabled, gain_db: c.gain_db, to_left: c.to_left, to_right: c.to_right })
            })
            .collect(),
        master_gain_db: mix.master_gain_db,
        gain_mode: mix.gain_mode,
    }
}

pub struct PresetStore {
    path: PathBuf,
}

impl PresetStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    fn load_user(&self) -> Result<Vec<Preset>> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }
        let text = std::fs::read_to_string(&self.path).with_context(|| format!("reading {}", self.path.display()))?;
        let mut v: Vec<Preset> = serde_json::from_str(&text).context("presets file is corrupt")?;
        v.iter_mut().for_each(|p| p.builtin = false);
        Ok(v)
    }

    fn write_user(&self, presets: &[Preset]) -> Result<()> {
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = self.path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_string_pretty(presets)?)?;
        std::fs::rename(&tmp, &self.path)?;
        Ok(())
    }

    pub fn all(&self) -> Result<Vec<Preset>> {
        let mut v = builtins();
        v.extend(self.load_user()?);
        Ok(v)
    }

    pub fn find(&self, name: &str) -> Result<Preset> {
        self.all()?
            .into_iter()
            .find(|p| p.name == name)
            .ok_or_else(|| anyhow::anyhow!("preset \"{name}\" not found"))
    }

    /// Saves (or overwrites) a user preset.
    pub fn save(&self, preset: Preset) -> Result<()> {
        if preset.name.is_empty() {
            bail!("preset name is empty");
        }
        if builtins().iter().any(|b| b.name.eq_ignore_ascii_case(&preset.name)) {
            bail!("\"{}\" is a built-in preset name; choose another", preset.name);
        }
        let mut user = self.load_user()?;
        user.retain(|p| p.name != preset.name);
        user.push(preset);
        self.write_user(&user)
    }

    pub fn delete(&self, name: &str) -> Result<()> {
        if builtins().iter().any(|b| b.name == name) {
            bail!("built-in presets cannot be deleted");
        }
        let mut user = self.load_user()?;
        user.retain(|p| p.name != name);
        self.write_user(&user)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn surround_preset_on_side_and_back_layouts() {
        let p = builtins().into_iter().find(|p| p.name == "Surround to Stereo").unwrap();
        for layout in [ids(&["FL", "FR", "FC", "LFE", "SL", "SR"]), ids(&["FL", "FR", "FC", "LFE", "BL", "BR"])] {
            let mix = apply(&p, &layout);
            let m = mix.raw_matrix(6);
            assert_eq!(m.left, vec![0.0, 0.0, 0.0, 0.0, 1.0, 0.0], "{layout:?}");
            assert_eq!(m.right, vec![0.0, 0.0, 0.0, 0.0, 0.0, 1.0], "{layout:?}");
        }
    }

    #[test]
    fn itu_preset_equals_defaults() {
        let p = builtins().into_iter().next().unwrap();
        let l = ids(&["FL", "FR", "FC", "LFE", "SL", "SR"]);
        assert_eq!(apply(&p, &l), crate::mix::default_mix(&l));
    }

    #[test]
    fn save_roundtrip_and_builtin_protection() {
        let dir = std::env::temp_dir().join(format!("cp-presets-{}", std::process::id()));
        let store = PresetStore::new(dir.join("presets.json"));
        let l = ids(&["FL", "FR", "FC", "LFE", "BL", "BR"]);
        let mut mix = crate::mix::default_mix(&l);
        mix.channels[4].gain_db = -3.0;
        store.save(from_mix("Mine", "", &mix)).unwrap();
        let loaded = store.find("Mine").unwrap();
        assert!(loaded.channels.contains_key("SL")); // stored by role
        assert_eq!(apply(&loaded, &l), MixSettings { gain_mode: mix.gain_mode, ..mix.clone() });
        assert!(store.save(from_mix("ITU Downmix", "", &mix)).is_err());
        assert!(store.delete("ITU Downmix").is_err());
        store.delete("Mine").unwrap();
        assert!(store.find("Mine").is_err());
        let _ = std::fs::remove_dir_all(dir);
    }
}
