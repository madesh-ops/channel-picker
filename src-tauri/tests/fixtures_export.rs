//! End-to-end checks against real encoded files. Requires ffmpeg/ffprobe on
//! PATH (or CHANNEL_PICKER_FFMPEG_DIR). Fixtures are generated into
//! target/fixtures: each 5.1 channel carries its own sine tone.

use channel_picker_lib::export::{self, ExportRequest, OutputFormat};
use channel_picker_lib::ffmpeg::{self, FfmpegInfo};
use channel_picker_lib::mix::{GainMode, MixSettings};
use channel_picker_lib::presets;
use channel_picker_lib::probe::{self, SourceKind};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

const TONES: [(&str, f64); 6] = [("FL", 300.0), ("FR", 500.0), ("FC", 700.0), ("LFE", 60.0), ("SL", 1100.0), ("SR", 1300.0)];
const SECONDS: f64 = 6.0;

fn ff() -> FfmpegInfo {
    ffmpeg::locate(None).expect("ffmpeg + ffprobe must be installed for integration tests")
}

fn fixtures_dir() -> PathBuf {
    let d = Path::new(env!("CARGO_MANIFEST_DIR")).join("target").join("fixtures");
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn run_ffmpeg(ff: &FfmpegInfo, args: &[&str]) {
    let out = ffmpeg::command(&ff.ffmpeg)
        .args(["-hide_banner", "-loglevel", "error", "-y"])
        .args(args)
        .output()
        .unwrap();
    assert!(out.status.success(), "ffmpeg {:?} failed: {}", args, String::from_utf8_lossy(&out.stderr));
}

/// 6-channel 5.1(side) master WAV plus encoded variants. Returns name -> path.
fn make_fixtures(ff: &FfmpegInfo) -> Vec<(&'static str, PathBuf)> {
    static ONCE: std::sync::OnceLock<Vec<(&'static str, PathBuf)>> = std::sync::OnceLock::new();
    ONCE.get_or_init(|| build_fixtures(ff)).clone()
}

fn build_fixtures(ff: &FfmpegInfo) -> Vec<(&'static str, PathBuf)> {
    let dir = fixtures_dir();
    let master = dir.join("tones_51.wav");
    if !master.exists() {
        let mut args: Vec<String> = Vec::new();
        for (_, f) in TONES {
            args.extend(["-f".into(), "lavfi".into(), "-i".into(), format!("sine=frequency={f}:duration={SECONDS}:sample_rate=48000")]);
        }
        args.extend([
            "-filter_complex".into(),
            "[0][1][2][3][4][5]join=inputs=6:channel_layout=5.1(side):map=0.0-FL|1.0-FR|2.0-FC|3.0-LFE|4.0-SL|5.0-SR[a]".into(),
            "-map".into(),
            "[a]".into(),
            "-c:a".into(),
            "pcm_s24le".into(),
            master.to_string_lossy().into(),
        ]);
        run_ffmpeg(ff, &args.iter().map(String::as_str).collect::<Vec<_>>());
    }
    let m = master.to_string_lossy().to_string();
    let variants: [(&str, &str, &[&str]); 6] = [
        ("ac3", "tones_51.ac3", &["-c:a", "ac3", "-b:a", "640k"]),
        ("dts", "tones_51.dts", &["-c:a", "dca", "-strict", "-2", "-b:a", "1509k"]),
        ("aac", "tones_51.m4a", &["-c:a", "aac", "-b:a", "512k"]),
        ("flac", "tones_51.flac", &["-c:a", "flac"]),
        ("stereo", "tones_20.wav", &["-af", "pan=stereo|FL=FL|FR=FR", "-c:a", "pcm_s16le"]),
        ("mkv", "movie_51.mkv", &[]),
    ];
    let mut out = vec![("wav", master.clone())];
    for (name, file, args) in variants {
        let p = dir.join(file);
        if !p.exists() {
            let ps = p.to_string_lossy().to_string();
            if name == "mkv" {
                // Video + AC3 audio whose stream starts 0.5 s after the container.
                run_ffmpeg(ff, &[
                    "-f", "lavfi", "-i", &format!("testsrc=size=320x180:rate=25:duration={}", SECONDS + 0.5),
                    "-itsoffset", "0.5", "-i", &m,
                    "-map", "0:v", "-map", "1:a", "-c:v", "libx264", "-preset", "ultrafast",
                    "-c:a", "ac3", "-b:a", "640k", &ps,
                ]);
            } else {
                let mut a = vec!["-i", m.as_str()];
                a.extend_from_slice(args);
                a.push(&ps);
                run_ffmpeg(ff, &a);
            }
        }
        out.push((name, p));
    }
    out
}

/// Amplitude of frequency `f` in `x` (Hann-windowed Goertzel, so a strong
/// tone at another frequency does not leak into the measurement).
fn tone_level(x: &[f32], f: f64, rate: f64) -> f64 {
    let n = x.len() as f64;
    let w = 2.0 * std::f64::consts::PI * f / rate;
    let c = 2.0 * w.cos();
    let (mut s1, mut s2) = (0.0f64, 0.0f64);
    for (i, &v) in x.iter().enumerate() {
        let hann = 0.5 - 0.5 * (2.0 * std::f64::consts::PI * i as f64 / (n - 1.0)).cos();
        let s = v as f64 * hann + c * s1 - s2;
        s2 = s1;
        s1 = s;
    }
    let power = s1 * s1 + s2 * s2 - c * s1 * s2;
    // Hann coherent gain is 0.5.
    2.0 * power.sqrt() / (n * 0.5)
}

fn db(x: f64) -> f64 {
    20.0 * x.max(1e-12).log10()
}

fn read_wav(path: &Path) -> (Vec<f32>, Vec<f32>, u32) {
    let mut r = hound::WavReader::open(path).unwrap();
    let spec = r.spec();
    assert_eq!(spec.channels, 2);
    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => r.samples::<f32>().map(|s| s.unwrap()).collect(),
        hound::SampleFormat::Int => {
            let scale = (1i64 << (spec.bits_per_sample - 1)) as f32;
            r.samples::<i32>().map(|s| s.unwrap() as f32 / scale).collect()
        }
    };
    let l = samples.iter().step_by(2).copied().collect();
    let rr = samples.iter().skip(1).step_by(2).copied().collect();
    (l, rr, spec.sample_rate)
}

fn surround_mix(ids: &[String]) -> MixSettings {
    let p = presets::builtins().into_iter().find(|p| p.name == "Surround to Stereo").unwrap();
    presets::apply(&p, ids)
}

fn export_to(ff: &FfmpegInfo, input: &Path, stream: u32, mix: MixSettings, format: OutputFormat, name: &str, align: bool) -> export::ExportResult {
    let out = fixtures_dir().join(name);
    let req = ExportRequest {
        input: input.to_string_lossy().into(),
        stream_index: stream,
        output: out.to_string_lossy().into(),
        format,
        sample_rate: None,
        normalize_dbfs: None,
        align_to_container: align,
        mix,
    };
    export::run(ff, &req, &AtomicBool::new(false), |_| {}).unwrap_or_else(|e| panic!("export {name}: {e}"))
}

#[test]
fn detects_sources() {
    let ff = ff();
    for (name, path) in make_fixtures(&ff) {
        let info = probe::probe(&ff, &path.to_string_lossy()).unwrap();
        let a = &info.audio_streams[0];
        assert!(a.decoder_available, "{name}");
        if name == "stereo" {
            assert_eq!(a.kind, SourceKind::Stereo, "{name}");
            assert_eq!(a.channels, 2);
        } else {
            assert_eq!(a.kind, SourceKind::Multichannel, "{name}");
            assert_eq!(a.channels, 6, "{name}");
            assert!(a.channel_layout.starts_with("5.1"), "{name}: {}", a.channel_layout);
            assert_eq!(a.channel_info.iter().filter(|c| c.role == "SL" || c.role == "SR").count(), 2, "{name}");
        }
        match name {
            "dts" => assert!(a.codec_label.starts_with("DTS"), "{}", a.codec_label),
            "ac3" | "mkv" => assert!(a.codec_label.contains("Dolby Digital")),
            _ => {}
        }
        assert_eq!(info.has_video, name == "mkv", "{name}");
    }
}

#[test]
fn surround_to_stereo_keeps_only_surround_content() {
    let ff = ff();
    for (name, path) in make_fixtures(&ff) {
        if name == "stereo" {
            continue;
        }
        let info = probe::probe(&ff, &path.to_string_lossy()).unwrap();
        let a = &info.audio_streams[0];
        let res = export_to(&ff, &path, a.index, surround_mix(&a.channel_ids()), OutputFormat::Wav { bits: 32 }, &format!("out_{name}.wav"), false);
        assert!(res.verification.ok, "{name}: {:?}", res.verification);
        assert_eq!(res.verification.channels, 2);
        assert!(res.description.starts_with("Stereo 2.0"));

        let (l, r, rate) = read_wav(Path::new(&res.output));
        // Skip codec start-up transients.
        let (l, r) = (&l[rate as usize / 4..], &r[rate as usize / 4..]);
        let rate = rate as f64;
        let lossless = matches!(name, "wav" | "flac");
        let floor_db = if lossless { -90.0 } else { -40.0 };
        let sine = 0.125; // lavfi sine amplitude
        let wanted_l = db(tone_level(l, 1100.0, rate) / sine);
        let wanted_r = db(tone_level(r, 1300.0, rate) / sine);
        assert!(wanted_l.abs() < 1.0, "{name}: SL in Left at {wanted_l:.1} dB");
        assert!(wanted_r.abs() < 1.0, "{name}: SR in Right at {wanted_r:.1} dB");
        for (id, f) in TONES {
            let lev_l = db(tone_level(l, f, rate) / sine);
            let lev_r = db(tone_level(r, f, rate) / sine);
            if id != "SL" {
                assert!(lev_l < floor_db, "{name}: {id} leaks into Left at {lev_l:.1} dB");
            }
            if id != "SR" {
                assert!(lev_r < floor_db, "{name}: {id} leaks into Right at {lev_r:.1} dB");
            }
        }
    }
}

#[test]
fn preserves_duration_and_video_sync() {
    let ff = ff();
    let fx = make_fixtures(&ff);
    let (_, wav) = fx.iter().find(|(n, _)| *n == "wav").unwrap();
    let info = probe::probe(&ff, &wav.to_string_lossy()).unwrap();
    let a = &info.audio_streams[0];
    let res = export_to(&ff, wav, a.index, surround_mix(&a.channel_ids()), OutputFormat::Flac { bits: 24 }, "dur.flac", true);
    // Every decoded source frame is written: exactly the source's own length.
    assert_eq!(res.frames_written, (a.duration * a.sample_rate as f64).round() as u64);
    assert!(res.verification.duration_diff_ms.abs() < 1.0, "{:?}", res.verification);

    let (_, mkv) = fx.iter().find(|(n, _)| *n == "mkv").unwrap();
    let info = probe::probe(&ff, &mkv.to_string_lossy()).unwrap();
    let a = &info.audio_streams[0];
    let res = export_to(&ff, mkv, a.index, surround_mix(&a.channel_ids()), OutputFormat::Wav { bits: 16 }, "sync.wav", true);
    assert!((res.padding_seconds - 0.5).abs() < 0.05, "padding {}", res.padding_seconds);
    // First half-second is silence, then the tones begin.
    let (l, _, rate) = read_wav(Path::new(&res.output));
    let lead = &l[..(rate as f64 * 0.45) as usize];
    assert!(lead.iter().all(|x| x.abs() < 1e-3), "expected silent lead-in");
}

#[test]
fn every_format_exports_stereo() {
    let ff = ff();
    let fx = make_fixtures(&ff);
    let (_, dts) = fx.iter().find(|(n, _)| *n == "dts").unwrap();
    let info = probe::probe(&ff, &dts.to_string_lossy()).unwrap();
    let a = &info.audio_streams[0];
    for (fmt, file) in [
        (OutputFormat::Wav { bits: 24 }, "fmt.wav"),
        (OutputFormat::Flac { bits: 16 }, "fmt.flac"),
        (OutputFormat::Mp3 { vbr: false }, "fmt.mp3"),
        (OutputFormat::Aac { kbps: 256 }, "fmt.m4a"),
    ] {
        let res = export_to(&ff, dts, a.index, surround_mix(&a.channel_ids()), fmt, file, false);
        assert!(res.verification.ok, "{file}: {:?}", res.verification);
        assert_eq!(res.verification.channels, 2, "{file}");
        assert_eq!(res.verification.channel_layout, "stereo", "{file}");
        // The output file never claims to be DTS.
        assert_ne!(res.verification.codec, "dts", "{file}");
        let probed = probe::probe(&ff, &res.output).unwrap();
        assert_eq!(probed.audio_streams[0].kind, SourceKind::Stereo, "{file}");
    }
}

#[test]
fn full_itu_downmix_with_safe_gain_does_not_clip() {
    let ff = ff();
    let dir = fixtures_dir();
    let loud = dir.join("fullscale_51.wav");
    if !loud.exists() {
        // In-phase full-scale square-ish content on all six channels.
        run_ffmpeg(&ff, &[
            "-f", "lavfi", "-i", "aevalsrc=exprs='if(lt(mod(t*100,1),0.5),0.99,-0.99)':s=48000:d=2:c=5.1(side)",
            "-c:a", "pcm_f32le", &loud.to_string_lossy(),
        ]);
    }
    let info = probe::probe(&ff, &loud.to_string_lossy()).unwrap();
    let a = &info.audio_streams[0];
    let mut mix = channel_picker_lib::mix::default_mix(&a.channel_ids());
    mix.channels.iter_mut().for_each(|c| c.enabled = true);
    mix.gain_mode = GainMode::Safe;
    let res = export_to(&ff, &loud, a.index, mix, OutputFormat::Wav { bits: 32 }, "fullscale_out.wav", false);
    assert_eq!(res.clipped_samples, 0);
    assert!(res.peak_dbfs <= 0.0, "peak {}", res.peak_dbfs);
}
