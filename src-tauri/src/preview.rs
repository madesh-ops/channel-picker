//! Real-time preview: ffmpeg decode -> mix (live settings) -> resample ->
//! small ring buffer -> cpal output device.

use crate::ffmpeg::PcmDecoder;
use crate::mix::{MixSettings, Peaks};
use anyhow::{anyhow, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SizedSample};
use parking_lot::{Mutex, RwLock};
use serde::Serialize;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::io::Read;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{mpsc, Arc};
use std::time::Duration;

/// Audio buffered ahead of the device. Small, so mix changes are heard quickly.
const BUFFER_MS: usize = 120;
/// Mono samples kept from what the device actually played, for the spectrum.
const SCOPE_LEN: usize = 2048;
const FFT_LEN: usize = 1024;
/// Log-spaced frequency bands sent to the visualizer.
pub const SPECTRUM_BANDS: usize = 32;

#[derive(Debug, Clone)]
pub struct PreviewSource {
    pub ffmpeg: PathBuf,
    pub path: String,
    pub stream_index: u32,
    pub channels: usize,
    pub sample_rate: u32,
    pub duration: f64,
    pub video: Option<VideoParams>,
}

#[derive(Debug, Clone)]
pub struct VideoParams {
    /// Absolute index of the video stream.
    pub index: u32,
    pub fps: f64,
    /// Audio stream start minus container start: added to the audio position
    /// to get the file time of the matching video frame.
    pub offset: f64,
}

/// Full quality: source resolution, source frame rate (up to 60), near-lossless JPEG.
const FULL_MAX_FPS: f64 = 60.0;
const FULL_JPEG_Q: &str = "2";
/// Fast mode for slower machines: 960 px wide, 30 fps.
const FAST_WIDTH: u32 = 960;
const FAST_MAX_FPS: f64 = 30.0;
const FAST_JPEG_Q: &str = "5";

pub type FrameNotifier = Arc<dyn Fn(u64) + Send + Sync>;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Tick {
    pub position: f64,
    pub duration: f64,
    pub playing: bool,
    pub ended: bool,
    pub input_peaks: Vec<f32>,
    pub output_peaks: [f32; 2],
    pub clipped: bool,
    pub error: Option<String>,
    /// 0..1 per band, 40 Hz .. 16 kHz, from audio the device just played.
    pub spectrum: Vec<f32>,
}

struct Shared {
    scope: Mutex<VecDeque<f32>>,
    /// Playback clock: start position (f64 bits) + played frames / device rate.
    start_pos: AtomicU64,
    rate: AtomicU32,
    video_enabled: AtomicBool,
    /// true = full quality (default), false = fast/downscaled.
    video_full: AtomicBool,
    video_gen: AtomicU64,
    /// Latest decoded video frame: (sequence number, JPEG bytes).
    frame: Mutex<Option<(u64, Arc<Vec<u8>>)>>,
    notify: RwLock<Option<FrameNotifier>>,
    buf: Mutex<VecDeque<[f32; 2]>>,
    playing: AtomicBool,
    generation: AtomicU64,
    /// Device frames consumed since the last (re)start.
    played: AtomicU64,
    eof: AtomicBool,
    peaks: Mutex<Peaks>,
    error: Mutex<Option<String>>,
}

struct Output {
    rate: u32,
    _stop: mpsc::Sender<()>,
}

#[derive(Default)]
struct PlayState {
    source: Option<PreviewSource>,
    start_pos: f64,
    decoder_started: bool,
    ended: bool,
}

pub struct Preview {
    shared: Arc<Shared>,
    mix: Arc<RwLock<Option<MixSettings>>>,
    output: Mutex<Option<Output>>,
    state: Mutex<PlayState>,
}

impl Default for Preview {
    fn default() -> Self {
        Self::new()
    }
}

