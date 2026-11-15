//! ffprobe -> MediaInfo: codec, profile, channel count/layout, timing, and
//! whether the source is genuinely multichannel.

use crate::ffmpeg::{run_text, FfmpegInfo};
use crate::layout;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SourceKind {
    /// Three or more discrete decoded channels (5.1, 7.1, ...).
    Multichannel,
    Stereo,
    Mono,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelInfo {
    pub id: String,
    pub label: String,
    pub role: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioStream {
    /// Absolute stream index in the container (for `-map 0:<index>`).
    pub index: u32,
    /// Position among audio streams (track number shown to the user, 1-based).
    pub track: u32,
    pub codec_name: String,
    pub codec_label: String,
    pub profile: Option<String>,
    pub channels: u32,
    pub channel_layout: String,
    pub layout_assumed: bool,
    pub channel_info: Vec<ChannelInfo>,
    pub sample_rate: u32,
    pub start_time: f64,
    pub duration: f64,
    pub bit_rate: Option<u64>,
    pub language: Option<String>,
    pub title: Option<String>,
    pub is_default: bool,
    pub decoder_available: bool,
    pub kind: SourceKind,
    pub kind_label: String,
}

impl AudioStream {
    pub fn channel_ids(&self) -> Vec<String> {
        self.channel_info.iter().map(|c| c.id.clone()).collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoInfo {
    /// Absolute stream index in the container.
    pub index: u32,
    pub codec_name: String,
    pub width: u32,
    pub height: u32,
    /// Display aspect ratio (accounts for non-square pixels).
    pub aspect: f64,
    pub fps: f64,
    pub start_time: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaInfo {
    pub path: String,
    pub format_name: String,
    pub format_long_name: String,
    pub duration: f64,
    pub start_time: f64,
    pub has_video: bool,
    pub video: Option<VideoInfo>,
    pub audio_streams: Vec<AudioStream>,
}

#[derive(Deserialize)]
struct ProbeOut {
    #[serde(default)]
    streams: Vec<PStream>,
    format: Option<PFormat>,
}

#[derive(Deserialize)]
struct PStream {
    index: u32,
    codec_type: Option<String>,
    codec_name: Option<String>,
    codec_long_name: Option<String>,
    profile: Option<String>,
    channels: Option<u32>,
    channel_layout: Option<String>,
    sample_rate: Option<String>,
    start_time: Option<String>,
    duration: Option<String>,
    bit_rate: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    avg_frame_rate: Option<String>,
    r_frame_rate: Option<String>,
    sample_aspect_ratio: Option<String>,
    #[serde(default)]
    tags: HashMap<String, String>,
    #[serde(default)]
    disposition: HashMap<String, i64>,
}

#[derive(Deserialize)]
struct PFormat {
    format_name: Option<String>,
    format_long_name: Option<String>,
    duration: Option<String>,
    start_time: Option<String>,
}

fn num(s: &Option<String>) -> Option<f64> {
    s.as_deref().and_then(|v| v.trim().parse().ok())
}

/// Case-insensitive tag lookup that also matches "DURATION-eng"-style keys.
fn tag<'a>(tags: &'a HashMap<String, String>, key: &str) -> Option<&'a str> {
    tags.iter()
        .find(|(k, _)| {
            let k = k.to_ascii_lowercase();
            k == key || k.starts_with(&format!("{key}-"))
        })
        .map(|(_, v)| v.as_str())
}

/// "24000/1001" or "16:9" -> number (None for 0/0 or junk).
fn ratio(s: Option<&str>) -> Option<f64> {
    let s = s?;
    let (a, b) = s.split_once(['/', ':'])?;
    let (a, b): (f64, f64) = (a.trim().parse().ok()?, b.trim().parse().ok()?);
    (a > 0.0 && b > 0.0).then_some(a / b)
}

/// "01:23:45.678000000" -> seconds.
fn parse_hms(s: &str) -> Option<f64> {
    let mut total = 0.0;
    for part in s.trim().split(':') {
        total = total * 60.0 + part.parse::<f64>().ok()?;
    }
    Some(total)
}

/// ffmpeg decoder names that handle a codec.
pub fn decoder_names(codec: &str) -> Vec<&str> {
    match codec {
        "dts" => vec!["dca"],
        "aac" => vec!["aac", "aac_fixed"],
        "ac3" => vec!["ac3", "ac3_fixed"],
        other => vec![other],
    }
}

pub fn codec_label(codec: &str, profile: Option<&str>, long_name: Option<&str>) -> String {
    let p = profile.unwrap_or_default();
    match codec {
        "dts" => match p {
            "" | "DTS" => "DTS".into(),
            "DTS-HD MA" => "DTS-HD Master Audio".into(),
            "DTS-HD HRA" => "DTS-HD High Resolution".into(),
            "DTS-HD MA + DTS:X" | "DTS-HD MA + DTS:X IMAX" => "DTS-HD Master Audio (DTS:X)".into(),
            "DTS-ES" => "DTS-ES".into(),
            "DTS 96/24" => "DTS 96/24".into(),
            other => format!("DTS ({other})"),
        },
        "ac3" => "Dolby Digital (AC-3)".into(),
        "eac3" if p.contains("Atmos") => "Dolby Digital Plus (E-AC-3, Atmos bed)".into(),
        "eac3" => "Dolby Digital Plus (E-AC-3)".into(),
        "truehd" if p.contains("Atmos") => "Dolby TrueHD (Atmos bed)".into(),
        "truehd" => "Dolby TrueHD".into(),
        "aac" if !p.is_empty() => format!("AAC ({p})"),
        "aac" => "AAC".into(),
        "flac" => "FLAC".into(),
        c if c.starts_with("pcm_") => format!("PCM ({})", &c[4..]),
        _ => long_name.unwrap_or(codec).to_string(),
    }
}

fn kind_for(channels: u32) -> SourceKind {
    match channels {
        0 | 1 => SourceKind::Mono,
        2 => SourceKind::Stereo,
        _ => SourceKind::Multichannel,
    }
}

fn kind_label(kind: SourceKind, layout: &layout::Layout, channels: u32) -> String {
    match kind {
        SourceKind::Multichannel => {
            let assumed = if layout.assumed { ", layout assumed" } else { "" };
            format!("Native {} multichannel — {channels} discrete channels{assumed}", layout.name)
        }
        SourceKind::Stereo => "Stereo source (2.0) — no discrete surround channels".into(),
        SourceKind::Mono => "Mono source — single channel".into(),
    }
}

pub fn probe(ff: &FfmpegInfo, path: &str) -> Result<MediaInfo> {
    let text = run_text(
        &ff.ffprobe,
        &["-v", "error", "-show_streams", "-show_format", "-of", "json", path],
    )
    .with_context(|| format!("could not read {path}"))?;
    parse(ff, path, &text)
}

pub fn parse(ff: &FfmpegInfo, path: &str, json: &str) -> Result<MediaInfo> {
    let out: ProbeOut = serde_json::from_str(json).context("unexpected ffprobe output")?;
    let format = out.format.unwrap_or(PFormat { format_name: None, format_long_name: None, duration: None, start_time: None });
    let fmt_duration = num(&format.duration).unwrap_or(0.0);
    let fmt_start = num(&format.start_time).unwrap_or(0.0);

    let is_real_video = |s: &&PStream| {
        s.codec_type.as_deref() == Some("video") && s.disposition.get("attached_pic").copied().unwrap_or(0) == 0
    };
    let has_video = out.streams.iter().any(|s| is_real_video(&s));
    let video = out
        .streams
        .iter()
        .filter(is_real_video)
        .max_by_key(|s| (s.disposition.get("default").copied().unwrap_or(0), s.width.unwrap_or(0)))
        .map(|s| {
            let (w, h) = (s.width.unwrap_or(0), s.height.unwrap_or(0));
            let sar = ratio(s.sample_aspect_ratio.as_deref()).unwrap_or(1.0);
            VideoInfo {
                index: s.index,
                codec_name: s.codec_name.clone().unwrap_or_default(),
                width: w,
                height: h,
                aspect: if h > 0 { w as f64 * sar / h as f64 } else { 16.0 / 9.0 },
                fps: ratio(s.avg_frame_rate.as_deref()).or_else(|| ratio(s.r_frame_rate.as_deref())).unwrap_or(24.0),
                start_time: num(&s.start_time).unwrap_or(fmt_start),
            }
        });

    let mut audio_streams = Vec::new();
    for s in out.streams.iter().filter(|s| s.codec_type.as_deref() == Some("audio")) {
        let channels = s.channels.unwrap_or(0);
        let codec = s.codec_name.clone().unwrap_or_else(|| "unknown".into());
        let lay = layout::resolve(s.channel_layout.as_deref(), channels);
        let kind = kind_for(channels);
        let duration = num(&s.duration)
            .or_else(|| tag(&s.tags, "duration").and_then(parse_hms))
            .unwrap_or(fmt_duration);
        let channel_info = lay
            .ids
            .iter()
            .map(|id| ChannelInfo { id: id.clone(), label: layout::label(id, &lay.ids), role: layout::role(id, &lay.ids) })
            .collect();
        audio_streams.push(AudioStream {
            index: s.index,
            track: audio_streams.len() as u32 + 1,
            codec_label: codec_label(&codec, s.profile.as_deref(), s.codec_long_name.as_deref()),
            decoder_available: decoder_names(&codec).iter().any(|d| ff.has_decoder(d)),
            codec_name: codec,
            profile: s.profile.clone(),
            channels,
            kind_label: kind_label(kind, &lay, channels),
            channel_layout: lay.name.clone(),
            layout_assumed: lay.assumed,
            channel_info,
            sample_rate: s.sample_rate.as_deref().and_then(|v| v.parse().ok()).unwrap_or(48_000),
            start_time: num(&s.start_time).unwrap_or(fmt_start),
            duration,
            bit_rate: s.bit_rate.as_deref().and_then(|v| v.parse().ok()),
            language: tag(&s.tags, "language").map(str::to_string),
            title: tag(&s.tags, "title").map(str::to_string),
            is_default: s.disposition.get("default").copied().unwrap_or(0) == 1,
            kind,
        });
    }

    Ok(MediaInfo {
        path: path.to_string(),
        format_name: format.format_name.unwrap_or_default(),
        format_long_name: format.format_long_name.unwrap_or_default(),
        duration: if fmt_duration > 0.0 {
            fmt_duration
        } else {
            audio_streams.iter().map(|a| a.duration).fold(0.0, f64::max)
        },
        start_time: fmt_start,
        has_video,
        video,
        audio_streams,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;
    use std::path::PathBuf;

    fn ff() -> FfmpegInfo {
        FfmpegInfo {
            ffmpeg: PathBuf::new(),
            ffprobe: PathBuf::new(),
            version: String::new(),
            decoders: ["dca", "ac3"].iter().map(|s| s.to_string()).collect::<BTreeSet<_>>(),
            encoders: BTreeSet::new(),
        }
    }

    #[test]
    fn parses_mkv_with_dts_and_stereo_tracks() {
        let json = r#"{
          "streams": [
            {"index":0,"codec_type":"video","codec_name":"h264","width":1440,"height":1080,
             "sample_aspect_ratio":"4:3","avg_frame_rate":"24000/1001"},
            {"index":1,"codec_type":"audio","codec_name":"dts","profile":"DTS-HD MA","channels":6,
             "channel_layout":"5.1(side)","sample_rate":"48000","start_time":"0.000",
             "tags":{"language":"eng","DURATION-eng":"00:01:30.500000000"},"disposition":{"default":1}},
            {"index":2,"codec_type":"audio","codec_name":"aac","profile":"LC","channels":2,
             "channel_layout":"stereo","sample_rate":"48000"}
          ],
          "format": {"format_name":"matroska,webm","duration":"90.5","start_time":"0.000"}
        }"#;
        let info = parse(&ff(), "x.mkv", json).unwrap();
        assert!(info.has_video);
        let v = info.video.as_ref().unwrap();
        assert!((v.fps - 23.976).abs() < 0.001);
        assert!((v.aspect - 16.0 / 9.0).abs() < 1e-6); // anamorphic 1440x1080 @ 4:3 SAR
        let dts = &info.audio_streams[0];
        assert_eq!(dts.kind, SourceKind::Multichannel);
        assert_eq!(dts.codec_label, "DTS-HD Master Audio");
        assert!(dts.decoder_available);
        assert!((dts.duration - 90.5).abs() < 1e-6);
        assert_eq!(dts.language.as_deref(), Some("eng"));
        assert!(dts.kind_label.starts_with("Native 5.1(side)"));
        let st = &info.audio_streams[1];
        assert_eq!(st.kind, SourceKind::Stereo);
        assert!(!st.decoder_available);
        assert_eq!(st.track, 2);
    }
}
