export type SourceKind = "multichannel" | "stereo" | "mono";
export type GainMode = "safe" | "manual";

export interface ChannelInfo {
  id: string;
  label: string;
  role: string;
}

export interface AudioStream {
  index: number;
  track: number;
  codecName: string;
  codecLabel: string;
  profile: string | null;
  channels: number;
  channelLayout: string;
  layoutAssumed: boolean;
  channelInfo: ChannelInfo[];
  sampleRate: number;
  startTime: number;
  duration: number;
  bitRate: number | null;
  language: string | null;
  title: string | null;
  isDefault: boolean;
  decoderAvailable: boolean;
  kind: SourceKind;
  kindLabel: string;
}

export interface VideoInfo {
  index: number;
  codecName: string;
  width: number;
  height: number;
  aspect: number;
  fps: number;
  startTime: number;
}

export interface MediaInfo {
  path: string;
  formatName: string;
  formatLongName: string;
  duration: number;
  startTime: number;
  hasVideo: boolean;
  video: VideoInfo | null;
  audioStreams: AudioStream[];
}

export interface ChannelStrip {
  id: string;
  enabled: boolean;
  solo: boolean;
  gainDb: number;
  toLeft: number;
  toRight: number;
}

export interface MixSettings {
  channels: ChannelStrip[];
  masterGainDb: number;
  gainMode: GainMode;
}

export interface GainReport {
  worstCaseLeft: number;
  worstCaseRight: number;
  safeGainDb: number;
  outputGainDb: number;
  canClip: boolean;
  routes: string[];
}

export interface Preset {
  name: string;
  description: string;
  builtin: boolean;
}

export interface Tick {
  position: number;
  duration: number;
  playing: boolean;
  ended: boolean;
  inputPeaks: number[];
  outputPeaks: [number, number];
  clipped: boolean;
  error: string | null;
  /** 0..1 per log-spaced band, 40 Hz .. 16 kHz, of what the device just played. */
  spectrum: number[];
}

export interface Support {
  label: string;
  codec: string;
  available: boolean;
}

export interface AppStatus {
  ffmpeg: { ffmpeg: string; ffprobe: string; version: string } | null;
  error: string | null;
  decoders: Support[];
  encoders: Support[];
}

export type OutputFormat =
  | { kind: "wav"; bits: 16 | 24 | 32 }
  | { kind: "flac"; bits: 16 | 24 }
  | { kind: "mp3"; vbr: boolean }
  | { kind: "aac"; kbps: number };

export interface ExportRequest {
  input: string;
  streamIndex: number;
  output: string;
  format: OutputFormat;
  sampleRate: number | null;
  normalizeDbfs: number | null;
  alignToContainer: boolean;
  mix: MixSettings;
}

export interface ExportProgress {
  phase: "analyzing" | "rendering";
  fraction: number;
}

export interface Verification {
  ok: boolean;
  channels: number;
  channelLayout: string;
  codec: string;
  sampleRate: number;
  duration: number;
  expectedDuration: number;
  sourceDuration: number;
  durationDiffMs: number;
  notes: string[];
}

export interface ExportResult {
  output: string;
  formatLabel: string;
  description: string;
  framesWritten: number;
  paddingSeconds: number;
  appliedGainDb: number;
  peakDbfs: number;
  clippedSamples: number;
  verification: Verification;
}

/** The six 5.1 roles, in display order. */
export const ROLES_51 = ["FL", "FR", "FC", "LFE", "SL", "SR"] as const;
export const ROLE_LABELS: Record<string, string> = {
  FL: "Front Left",
  FR: "Front Right",
  FC: "Center",
  LFE: "LFE / Subwoofer",
  SL: "Surround Left",
  SR: "Surround Right",
};