impl Preview {
    pub fn new() -> Self {
        Self {
            shared: Arc::new(Shared {
                scope: Mutex::new(VecDeque::with_capacity(SCOPE_LEN + 1)),
                start_pos: AtomicU64::new(0f64.to_bits()),
                rate: AtomicU32::new(0),
                video_enabled: AtomicBool::new(true),
                video_full: AtomicBool::new(true),
                video_gen: AtomicU64::new(0),
                frame: Mutex::new(None),
                notify: RwLock::new(None),
                buf: Mutex::new(VecDeque::new()),
                playing: AtomicBool::new(false),
                generation: AtomicU64::new(0),
                played: AtomicU64::new(0),
                eof: AtomicBool::new(false),
                peaks: Mutex::new(Peaks::new(0)),
                error: Mutex::new(None),
            }),
            mix: Arc::new(RwLock::new(None)),
            output: Mutex::new(None),
            state: Mutex::new(PlayState::default()),
        }
    }

    fn device_rate(&self) -> Result<u32> {
        let mut out = self.output.lock();
        if let Some(o) = out.as_ref() {
            return Ok(o.rate);
        }
        let o = start_output(self.shared.clone())?;
        let rate = o.rate;
        self.shared.rate.store(rate, Ordering::SeqCst);
        *out = Some(o);
        Ok(rate)
    }

    pub fn load(&self, src: PreviewSource) {
        self.stop();
        *self.shared.peaks.lock() = Peaks::new(src.channels);
        *self.shared.frame.lock() = None;
        self.shared.start_pos.store(0f64.to_bits(), Ordering::SeqCst);
        self.shared.played.store(0, Ordering::SeqCst);
        let mut st = self.state.lock();
        *st = PlayState { source: Some(src), ..Default::default() };
        // Poster frame: the video thread shows frame 0 and then waits for play.
        self.start_video(&st, 0.0);
    }

    /// Called whenever a new video frame is ready (sequence number).
    pub fn set_frame_notifier(&self, f: FrameNotifier) {
        *self.shared.notify.write() = Some(f);
    }

    /// Latest preview video frame as JPEG.
    pub fn frame(&self) -> Option<(u64, Arc<Vec<u8>>)> {
        self.shared.frame.lock().clone()
    }

    pub fn set_video_enabled(&self, enabled: bool) {
        self.shared.video_enabled.store(enabled, Ordering::SeqCst);
        self.shared.video_gen.fetch_add(1, Ordering::SeqCst);
        if enabled {
            let st = self.state.lock();
            let pos = self.shared.clock();
            self.start_video(&st, pos);
        }
    }

    /// Full quality (source resolution) or fast (960 px, 30 fps). Restarts the picture in place.
    pub fn set_video_full_quality(&self, full: bool) {
        self.shared.video_full.store(full, Ordering::SeqCst);
        self.shared.video_gen.fetch_add(1, Ordering::SeqCst);
        let st = self.state.lock();
        let pos = self.shared.clock();
        self.start_video(&st, pos);
    }

    fn start_video(&self, st: &PlayState, pos: f64) {
        let Some(src) = st.source.clone() else { return };
        let Some(video) = src.video.clone() else { return };
        if !self.shared.video_enabled.load(Ordering::SeqCst) {
            return;
        }
        let gen = self.shared.generation.load(Ordering::SeqCst);
        let vgen = self.shared.video_gen.fetch_add(1, Ordering::SeqCst) + 1;
        let shared = self.shared.clone();
        std::thread::Builder::new()
            .name("preview-video".into())
            .spawn(move || {
                let _ = video_loop(&shared, &src, &video, pos, gen, vgen);
            })
            .expect("spawn video thread");
    }

    pub fn set_mix(&self, mix: MixSettings) {
        *self.mix.write() = Some(mix);
    }

    pub fn play(&self) -> Result<()> {
        let rate = self.device_rate()?;
        let mut st = self.state.lock();
        if st.source.is_none() {
            return Err(anyhow!("no file loaded"));
        }
        if st.ended || !st.decoder_started {
            let pos = if st.ended { 0.0 } else { st.start_pos };
            self.start_decoder(&mut st, pos, rate);
        }
        self.shared.playing.store(true, Ordering::SeqCst);
        Ok(())
    }

