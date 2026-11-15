//! Export: ffmpeg decode -> mix (same engine as preview) -> ffmpeg encode.
//! Optional peak-normalization pre-pass, sync padding, and a post-export check.

use crate::ffmpeg::{command, drain_stderr, run_text, FfmpegInfo, PcmDecoder};
use crate::mix::{db_to_lin, lin_to_db, MixSettings, Peaks};
use crate::probe::{self, AudioStream, MediaInfo};
use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};

/// Label for every file this app writes. Output is always plain two-channel
/// stereo; it is never DTS, Dolby or 5.1, whatever the source was.
pub const OUTPUT_FORMAT_LABEL: &str = "Stereo 2.0 (L/R)";

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum OutputFormat {
    /// bits: 16, 24 or 32 (32 = float)
    Wav { bits: u32 },
    /// bits: 16 or 24
    Flac { bits: u32 },
    /// vbr: true = LAME V0, false = CBR 320 kbps
    Mp3 { vbr: bool },
    Aac { kbps: u32 },
}

impl OutputFormat {
    pub fn extension(&self) -> &'static str {
        match self {
            OutputFormat::Wav { .. } => "wav",
            OutputFormat::Flac { .. } => "flac",
            OutputFormat::Mp3 { .. } => "mp3",
            OutputFormat::Aac { .. } => "m4a",
        }
    }
    fn is_lossy(&self) -> bool {
        matches!(self, OutputFormat::Mp3 { .. } | OutputFormat::Aac { .. })
    }
    fn max_rate(&self) -> u32 {
        match self {
            OutputFormat::Mp3 { .. } => 48_000,
            OutputFormat::Aac { .. } => 96_000,
            _ => 384_000,
        }
    }
    fn needs_dither(&self) -> bool {
        matches!(self, OutputFormat::Wav { bits: 16 } | OutputFormat::Flac { bits: 16 })
    }
    fn codec_args(&self) -> Result<Vec<String>> {
        let v: Vec<&str> = match *self {
            OutputFormat::Wav { bits: 16 } => vec!["-c:a", "pcm_s16le"],
            OutputFormat::Wav { bits: 24 } => vec!["-c:a", "pcm_s24le"],
            OutputFormat::Wav { bits: 32 } => vec!["-c:a", "pcm_f32le"],
            OutputFormat::Flac { bits: 16 } => vec!["-c:a", "flac", "-sample_fmt", "s16", "-compression_level", "5"],
            OutputFormat::Flac { bits: 24 } => {
                vec!["-c:a", "flac", "-sample_fmt", "s32", "-bits_per_raw_sample", "24", "-compression_level", "5"]
            }
            OutputFormat::Mp3 { vbr: true } => vec!["-c:a", "libmp3lame", "-q:a", "0"],
            OutputFormat::Mp3 { vbr: false } => vec!["-c:a", "libmp3lame", "-b:a", "320k"],
            OutputFormat::Aac { kbps } => {
                return Ok(vec!["-c:a".into(), "aac".into(), "-b:a".into(), format!("{}k", kbps.clamp(96, 512)), "-movflags".into(), "+faststart".into()])
            }
            other => bail!("unsupported output format {other:?}"),
        };
        Ok(v.into_iter().map(String::from).collect())
    }
    fn encoder_name(&self) -> &'static str {
        match self {
            OutputFormat::Wav { bits: 24 } => "pcm_s24le",
            OutputFormat::Wav { bits: 32 } => "pcm_f32le",
            OutputFormat::Wav { .. } => "pcm_s16le",
            OutputFormat::Flac { .. } => "flac",
            OutputFormat::Mp3 { .. } => "libmp3lame",
            OutputFormat::Aac { .. } => "aac",
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportRequest {
    pub input: String,
    pub stream_index: u32,
    pub output: String,
    pub format: OutputFormat,
    /// None = keep the source sample rate.
    pub sample_rate: Option<u32>,
    /// Peak-normalize to this dBFS level (e.g. -1.0). None = off.
    pub normalize_dbfs: Option<f32>,
    /// Pad with silence so the audio lines up with the container start (video sync).
    pub align_to_container: bool,
    pub mix: MixSettings,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportProgress {
    /// "analyzing" (normalization pre-pass) or "rendering"
    pub phase: &'static str,
    pub fraction: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Verification {
    pub ok: bool,
    pub channels: u32,
    pub channel_layout: String,
    pub codec: String,
    pub sample_rate: u32,
    pub duration: f64,
    pub expected_duration: f64,
    pub source_duration: f64,
    pub duration_diff_ms: f64,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportResult {
    pub output: String,
    pub format_label: String,
    pub description: String,
    pub frames_written: u64,
    pub padding_seconds: f64,
    pub applied_gain_db: f32,
    pub peak_dbfs: f32,
    pub clipped_samples: u64,
    pub verification: Verification,
}

/// Human-readable description written to file metadata and shown in the UI.
pub fn describe_output(src: &AudioStream, mix: &MixSettings) -> String {
    let routes = mix.routes();
    let routes = if routes.is_empty() { "silence (no channels selected)".to_string() } else { routes.join(", ") };
    format!(
        "{OUTPUT_FORMAT_LABEL} mix, downmixed from a {} {} source track. Channels used: {routes}.",
        src.channel_layout, src.codec_label
    )
}

fn find_stream<'a>(info: &'a MediaInfo, index: u32) -> Result<&'a AudioStream> {
    info.audio_streams
        .iter()
        .find(|s| s.index == index)
        .ok_or_else(|| anyhow!("audio stream {index} not found"))
}

/// Runs decode+mix over the whole stream and returns the output peak (linear).
fn analyze_peak(
    ff: &FfmpegInfo,
    req: &ExportRequest,
    src: &AudioStream,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(ExportProgress),
) -> Result<f32> {
    let n = src.channels as usize;
    let matrix = req.mix.matrix(n);
    let total = (src.duration * src.sample_rate as f64).max(1.0);
    let mut dec = PcmDecoder::spawn(&ff.ffmpeg, &req.input, req.stream_index, 0.0, n)?;
    let (mut input, mut mixed) = (Vec::new(), Vec::new());
    let mut peaks = Peaks::new(0);
    let mut done = 0u64;
    let mut last_report = 0u64;
    while dec.read_chunk(&mut input)? {
        if cancel.load(Ordering::SeqCst) {
            bail!("cancelled");
        }
        mixed.clear();
        matrix.mix(&input, &mut mixed);
        peaks.track_output(&mixed);
        done += mixed.len() as u64;
        if done - last_report > src.sample_rate as u64 / 2 {
            last_report = done;
            progress(ExportProgress { phase: "analyzing", fraction: (done as f64 / total).min(1.0) });
        }
    }
    dec.finish()?;
    Ok(peaks.output[0].max(peaks.output[1]))
}

pub fn run(
    ff: &FfmpegInfo,
    req: &ExportRequest,
    cancel: &AtomicBool,
    mut progress: impl FnMut(ExportProgress),
) -> Result<ExportResult> {
    let info = probe::probe(ff, &req.input)?;
    let src = find_stream(&info, req.stream_index)?.clone();
    if !src.decoder_available {
        bail!("this ffmpeg build cannot decode {} ({})", src.codec_label, src.codec_name);
    }
    if !ff.has_encoder(req.format.encoder_name()) {
        bail!("this ffmpeg build has no {} encoder", req.format.encoder_name());
    }
    let n = src.channels as usize;
    if req.mix.channels.len() != n {
        bail!("mix has {} channels but the stream has {n}", req.mix.channels.len());
    }

    // Gain staging: matrix (with Safe/Manual gain) times optional normalization.
    let mut gain = 1.0f32;
    if let Some(target) = req.normalize_dbfs {
        let peak = analyze_peak(ff, req, &src, cancel, &mut progress)?;
        if peak > 1e-6 {
            gain = db_to_lin(target.min(0.0)) / peak;
        }
    }
    let matrix = req.mix.matrix(n).scaled(gain);
    let applied_gain_db = lin_to_db(req.mix.output_gain() * gain);

    let src_rate = src.sample_rate;
    let out_rate = req.sample_rate.unwrap_or(src_rate).min(req.format.max_rate());
    let pad_seconds = if req.align_to_container { (src.start_time - info.start_time).max(0.0) } else { 0.0 };
    let pad_frames = (pad_seconds * src_rate as f64).round() as u64;
    let description = describe_output(&src, &req.mix);

    let mut af = Vec::new();
    if out_rate != src_rate || req.format.needs_dither() {
        let mut f = format!("aresample={out_rate}");
        if req.format.needs_dither() {
            f.push_str(":dither_method=triangular");
        }
        af.push(f);
    }

    let mut cmd = command(&ff.ffmpeg);
    cmd.args(["-hide_banner", "-loglevel", "error", "-y"])
        .args(["-f", "f32le", "-ar", &src_rate.to_string(), "-ch_layout", "stereo", "-i", "pipe:0"]);
    if !af.is_empty() {
        cmd.args(["-af", &af.join(",")]);
    }
    cmd.args(["-ac", "2", "-ar", &out_rate.to_string()])
        .args(req.format.codec_args()?)
        .args(["-map_metadata", "-1"])
        .args(["-metadata", &format!("comment={description}")])
        .args(["-metadata", "encoded_by=Channel Picker"])
        .arg(&req.output)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    let mut enc = cmd.spawn().context("failed to start ffmpeg encoder")?;
    let enc_err = drain_stderr(&mut enc);
    let mut stdin = enc.stdin.take().ok_or_else(|| anyhow!("no encoder stdin"))?;

    let cleanup = |enc: &mut std::process::Child| {
        let _ = enc.kill();
        let _ = enc.wait();
        let _ = std::fs::remove_file(&req.output);
    };

    let total = (src.duration * src_rate as f64).max(1.0) + pad_frames as f64;
    let mut written = 0u64;
    let mut peaks = Peaks::new(0);
    let mut bytes: Vec<u8> = Vec::new();
    let mut write_frames = |frames: &[[f32; 2]], stdin: &mut std::process::ChildStdin| -> std::io::Result<()> {
        bytes.clear();
        for f in frames {
            bytes.extend_from_slice(&f[0].to_le_bytes());
            bytes.extend_from_slice(&f[1].to_le_bytes());
        }
        stdin.write_all(&bytes)
    };

    let result: Result<()> = (|| {
        // Sync padding.
        let silence = vec![[0.0f32; 2]; 4096];
        let mut left = pad_frames;
        while left > 0 {
            let k = left.min(silence.len() as u64) as usize;
            write_frames(&silence[..k], &mut stdin)?;
            left -= k as u64;
            written += k as u64;
        }

        let mut dec = PcmDecoder::spawn(&ff.ffmpeg, &req.input, req.stream_index, 0.0, n)?;
        let (mut input, mut mixed) = (Vec::new(), Vec::new());
        let mut last_report = 0u64;
        while dec.read_chunk(&mut input)? {
            if cancel.load(Ordering::SeqCst) {
                bail!("cancelled");
            }
            mixed.clear();
            matrix.mix(&input, &mut mixed);
            peaks.track_output(&mixed);
            write_frames(&mixed, &mut stdin)?;
            written += mixed.len() as u64;
            if written - last_report > src_rate as u64 / 2 {
                last_report = written;
                progress(ExportProgress { phase: "rendering", fraction: (written as f64 / total).min(1.0) });
            }
        }
        dec.finish()?;
        Ok(())
    })();

    if let Err(e) = result {
        cleanup(&mut enc);
        let detail = enc_err.and_then(|h| h.join().ok()).unwrap_or_default();
        if detail.trim().is_empty() || e.to_string() == "cancelled" {
            return Err(e);
        }
        return Err(anyhow!("{e} — encoder: {}", detail.trim()));
    }
    drop(stdin);
    let status = enc.wait()?;
    let detail = enc_err.and_then(|h| h.join().ok()).unwrap_or_default();
    if !status.success() {
        let _ = std::fs::remove_file(&req.output);
        bail!("encoder failed: {}", detail.trim());
    }
    progress(ExportProgress { phase: "rendering", fraction: 1.0 });

    let expected = written as f64 / src_rate as f64;
    let verification = verify(ff, &req.output, expected, src.duration, pad_seconds, &req.format)?;
    Ok(ExportResult {
        output: req.output.clone(),
        format_label: OUTPUT_FORMAT_LABEL.into(),
        description,
        frames_written: written,
        padding_seconds: pad_frames as f64 / src_rate as f64,
        applied_gain_db,
        peak_dbfs: lin_to_db(peaks.output[0].max(peaks.output[1])),
        clipped_samples: peaks.clipped,
        verification,
    })
}

/// Re-probes the written file: two channels, stereo layout, duration close to the source.
/// `source_duration` is the stream duration the container reports; `pad` is the
/// sync lead-in that was added. Some containers (MKV's DURATION tag) already
/// count the stream's start offset, so either reading may match.
pub fn verify(
    ff: &FfmpegInfo,
    path: &str,
    expected: f64,
    source_duration: f64,
    pad: f64,
    fmt: &OutputFormat,
) -> Result<Verification> {
    let json = run_text(&ff.ffprobe, &["-v", "error", "-show_streams", "-show_format", "-of", "json", path])?;
    let v: serde_json::Value = serde_json::from_str(&json)?;
    let s = v["streams"].get(0).ok_or_else(|| anyhow!("exported file has no audio stream"))?;
    let parse = |x: &serde_json::Value| x.as_str().and_then(|t| t.parse::<f64>().ok());
    let duration = parse(&s["duration"]).or_else(|| parse(&v["format"]["duration"])).unwrap_or(0.0);
    let channels = s["channels"].as_u64().unwrap_or(0) as u32;
    let channel_layout = s["channel_layout"].as_str().unwrap_or("").to_string();
    let codec = s["codec_name"].as_str().unwrap_or("").to_string();
    let sample_rate = s["sample_rate"].as_str().and_then(|t| t.parse().ok()).unwrap_or(0);
    let diff_ms = (duration - expected) * 1000.0;
    // Lossy encoders pad to whole frames (MP3 1152, AAC 1024 samples) + priming.
    let tolerance_ms = if fmt.is_lossy() { 80.0 } else { 2.0 };
    let mut notes = Vec::new();
    if channels != 2 {
        notes.push(format!("expected 2 channels, found {channels}"));
    }
    if diff_ms.abs() > tolerance_ms {
        notes.push(format!("duration differs from the mixed audio by {diff_ms:.1} ms"));
    } else if fmt.is_lossy() && diff_ms.abs() > 2.0 {
        notes.push(format!("{diff_ms:+.1} ms from encoder frame padding/priming (normal for lossy formats)"));
    }
    let matches_source = (expected - (source_duration + pad)).abs() <= 0.05 || (expected - source_duration).abs() <= 0.05;
    if !matches_source {
        notes.push(format!(
            "decoded length {expected:.3}s vs container-reported {source_duration:.3}s (container metadata can be approximate)"
        ));
    }
    Ok(Verification {
        ok: channels == 2 && diff_ms.abs() <= tolerance_ms,
        channels,
        channel_layout,
        codec,
        sample_rate,
        duration,
        expected_duration: expected,
        source_duration,
        duration_diff_ms: diff_ms,
        notes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::probe::{ChannelInfo, SourceKind};

    fn dts51() -> AudioStream {
        let ids = ["FL", "FR", "FC", "LFE", "SL", "SR"];
        AudioStream {
            index: 1,
            track: 1,
            codec_name: "dts".into(),
            codec_label: "DTS".into(),
            profile: Some("DTS".into()),
            channels: 6,
            channel_layout: "5.1(side)".into(),
            layout_assumed: false,
            channel_info: ids.iter().map(|i| ChannelInfo { id: i.to_string(), label: i.to_string(), role: i.to_string() }).collect(),
            sample_rate: 48_000,
            start_time: 0.0,
            duration: 10.0,
            bit_rate: None,
            language: None,
            title: None,
            is_default: true,
            decoder_available: true,
            kind: SourceKind::Multichannel,
            kind_label: String::new(),
        }
    }

    #[test]
    fn description_never_claims_dts_or_51_output() {
        let src = dts51();
        let mix = crate::mix::default_mix(&src.channel_ids());
        let d = describe_output(&src, &mix);
        assert!(d.starts_with("Stereo 2.0"), "{d}");
        assert!(!d.contains("DTS 5.1"), "{d}");
        assert!(!OUTPUT_FORMAT_LABEL.contains("DTS") && !OUTPUT_FORMAT_LABEL.contains("5.1"));
    }

    #[test]
    fn format_serde_shape() {
        let f: OutputFormat = serde_json::from_str(r#"{"kind":"flac","bits":24}"#).unwrap();
        assert_eq!(f, OutputFormat::Flac { bits: 24 });
        assert_eq!(f.extension(), "flac");
    }
}
