//! Locating ffmpeg/ffprobe, querying their codec support, and streaming raw
//! float PCM out of ffmpeg's decoder.

use anyhow::{anyhow, bail, Context, Result};
use serde::Serialize;
use std::collections::BTreeSet;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdout, Command, Stdio};
use std::thread::JoinHandle;

/// Environment variable that points at a folder containing ffmpeg + ffprobe.
pub const FFMPEG_DIR_ENV: &str = "CHANNEL_PICKER_FFMPEG_DIR";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FfmpegInfo {
    pub ffmpeg: PathBuf,
    pub ffprobe: PathBuf,
    pub version: String,
    #[serde(skip)]
    pub decoders: BTreeSet<String>,
    #[serde(skip)]
    pub encoders: BTreeSet<String>,
}

impl FfmpegInfo {
    pub fn has_decoder(&self, name: &str) -> bool {
        self.decoders.contains(name)
    }
    pub fn has_encoder(&self, name: &str) -> bool {
        self.encoders.contains(name)
    }
}

/// A `Command` that never opens a console window on Windows.
pub fn command(program: &Path) -> Command {
    let mut c = Command::new(program);
    c.stdin(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        c.creation_flags(CREATE_NO_WINDOW);
    }
    c
}

fn exe(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    }
}

fn find_on_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|d| d.join(exe(name)))
        .find(|p| p.is_file())
}

/// Find ffmpeg/ffprobe: explicit folder, then `$CHANNEL_PICKER_FFMPEG_DIR`, then PATH.
pub fn locate(dir_override: Option<&Path>) -> Result<FfmpegInfo> {
    let env_dir = std::env::var_os(FFMPEG_DIR_ENV).map(PathBuf::from);
    let dir = dir_override.map(Path::to_path_buf).or(env_dir);
    let (ffmpeg, ffprobe) = match dir {
        Some(d) => (d.join(exe("ffmpeg")), d.join(exe("ffprobe"))),
        None => (
            find_on_path("ffmpeg").ok_or_else(|| anyhow!("ffmpeg was not found on PATH"))?,
            find_on_path("ffprobe").ok_or_else(|| anyhow!("ffprobe was not found on PATH"))?,
        ),
    };
    if !ffmpeg.is_file() {
        bail!("ffmpeg not found at {}", ffmpeg.display());
    }
    if !ffprobe.is_file() {
        bail!("ffprobe not found at {}", ffprobe.display());
    }
    let version = run_text(&ffmpeg, &["-hide_banner", "-version"])?
        .lines()
        .next()
        .unwrap_or_default()
        .to_string();
    let decoders = parse_codec_list(&run_text(&ffmpeg, &["-hide_banner", "-decoders"])?);
    let encoders = parse_codec_list(&run_text(&ffmpeg, &["-hide_banner", "-encoders"])?);
    Ok(FfmpegInfo { ffmpeg, ffprobe, version, decoders, encoders })
}

pub fn run_text(program: &Path, args: &[&str]) -> Result<String> {
    let out = command(program)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .with_context(|| format!("failed to run {}", program.display()))?;
    if !out.status.success() {
        bail!(
            "{} failed: {}",
            program.display(),
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Parse `ffmpeg -decoders` / `-encoders` output, keeping audio codecs only.
pub fn parse_codec_list(text: &str) -> BTreeSet<String> {
    let mut set = BTreeSet::new();
    let mut in_list = false;
    for line in text.lines() {
        let t = line.trim();
        if !in_list {
            in_list = t.starts_with("------");
            continue;
        }
        let mut parts = t.split_whitespace();
        if let (Some(flags), Some(name)) = (parts.next(), parts.next()) {
            if flags.starts_with('A') {
                set.insert(name.to_string());
            }
        }
    }
    set
}

/// Collects a child's stderr on a background thread (keeps the pipe drained).
pub fn drain_stderr(child: &mut Child) -> Option<JoinHandle<String>> {
    let mut err = child.stderr.take()?;
    Some(std::thread::spawn(move || {
        let mut s = String::new();
        let _ = err.read_to_string(&mut s);
        s
    }))
}

/// Streams decoded audio from one stream as interleaved little-endian f32.
pub struct PcmDecoder {
    child: Child,
    stdout: ChildStdout,
    channels: usize,
    bytes: Vec<u8>,
    filled: usize,
    stderr: Option<JoinHandle<String>>,
}

impl PcmDecoder {
    pub const CHUNK_FRAMES: usize = 1024;

    /// `stream_index` is the absolute stream index in the container.
    pub fn spawn(ffmpeg: &Path, input: &str, stream_index: u32, seek: f64, channels: usize) -> Result<Self> {
        if channels == 0 {
            bail!("stream has no channels");
        }
        let mut cmd = command(ffmpeg);
        cmd.args(["-hide_banner", "-loglevel", "error", "-nostdin"]);
        if seek > 0.0 {
            cmd.args(["-ss", &format!("{seek:.3}")]);
        }
        cmd.args(["-i", input])
            .args(["-map", &format!("0:{stream_index}")])
            .args(["-vn", "-sn", "-dn", "-c:a", "pcm_f32le", "-f", "f32le", "pipe:1"])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = cmd.spawn().context("failed to start ffmpeg decoder")?;
        let stdout = child.stdout.take().ok_or_else(|| anyhow!("no decoder stdout"))?;
        let stderr = drain_stderr(&mut child);
        Ok(Self {
            child,
            stdout,
            channels,
            bytes: vec![0; Self::CHUNK_FRAMES * channels * 4],
            filled: 0,
            stderr,
        })
    }

    pub fn channels(&self) -> usize {
        self.channels
    }

    /// Fills `out` with whole interleaved frames. Returns false at end of stream.
    pub fn read_chunk(&mut self, out: &mut Vec<f32>) -> Result<bool> {
        out.clear();
        let frame_bytes = self.channels * 4;
        loop {
            let n = self.stdout.read(&mut self.bytes[self.filled..])?;
            if n == 0 {
                return Ok(false); // a trailing partial frame is dropped
            }
            self.filled += n;
            let whole = self.filled / frame_bytes * frame_bytes;
            if whole == 0 {
                continue;
            }
            out.extend(
                self.bytes[..whole]
                    .chunks_exact(4)
                    .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])),
            );
            self.bytes.copy_within(whole..self.filled, 0);
            self.filled -= whole;
            return Ok(true);
        }
    }

    /// Waits for ffmpeg to exit and reports any decode failure.
    pub fn finish(mut self) -> Result<()> {
        let status = self.child.wait()?;
        let err = self.stderr.take().and_then(|h| h.join().ok()).unwrap_or_default();
        if !status.success() {
            bail!("decoder failed: {}", err.trim());
        }
        Ok(())
    }
}

impl Drop for PcmDecoder {
    fn drop(&mut self) {
        if let Ok(None) = self.child.try_wait() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_audio_codecs_only() {
        let text = "Decoders:\n A....D = Audio\n ------\n V....D h264   H.264\n A....D dca    DCA (DTS)\n A....D ac3    ATSC A/52A\n S..... srt    SubRip\n";
        let set = parse_codec_list(text);
        assert!(set.contains("dca"));
        assert!(set.contains("ac3"));
        assert!(!set.contains("h264"));
        assert!(!set.contains("srt"));
    }
}