    pub fn pause(&self) {
        self.shared.playing.store(false, Ordering::SeqCst);
    }

    pub fn seek(&self, pos: f64) -> Result<()> {
        let rate = self.device_rate()?;
        let mut st = self.state.lock();
        let Some(src) = st.source.as_ref() else { return Ok(()) };
        let pos = pos.clamp(0.0, src.duration.max(0.0));
        self.start_decoder(&mut st, pos, rate);
        Ok(())
    }

    pub fn stop(&self) {
        self.shared.playing.store(false, Ordering::SeqCst);
        self.shared.generation.fetch_add(1, Ordering::SeqCst);
        self.shared.buf.lock().clear();
        let mut st = self.state.lock();
        st.decoder_started = false;
        st.start_pos = 0.0;
    }

    fn start_decoder(&self, st: &mut PlayState, pos: f64, dev_rate: u32) {
        let gen = self.shared.generation.fetch_add(1, Ordering::SeqCst) + 1;
        self.shared.buf.lock().clear();
        self.shared.played.store(0, Ordering::SeqCst);
        self.shared.eof.store(false, Ordering::SeqCst);
        *self.shared.error.lock() = None;
        self.shared.start_pos.store(pos.to_bits(), Ordering::SeqCst);
        st.start_pos = pos;
        st.ended = false;
        st.decoder_started = true;
        self.start_video(st, pos);
        let src = st.source.clone().expect("source checked by caller");
        let shared = self.shared.clone();
        let mix = self.mix.clone();
        std::thread::Builder::new()
            .name("preview-decoder".into())
            .spawn(move || {
                if let Err(e) = decode_loop(&shared, &mix, &src, pos, dev_rate, gen) {
                    if shared.generation.load(Ordering::SeqCst) == gen {
                        *shared.error.lock() = Some(e.to_string());
                    }
                }
                if shared.generation.load(Ordering::SeqCst) == gen {
                    shared.eof.store(true, Ordering::SeqCst);
                }
            })
            .expect("spawn decoder thread");
    }

    /// Snapshot for the UI; resets meter peaks.
    pub fn tick(&self) -> Option<Tick> {
        let rate = self.output.lock().as_ref().map(|o| o.rate);
        let mut st = self.state.lock();
        let src = st.source.as_ref()?.clone();
        let played = self.shared.played.load(Ordering::SeqCst) as f64;
        let mut position = st.start_pos + rate.map(|r| played / r as f64).unwrap_or(0.0);
        if self.shared.eof.load(Ordering::SeqCst) && self.shared.buf.lock().is_empty() && st.decoder_started {
            if self.shared.playing.swap(false, Ordering::SeqCst) || !st.ended {
                st.ended = true;
            }
            position = src.duration;
        }
        position = position.min(src.duration.max(0.0));
        let playing = self.shared.playing.load(Ordering::SeqCst);
        let spectrum = match (playing, rate) {
            (true, Some(r)) => {
                let s = self.shared.scope.lock();
                if s.len() >= FFT_LEN {
                    let latest: Vec<f32> = s.iter().skip(s.len() - FFT_LEN).copied().collect();
                    spectrum_bands(&latest, r, SPECTRUM_BANDS)
                } else {
                    vec![0.0; SPECTRUM_BANDS]
                }
            }
            _ => vec![0.0; SPECTRUM_BANDS],
        };
        let mut peaks = self.shared.peaks.lock();
        let tick = Tick {
            spectrum,
            position,
            duration: src.duration,
            playing: self.shared.playing.load(Ordering::SeqCst),
            ended: st.ended,
            input_peaks: peaks.input.clone(),
            output_peaks: peaks.output,
            clipped: peaks.clipped > 0,
            error: self.shared.error.lock().clone(),
        };
        *peaks = Peaks::new(src.channels);
        Some(tick)
    }
}

impl Drop for Preview {
    fn drop(&mut self) {
        self.shared.generation.fetch_add(1, Ordering::SeqCst);
    }
}

