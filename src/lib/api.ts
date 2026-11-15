import { invoke } from "@tauri-apps/api/core";
import type {
  AppStatus,
  AudioStream,
  ExportRequest,
  ExportResult,
  GainReport,
  MediaInfo,
  MixSettings,
  Preset,
} from "./types";

export const api = {
  appStatus: () => invoke<AppStatus>("app_status"),
  setFfmpegDir: (dir: string | null) => invoke<AppStatus>("set_ffmpeg_dir", { dir }),
  probeFile: (path: string) => invoke<MediaInfo>("probe_file", { path }),
  defaultMix: (ids: string[]) => invoke<MixSettings>("default_mix", { ids }),
  mixReport: (mix: MixSettings) => invoke<GainReport>("mix_report", { mix }),
  listPresets: () => invoke<Preset[]>("list_presets"),
  applyPreset: (name: string, ids: string[]) => invoke<MixSettings>("apply_preset", { name, ids }),
  savePreset: (name: string, description: string, mix: MixSettings) =>
    invoke<Preset[]>("save_preset", { name, description, mix }),
  deletePreset: (name: string) => invoke<Preset[]>("delete_preset", { name }),
  previewLoad: (media: MediaInfo, s: AudioStream, mix: MixSettings) =>
    invoke<void>("preview_load", {
      path: media.path,
      streamIndex: s.index,
      channels: s.channels,
      sampleRate: s.sampleRate,
      duration: s.duration,
      mix,
      video: media.video
        ? { index: media.video.index, fps: media.video.fps, offset: s.startTime - media.startTime }
        : null,
    }),
  previewSetVideo: (enabled: boolean) => invoke<void>("preview_set_video", { enabled }),
  previewSetVideoQuality: (full: boolean) => invoke<void>("preview_set_video_quality", { full }),
  previewPlay: () => invoke<void>("preview_play"),
  previewPause: () => invoke<void>("preview_pause"),
  previewSeek: (position: number) => invoke<void>("preview_seek", { position }),
  previewStop: () => invoke<void>("preview_stop"),
  updateMix: (mix: MixSettings) => invoke<void>("update_mix", { mix }),
  exportAudio: (request: ExportRequest) => invoke<ExportResult>("export_audio", { request }),
  cancelExport: () => invoke<void>("cancel_export"),
};

export const ids = (s: AudioStream) => s.channelInfo.map((c) => c.id);

export function formatTime(sec: number): string {
  if (!isFinite(sec) || sec < 0) sec = 0;
  const h = Math.floor(sec / 3600);
  const m = Math.floor((sec % 3600) / 60);
  const s = Math.floor(sec % 60);
  const mm = String(m).padStart(2, "0");
  const ss = String(s).padStart(2, "0");
  return h > 0 ? `${h}:${mm}:${ss}` : `${m}:${ss}`;
}

export const toDb = (x: number) => (x <= 1e-9 ? -Infinity : 20 * Math.log10(x));
export const fmtDb = (db: number, digits = 1) =>
  !isFinite(db) ? "−∞ dB" : `${db > 0 ? "+" : db < 0 ? "−" : ""}${Math.abs(db).toFixed(digits)} dB`;

export function baseName(path: string): string {
  const name = path.split(/[\\/]/).pop() ?? path;
  return name.replace(/\.[^.]+$/, "");
}

export function dirName(path: string): string {
  const i = Math.max(path.lastIndexOf("\\"), path.lastIndexOf("/"));
  return i >= 0 ? path.slice(0, i) : "";
}
