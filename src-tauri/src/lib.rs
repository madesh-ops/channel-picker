pub mod export;
pub mod ffmpeg;
pub mod layout;
pub mod mix;
pub mod presets;
pub mod preview;
pub mod probe;

use ffmpeg::FfmpegInfo;
use mix::{GainReport, MixSettings};
use parking_lot::Mutex;
use presets::{Preset, PresetStore};
use preview::{Preview, PreviewSource, VideoParams};
use probe::MediaInfo;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tauri::{Emitter, Manager, State};

pub struct AppState {
    ff: Mutex<Result<FfmpegInfo, String>>,
    preview: Preview,
    cancel: Arc<AtomicBool>,
    exporting: Arc<AtomicBool>,
    presets: PresetStore,
}

type CmdResult<T> = Result<T, String>;

fn err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

impl AppState {
    fn ff(&self) -> CmdResult<FfmpegInfo> {
        self.ff.lock().clone()
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Support {
    label: &'static str,
    codec: &'static str,
    available: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AppStatus {
    ffmpeg: Option<FfmpegInfo>,
    error: Option<String>,
    decoders: Vec<Support>,
    encoders: Vec<Support>,
}

fn status_of(ff: &Result<FfmpegInfo, String>) -> AppStatus {
    let dec = [
        ("DTS / DTS-HD", "dca"),
        ("Dolby Digital (AC-3)", "ac3"),
        ("Dolby Digital Plus", "eac3"),
        ("Dolby TrueHD", "truehd"),
        ("AAC", "aac"),
        ("FLAC", "flac"),
        ("PCM", "pcm_s24le"),
    ];
    let enc = [("WAV", "pcm_s16le"), ("FLAC", "flac"), ("MP3", "libmp3lame"), ("AAC", "aac")];
    let map = |list: &[(&'static str, &'static str)], has: &dyn Fn(&str) -> bool| {
        list.iter().map(|&(label, codec)| Support { label, codec, available: has(codec) }).collect()
    };
    match ff {
        Ok(f) => AppStatus {
            ffmpeg: Some(f.clone()),
            error: None,
            decoders: map(&dec, &|c| f.has_decoder(c)),
            encoders: map(&enc, &|c| f.has_encoder(c)),
        },
        Err(e) => AppStatus {
            ffmpeg: None,
            error: Some(e.clone()),
            decoders: map(&dec, &|_| false),
            encoders: map(&enc, &|_| false),
        },
    }
}

#[tauri::command]
fn app_status(state: State<AppState>) -> AppStatus {
    status_of(&state.ff.lock())
}

#[tauri::command]
fn set_ffmpeg_dir(state: State<AppState>, dir: Option<String>) -> AppStatus {
    let found = ffmpeg::locate(dir.as_deref().map(std::path::Path::new)).map_err(err);
    let mut ff = state.ff.lock();
    *ff = found;
    status_of(&ff)
}

#[tauri::command]
async fn probe_file(state: State<'_, AppState>, path: String) -> CmdResult<MediaInfo> {
    let ff = state.ff()?;
    tauri::async_runtime::spawn_blocking(move || probe::probe(&ff, &path).map_err(err))
        .await
        .map_err(err)?
}

#[tauri::command]
fn default_mix(ids: Vec<String>) -> MixSettings {
    mix::default_mix(&ids)
}

#[tauri::command]
fn mix_report(mix: MixSettings) -> GainReport {
    mix.report()
}

#[tauri::command]
fn list_presets(state: State<AppState>) -> CmdResult<Vec<Preset>> {
    state.presets.all().map_err(err)
}

#[tauri::command]
fn apply_preset(state: State<AppState>, name: String, ids: Vec<String>) -> CmdResult<MixSettings> {
    let p = state.presets.find(&name).map_err(err)?;
    Ok(presets::apply(&p, &ids))
}

#[tauri::command]
fn save_preset(state: State<AppState>, name: String, description: String, mix: MixSettings) -> CmdResult<Vec<Preset>> {
    state.presets.save(presets::from_mix(&name, &description, &mix)).map_err(err)?;
    state.presets.all().map_err(err)
}

#[tauri::command]
fn delete_preset(state: State<AppState>, name: String) -> CmdResult<Vec<Preset>> {
    state.presets.delete(&name).map_err(err)?;
    state.presets.all().map_err(err)
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct VideoArg {
    index: u32,
    fps: f64,
    offset: f64,
}

#[allow(clippy::too_many_arguments)]
#[tauri::command]
fn preview_load(
    state: State<AppState>,
    path: String,
    stream_index: u32,
    channels: usize,
    sample_rate: u32,
    duration: f64,
    mix: MixSettings,
    video: Option<VideoArg>,
) -> CmdResult<()> {
    let ff = state.ff()?;
    state.preview.set_mix(mix);
    state.preview.load(PreviewSource {
        ffmpeg: ff.ffmpeg,
        path,
        stream_index,
        channels,
        sample_rate,
        duration,
        video: video.map(|v| VideoParams { index: v.index, fps: v.fps, offset: v.offset }),
    });
    Ok(())
}

#[tauri::command]
fn preview_set_video(state: State<AppState>, enabled: bool) {
    state.preview.set_video_enabled(enabled);
}

#[tauri::command]
fn preview_set_video_quality(state: State<AppState>, full: bool) {
    state.preview.set_video_full_quality(full);
}

#[tauri::command]
fn preview_play(state: State<AppState>) -> CmdResult<()> {
    state.preview.play().map_err(err)
}

#[tauri::command]
fn preview_pause(state: State<AppState>) {
    state.preview.pause();
}

#[tauri::command]
fn preview_seek(state: State<AppState>, position: f64) -> CmdResult<()> {
    state.preview.seek(position).map_err(err)
}

#[tauri::command]
fn preview_stop(state: State<AppState>) {
    state.preview.stop();
}

#[tauri::command]
fn update_mix(state: State<AppState>, mix: MixSettings) {
    state.preview.set_mix(mix);
}

#[tauri::command]
async fn export_audio(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    request: export::ExportRequest,
) -> CmdResult<export::ExportResult> {
    let ff = state.ff()?;
    if state.exporting.swap(true, Ordering::SeqCst) {
        return Err("an export is already running".into());
    }
    state.cancel.store(false, Ordering::SeqCst);
    let cancel = state.cancel.clone();
    let exporting = state.exporting.clone();
    let res = tauri::async_runtime::spawn_blocking(move || {
        let r = export::run(&ff, &request, &cancel, |p| {
            let _ = app.emit("export:progress", p);
        });
        exporting.store(false, Ordering::SeqCst);
        r.map_err(err)
    })
    .await
    .map_err(err)?;
    res
}

#[tauri::command]
fn cancel_export(state: State<AppState>) {
    state.cancel.store(true, Ordering::SeqCst);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        // Latest preview video frame as JPEG: vframe://localhost/frame (http://vframe.localhost/frame on Windows).
        .register_uri_scheme_protocol("vframe", |ctx, _request| {
            let frame = ctx.app_handle().try_state::<AppState>().and_then(|s| s.preview.frame());
            let builder = tauri::http::Response::builder()
                .header("Cache-Control", "no-store")
                .header("Access-Control-Allow-Origin", "*");
            match frame {
                Some((seq, jpeg)) => builder
                    .header("Content-Type", "image/jpeg")
                    .header("X-Frame-Seq", seq.to_string())
                    .body(std::borrow::Cow::Owned(jpeg.as_ref().clone()))
                    .unwrap(),
                None => builder.status(404).body(std::borrow::Cow::Borrowed(&[][..])).unwrap(),
            }
        })
        .setup(|app| {
            let config_dir: PathBuf = app.path().app_config_dir().unwrap_or_else(|_| std::env::temp_dir());
            let preview = Preview::new();
            let frame_handle = app.handle().clone();
            preview.set_frame_notifier(Arc::new(move |seq| {
                let _ = frame_handle.emit("video:frame", seq);
            }));
            app.manage(AppState {
                ff: Mutex::new(ffmpeg::locate(None).map_err(err)),
                preview,
                cancel: Arc::new(AtomicBool::new(false)),
                exporting: Arc::new(AtomicBool::new(false)),
                presets: PresetStore::new(config_dir.join("presets.json")),
            });
            // Meter/position ticker for the UI.
            let handle = app.handle().clone();
            std::thread::spawn(move || loop {
                std::thread::sleep(Duration::from_millis(33));
                let state = handle.state::<AppState>();
                if let Some(t) = state.preview.tick() {
                    let _ = handle.emit("preview:tick", t);
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_status,
            set_ffmpeg_dir,
            probe_file,
            default_mix,
            mix_report,
            list_presets,
            apply_preset,
            save_preset,
            delete_preset,
            preview_load,
            preview_play,
            preview_pause,
            preview_seek,
            preview_stop,
            preview_set_video,
            preview_set_video_quality,
            update_mix,
            export_audio,
            cancel_export,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Channel Picker");
}