impl Shared {
    /// Current playback position in seconds (what the device has played).
    fn clock(&self) -> f64 {
        let start = f64::from_bits(self.start_pos.load(Ordering::SeqCst));
        let rate = self.rate.load(Ordering::SeqCst);
        if rate == 0 {
            return start;
        }
        start + self.played.load(Ordering::SeqCst) as f64 / rate as f64
    }
}

/// Splits a concatenated JPEG stream (ffmpeg image2pipe/mjpeg) into frames.
/// Returns the byte length of the first complete frame in `buf`, if any.
fn next_jpeg(buf: &[u8]) -> Option<usize> {
    if buf.len() < 4 || buf[0] != 0xFF || buf[1] != 0xD8 {
        return None;
    }
    // Inside entropy-coded data 0xFF is always followed by 0x00 or a restart
    // marker, so the first FF D9 after the header is the end of image.
    (2..buf.len() - 1).find(|&i| buf[i] == 0xFF && buf[i + 1] == 0xD9).map(|i| i + 2)
}

/// Decodes the video stream to small JPEG frames and publishes each one when
/// the audio clock reaches its timestamp. Paused audio = held picture.
fn video_loop(shared: &Shared, src: &PreviewSource, v: &VideoParams, pos: f64, gen: u64, vgen: u64) -> Result<()> {
    let current = || {
        shared.generation.load(Ordering::SeqCst) == gen && shared.video_gen.load(Ordering::SeqCst) == vgen
    };
    let full = shared.video_full.load(Ordering::SeqCst);
    let (fps, quality, filter) = if full {
        let fps = v.fps.clamp(1.0, FULL_MAX_FPS);
        // Native resolution; only non-square pixels are stretched to their display width.
        (fps, FULL_JPEG_Q, format!("fps={fps:.6},scale=w='trunc(iw*sar/2)*2':h=ih,setsar=1"))
    } else {
        let fps = v.fps.clamp(1.0, FAST_MAX_FPS);
        (fps, FAST_JPEG_Q, format!("fps={fps:.6},scale=w='min({FAST_WIDTH},iw*sar)':h=-2,setsar=1"))
    };
    let mut child = crate::ffmpeg::command(&src.ffmpeg)
        .args(["-hide_banner", "-loglevel", "error", "-nostdin"])
        .args(["-ss", &format!("{:.3}", (pos + v.offset).max(0.0))])
        .args(["-i", &src.path, "-map", &format!("0:{}", v.index), "-an", "-sn", "-dn"])
        .args(["-vf", &filter, "-pix_fmt", "yuvj420p", "-c:v", "mjpeg", "-q:v", quality, "-f", "image2pipe", "pipe:1"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    let mut out = child.stdout.take().ok_or_else(|| anyhow!("no video stdout"))?;
    let result = (|| -> Result<()> {
        let mut buf: Vec<u8> = Vec::with_capacity(512 * 1024);
        let mut chunk = vec![0u8; 64 * 1024];
        let mut n: u64 = 0;
        loop {
            let len = loop {
                if let Some(len) = next_jpeg(&buf) {
                    break len;
                }
                if !current() {
                    return Ok(());
                }
                let r = out.read(&mut chunk)?;
                if r == 0 {
                    return Ok(()); // end of video
                }
                buf.extend_from_slice(&chunk[..r]);
            };
            let jpeg: Vec<u8> = buf.drain(..len).collect();
            let due = pos + n as f64 / fps;
            n += 1;
            // Wait for the audio clock (holds while paused).
            loop {
                if !current() {
                    return Ok(());
                }
                if shared.clock() + 0.5 / fps >= due {
                    break;
                }
                std::thread::sleep(Duration::from_millis(3));
            }
            // Too late (decoder fell behind): skip publishing, keep decoding to catch up.
            if shared.clock() - due > 2.0 / fps && n > 1 {
                continue;
            }
            let seq = {
                let mut f = shared.frame.lock();
                let seq = f.as_ref().map(|(s, _)| s + 1).unwrap_or(1);
                *f = Some((seq, Arc::new(jpeg)));
                seq
            };
            if let Some(cb) = shared.notify.read().as_ref() {
                cb(seq);
            }
        }
    })();
    let _ = child.kill();
    let _ = child.wait();
    result
}

/// Stateful linear resampler for stereo frames.
pub struct Resampler {
    step: f64,
    pos: f64,
    prev: [f32; 2],
}

impl Resampler {
    pub fn new(src_rate: u32, dst_rate: u32) -> Self {
        Self { step: src_rate as f64 / dst_rate as f64, pos: 1.0, prev: [0.0; 2] }
    }

    pub fn process(&mut self, input: &[[f32; 2]], out: &mut Vec<[f32; 2]>) {
        if (self.step - 1.0).abs() < 1e-9 {
            out.extend_from_slice(input);
            return;
        }
        let n = input.len();
        if n == 0 {
            return;
        }
        // Index i in [-1, n) where -1 is the last frame of the previous block.
        let at = |i: isize| if i < 0 { self.prev } else { input[i as usize] };
        while self.pos < n as f64 {
            let a = self.pos.floor() as isize - 1;
            let t = (self.pos - self.pos.floor()) as f32;
            let (x, y) = (at(a), at(a + 1));
            out.push([x[0] + (y[0] - x[0]) * t, x[1] + (y[1] - x[1]) * t]);
            self.pos += self.step;
        }
        self.pos -= n as f64;
        self.prev = input[n - 1];
    }
}

fn decode_loop(
    shared: &Shared,
    mix: &RwLock<Option<MixSettings>>,
    src: &PreviewSource,
    pos: f64,
    dev_rate: u32,
    gen: u64,
) -> Result<()> {
    let current = || shared.generation.load(Ordering::SeqCst) == gen;
    let mut dec = PcmDecoder::spawn(&src.ffmpeg, &src.path, src.stream_index, pos, src.channels)?;
    let n = src.channels;
    let mut rs = Resampler::new(src.sample_rate, dev_rate);
    let target = dev_rate as usize * BUFFER_MS / 1000;
    let (mut input, mut mixed, mut resampled) = (Vec::new(), Vec::new(), Vec::new());
    let mut local = Peaks::new(n);

    while current() {
        if !dec.read_chunk(&mut input)? {
            if current() {
                dec.finish()?;
            }
            return Ok(());
        }
        let matrix = match mix.read().as_ref() {
            Some(m) => m.matrix(n),
            None => crate::mix::default_mix(&vec![String::new(); n]).matrix(n),
        };
        mixed.clear();
        matrix.mix(&input, &mut mixed);

        local.input.iter_mut().for_each(|x| *x = 0.0);
        local.output = [0.0; 2];
        local.clipped = 0;
        local.track_input(&input);
        local.track_output(&mixed);

        resampled.clear();
        rs.process(&mixed, &mut resampled);

        // Back-pressure: wait until the device has room (or we are superseded).
        loop {
            if !current() {
                return Ok(());
            }
            {
                let mut b = shared.buf.lock();
                if b.len() < target {
                    b.extend(resampled.drain(..));
                    break;
                }
            }
            std::thread::sleep(Duration::from_millis(4));
        }
        let mut p = shared.peaks.lock();
        if p.input.len() == n {
            for (a, b) in p.input.iter_mut().zip(&local.input) {
                *a = a.max(*b);
            }
        }
        p.output[0] = p.output[0].max(local.output[0]);
        p.output[1] = p.output[1].max(local.output[1]);
        p.clipped += local.clipped;
    }
    Ok(())
}

fn start_output(shared: Arc<Shared>) -> Result<Output> {
    let (ready_tx, ready_rx) = mpsc::channel::<std::result::Result<u32, String>>();
    let (stop_tx, stop_rx) = mpsc::channel::<()>();
    std::thread::Builder::new().name("audio-out".into()).spawn(move || match build_stream(shared) {
        Ok((stream, rate)) => {
            let _ = ready_tx.send(Ok(rate));
            let _ = stop_rx.recv(); // keep the stream alive until Output is dropped
            drop(stream);
        }
        Err(e) => {
            let _ = ready_tx.send(Err(e.to_string()));
        }
    })?;
    let rate = ready_rx.recv()?.map_err(|e| anyhow!("audio output unavailable: {e}"))?;
    Ok(Output { rate, _stop: stop_tx })
}

fn build_stream(shared: Arc<Shared>) -> Result<(cpal::Stream, u32)> {
    let host = cpal::default_host();
    let device = host.default_output_device().ok_or_else(|| anyhow!("no output device"))?;
    let supported = device.default_output_config()?;
    let format = supported.sample_format();
    let config: cpal::StreamConfig = supported.into();
    let rate = config.sample_rate.0;
    let stream = match format {
        cpal::SampleFormat::F32 => build::<f32>(&device, &config, shared)?,
        cpal::SampleFormat::I16 => build::<i16>(&device, &config, shared)?,
        cpal::SampleFormat::U16 => build::<u16>(&device, &config, shared)?,
        cpal::SampleFormat::I32 => build::<i32>(&device, &config, shared)?,
        other => return Err(anyhow!("unsupported device sample format {other:?}")),
    };
    stream.play()?;
    Ok((stream, rate))
}

fn build<T>(device: &cpal::Device, config: &cpal::StreamConfig, shared: Arc<Shared>) -> Result<cpal::Stream>
where
    T: SizedSample + FromSample<f32>,
{
    let ch = config.channels as usize;
    let err_shared = shared.clone();
    let stream = device.build_output_stream(
        config,
        move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
            let playing = shared.playing.load(Ordering::Relaxed);
            let mut buf = if playing { shared.buf.try_lock() } else { None };
            let mut scope = if playing { shared.scope.try_lock() } else { None };
            let mut consumed = 0u64;
            for frame in data.chunks_mut(ch) {
                let [l, r] = match buf.as_mut().and_then(|b| b.pop_front()) {
                    Some(f) => {
                        consumed += 1;
                        if let Some(s) = scope.as_mut() {
                            if s.len() >= SCOPE_LEN {
                                s.pop_front();
                            }
                            s.push_back((f[0] + f[1]) * 0.5);
                        }
                        f
                    }
                    None => [0.0, 0.0],
                };
                let (l, r) = (l.clamp(-1.0, 1.0), r.clamp(-1.0, 1.0));
                if ch == 1 {
                    frame[0] = T::from_sample((l + r) * 0.5);
                } else {
                    frame[0] = T::from_sample(l);
                    frame[1] = T::from_sample(r);
                    for s in frame.iter_mut().skip(2) {
                        *s = T::from_sample(0.0f32);
                    }
                }
            }
            shared.played.fetch_add(consumed, Ordering::Relaxed);
        },
        move |e| {
            *err_shared.error.lock() = Some(format!("audio device error: {e}"));
        },
        None,
    )?;
    Ok(stream)
}

/// In-place iterative radix-2 FFT. `re.len()` must be a power of two.
fn fft(re: &mut [f32], im: &mut [f32]) {
    let n = re.len();
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        let ang = -2.0 * std::f64::consts::PI / len as f64;
        let (wr, wi) = (ang.cos() as f32, ang.sin() as f32);
        for start in (0..n).step_by(len) {
            let (mut cr, mut ci) = (1.0f32, 0.0f32);
            for k in 0..len / 2 {
                let (a, b) = (start + k, start + k + len / 2);
                let br = re[b] * cr - im[b] * ci;
                let bi = re[b] * ci + im[b] * cr;
                re[b] = re[a] - br;
                im[b] = im[a] - bi;
                re[a] += br;
                im[a] += bi;
                let ncr = cr * wr - ci * wi;
                ci = cr * wi + ci * wr;
                cr = ncr;
            }
        }
        len <<= 1;
    }
}

