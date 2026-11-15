//! The channel-selection / downmix model. Both preview and export use this
//! module, so what you hear is exactly what gets written.

use crate::layout::{self, Side};
use serde::{Deserialize, Serialize};

pub const MINUS_3DB: f32 = std::f32::consts::FRAC_1_SQRT_2;
/// Default LFE send when the user enables it (ITU leaves LFE out entirely).
pub const LFE_SEND: f32 = 0.5;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelStrip {
    pub id: String,
    /// false = muted / not selected.
    pub enabled: bool,
    #[serde(default)]
    pub solo: bool,
    pub gain_db: f32,
    /// Downmix coefficient into stereo Left (linear, 0..=1 typical).
    pub to_left: f32,
    /// Downmix coefficient into stereo Right.
    pub to_right: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GainMode {
    /// Scale down automatically so even full-scale input on every selected channel cannot clip.
    Safe,
    /// Use master gain only; the user is responsible for headroom.
    Manual,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MixSettings {
    pub channels: Vec<ChannelStrip>,
    pub master_gain_db: f32,
    pub gain_mode: GainMode,
}

pub fn db_to_lin(db: f32) -> f32 {
    if db <= -120.0 {
        0.0
    } else {
        10f32.powf(db / 20.0)
    }
}

pub fn lin_to_db(x: f32) -> f32 {
    if x <= 1e-9 {
        -180.0
    } else {
        20.0 * x.log10()
    }
}

/// ITU-R BS.775 style Lo/Ro defaults for one channel: (enabled, to_left, to_right).
pub fn default_routing(id: &str) -> (bool, f32, f32) {
    match id {
        "FL" => (true, 1.0, 0.0),
        "FR" => (true, 0.0, 1.0),
        "FC" => (true, MINUS_3DB, MINUS_3DB),
        "LFE" => (false, LFE_SEND, LFE_SEND),
        _ => match layout::side(id) {
            Side::Left => (true, MINUS_3DB, 0.0),
            Side::Right => (true, 0.0, MINUS_3DB),
            Side::Center => (true, 0.5, 0.5),
        },
    }
}

pub fn default_strip(id: &str) -> ChannelStrip {
    let (enabled, l, r) = default_routing(id);
    ChannelStrip { id: id.into(), enabled, solo: false, gain_db: 0.0, to_left: l, to_right: r }
}

pub fn default_mix(ids: &[String]) -> MixSettings {
    // A mono source goes to both sides at full level.
    let channels = if ids.len() == 1 {
        vec![ChannelStrip { id: ids[0].clone(), enabled: true, solo: false, gain_db: 0.0, to_left: 1.0, to_right: 1.0 }]
    } else {
        ids.iter().map(|id| default_strip(id)).collect()
    };
    MixSettings { channels, master_gain_db: 0.0, gain_mode: GainMode::Safe }
}

/// 2xN downmix matrix applied to interleaved N-channel frames.
#[derive(Debug, Clone, PartialEq)]
pub struct Matrix {
    pub left: Vec<f32>,
    pub right: Vec<f32>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GainReport {
    /// Sum of |coefficients| feeding each output (1.0 = can just reach full scale).
    pub worst_case_left: f32,
    pub worst_case_right: f32,
    /// Attenuation added by Safe mode (0 when not needed or in Manual mode).
    pub safe_gain_db: f32,
    /// Total gain applied after the matrix (master + safe).
    pub output_gain_db: f32,
    /// True when full-scale input could clip with the current settings.
    pub can_clip: bool,
    pub routes: Vec<String>,
}

impl MixSettings {
    fn any_solo(&self) -> bool {
        self.channels.iter().any(|c| c.solo)
    }

    /// Whether a channel is heard, taking solo into account.
    pub fn is_active(&self, c: &ChannelStrip) -> bool {
        if self.any_solo() {
            c.solo
        } else {
            c.enabled
        }
    }

    /// Per-channel coefficients including channel gain, before master/safe gain.
    pub fn raw_matrix(&self, n: usize) -> Matrix {
        let mut m = Matrix { left: vec![0.0; n], right: vec![0.0; n] };
        for (i, c) in self.channels.iter().enumerate().take(n) {
            if self.is_active(c) {
                let g = db_to_lin(c.gain_db);
                m.left[i] = c.to_left * g;
                m.right[i] = c.to_right * g;
            }
        }
        m
    }

    fn worst_case(&self) -> (f32, f32) {
        let m = self.raw_matrix(self.channels.len());
        (m.left.iter().map(|x| x.abs()).sum(), m.right.iter().map(|x| x.abs()).sum())
    }

    /// Linear attenuation Safe mode applies (1.0 = none).
    pub fn safe_scale(&self) -> f32 {
        if self.gain_mode != GainMode::Safe {
            return 1.0;
        }
        let (l, r) = self.worst_case();
        let w = l.max(r);
        if w > 1.0 {
            1.0 / w
        } else {
            1.0
        }
    }

    pub fn output_gain(&self) -> f32 {
        db_to_lin(self.master_gain_db) * self.safe_scale()
    }

    /// Final matrix for an N-channel stream (extra/missing strips are treated as silent).
    pub fn matrix(&self, n: usize) -> Matrix {
        self.raw_matrix(n).scaled(self.output_gain())
    }

    pub fn routes(&self) -> Vec<String> {
        let mut out = Vec::new();
        for c in self.channels.iter().filter(|c| self.is_active(c)) {
            let g = db_to_lin(c.gain_db);
            let (l, r) = (c.to_left * g, c.to_right * g);
            match (l.abs() > 1e-6, r.abs() > 1e-6) {
                (true, true) => out.push(format!("{}→L+R", c.id)),
                (true, false) => out.push(format!("{}→L", c.id)),
                (false, true) => out.push(format!("{}→R", c.id)),
                _ => {}
            }
        }
        out
    }

    pub fn report(&self) -> GainReport {
        let (l, r) = self.worst_case();
        let out_gain = self.output_gain();
        GainReport {
            worst_case_left: l,
            worst_case_right: r,
            safe_gain_db: lin_to_db(self.safe_scale()).max(-180.0),
            output_gain_db: lin_to_db(out_gain),
            can_clip: l.max(r) * out_gain > 1.0 + 1e-4,
            routes: self.routes(),
        }
    }
}

impl Matrix {
    pub fn channels(&self) -> usize {
        self.left.len()
    }

    pub fn scaled(mut self, g: f32) -> Self {
        self.left.iter_mut().for_each(|x| *x *= g);
        self.right.iter_mut().for_each(|x| *x *= g);
        self
    }

    /// Mixes interleaved N-channel `input` into stereo frames appended to `out`.
    pub fn mix(&self, input: &[f32], out: &mut Vec<[f32; 2]>) {
        let n = self.channels();
        out.reserve(input.len() / n.max(1));
        for frame in input.chunks_exact(n) {
            let mut l = 0.0;
            let mut r = 0.0;
            for i in 0..n {
                l += frame[i] * self.left[i];
                r += frame[i] * self.right[i];
            }
            out.push([l, r]);
        }
    }
}

/// Peak tracking for meters and export statistics.
#[derive(Debug, Clone, Default)]
pub struct Peaks {
    pub input: Vec<f32>,
    pub output: [f32; 2],
    pub clipped: u64,
}

impl Peaks {
    pub fn new(n: usize) -> Self {
        Self { input: vec![0.0; n], output: [0.0; 2], clipped: 0 }
    }

    pub fn track_input(&mut self, input: &[f32]) {
        let n = self.input.len();
        if n == 0 {
            return;
        }
        for frame in input.chunks_exact(n) {
            for (p, x) in self.input.iter_mut().zip(frame) {
                *p = p.max(x.abs());
            }
        }
    }

    pub fn track_output(&mut self, out: &[[f32; 2]]) {
        for f in out {
            for c in 0..2 {
                let a = f[c].abs();
                self.output[c] = self.output[c].max(a);
                if a > 1.0 {
                    self.clipped += 1;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids51() -> Vec<String> {
        ["FL", "FR", "FC", "LFE", "SL", "SR"].iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn db_conversions() {
        assert!((db_to_lin(0.0) - 1.0).abs() < 1e-6);
        assert!((db_to_lin(-6.0206) - 0.5).abs() < 1e-4);
        assert!((lin_to_db(0.5) + 6.0206).abs() < 1e-3);
        assert_eq!(db_to_lin(-150.0), 0.0);
    }

    #[test]
    fn itu_default_matrix() {
        let m = default_mix(&ids51()).raw_matrix(6);
        assert_eq!(m.left, vec![1.0, 0.0, MINUS_3DB, 0.0, MINUS_3DB, 0.0]);
        assert_eq!(m.right, vec![0.0, 1.0, MINUS_3DB, 0.0, 0.0, MINUS_3DB]);
    }

    #[test]
    fn surround_only_routes_sl_sr() {
        let mut mix = default_mix(&ids51());
        for c in mix.channels.iter_mut() {
            c.enabled = matches!(c.id.as_str(), "SL" | "SR");
            if c.id == "SL" {
                (c.to_left, c.to_right) = (1.0, 0.0);
            }
            if c.id == "SR" {
                (c.to_left, c.to_right) = (0.0, 1.0);
            }
        }
        let m = mix.matrix(6);
        // One frame with a distinct value on every channel.
        let frame = [0.1, 0.2, 0.3, 0.4, 0.5, 0.6];
        let mut out = Vec::new();
        m.mix(&frame, &mut out);
        assert_eq!(out, vec![[0.5, 0.6]]);
        assert_eq!(mix.routes(), vec!["SL→L", "SR→R"]);
        assert_eq!(mix.safe_scale(), 1.0);
    }

    #[test]
    fn safe_gain_prevents_clipping_at_full_scale() {
        let mut mix = default_mix(&ids51());
        mix.channels.iter_mut().for_each(|c| c.enabled = true); // include LFE too
        let m = mix.matrix(6);
        let mut out = Vec::new();
        m.mix(&[1.0; 6], &mut out);
        m.mix(&[-1.0; 6], &mut out);
        for f in &out {
            assert!(f[0].abs() <= 1.0 + 1e-6 && f[1].abs() <= 1.0 + 1e-6, "{f:?}");
        }
        assert!(!mix.report().can_clip);

        mix.gain_mode = GainMode::Manual;
        assert!(mix.report().can_clip);
    }

    #[test]
    fn solo_overrides_enable() {
        let mut mix = default_mix(&ids51());
        mix.channels[2].solo = true;
        let m = mix.raw_matrix(6);
        assert_eq!(m.left, vec![0.0, 0.0, MINUS_3DB, 0.0, 0.0, 0.0]);
    }

    #[test]
    fn channel_gain_applies() {
        let mut mix = default_mix(&ids51());
        mix.channels[0].gain_db = -6.0206;
        assert!((mix.raw_matrix(6).left[0] - 0.5).abs() < 1e-4);
    }
}