/// Hann-windowed spectrum folded into `bands` log-spaced bands (40 Hz .. 16 kHz),
/// each scaled to 0..1 from a −66..−6 dBFS range.
pub fn spectrum_bands(samples: &[f32], rate: u32, bands: usize) -> Vec<f32> {
    let n = samples.len();
    let mut re: Vec<f32> = samples
        .iter()
        .enumerate()
        .map(|(i, x)| x * (0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / (n as f32 - 1.0)).cos()))
        .collect();
    let mut im = vec![0.0f32; n];
    fft(&mut re, &mut im);
    let bin_hz = rate as f32 / n as f32;
    let (lo, hi) = (40.0f32, 16_000f32.min(rate as f32 / 2.0));
    let scale = 2.0 / (n as f32 * 0.5); // amplitude, Hann coherent gain 0.5
    (0..bands)
        .map(|b| {
            let f0 = lo * (hi / lo).powf(b as f32 / bands as f32);
            let f1 = lo * (hi / lo).powf((b + 1) as f32 / bands as f32);
            let k0 = ((f0 / bin_hz).floor() as usize).max(1);
            let k1 = ((f1 / bin_hz).ceil() as usize).clamp(k0 + 1, n / 2);
            let mag = (k0..k1).map(|k| (re[k] * re[k] + im[k] * im[k]).sqrt()).fold(0.0f32, f32::max) * scale;
            let db = 20.0 * mag.max(1e-9).log10();
            ((db + 66.0) / 60.0).clamp(0.0, 1.0)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_concatenated_jpegs() {
        let a = [0xFF, 0xD8, 0xFF, 0xE0, 0x01, 0xFF, 0x00, 0x02, 0xFF, 0xD9];
        let b = [0xFF, 0xD8, 0x03, 0xFF, 0xD9];
        let mut stream = a.to_vec();
        stream.extend_from_slice(&b);
        assert_eq!(next_jpeg(&stream), Some(a.len()));
        assert_eq!(next_jpeg(&stream[a.len()..]), Some(b.len()));
        assert_eq!(next_jpeg(&a[..6]), None); // incomplete
        assert_eq!(next_jpeg(&[0x00, 0x01, 0x02, 0x03]), None); // not a JPEG start
    }

    #[test]
    fn spectrum_finds_the_tone() {
        let rate = 48_000;
        let tone: Vec<f32> =
            (0..FFT_LEN).map(|i| 0.5 * (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / rate as f32).sin()).collect();
        let bands = spectrum_bands(&tone, rate, SPECTRUM_BANDS);
        let (peak_band, peak) = bands.iter().enumerate().fold((0, 0.0f32), |a, (i, &v)| if v > a.1 { (i, v) } else { a });
        // 1 kHz sits in band floor(32 * ln(1000/40) / ln(16000/40)) = 17.
        assert_eq!(peak_band, 17, "{bands:?}");
        assert!(peak > 0.85, "{peak}");
        assert!(bands[0] < 0.2 && bands[SPECTRUM_BANDS - 1] < 0.2, "{bands:?}");
        assert!(spectrum_bands(&vec![0.0; FFT_LEN], rate, SPECTRUM_BANDS).iter().all(|&b| b == 0.0));
    }

    #[test]
    fn resampler_ratio_and_continuity() {
        let mut rs = Resampler::new(48_000, 44_100);
        let input: Vec<[f32; 2]> = (0..48_000).map(|i| [i as f32, -(i as f32)]).collect();
        let mut out = Vec::new();
        for chunk in input.chunks(1000) {
            rs.process(chunk, &mut out);
        }
        assert!((out.len() as i64 - 44_100).abs() <= 2, "{}", out.len());
        // Linear ramp stays monotonic across block boundaries.
        assert!(out.windows(2).all(|w| w[1][0] > w[0][0]));
    }

    #[test]
    fn resampler_passthrough() {
        let mut rs = Resampler::new(48_000, 48_000);
        let mut out = Vec::new();
        rs.process(&[[0.5, 0.25]], &mut out);
        assert_eq!(out, vec![[0.5, 0.25]]);
    }
}
