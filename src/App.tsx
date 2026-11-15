import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { open, save } from "@tauri-apps/plugin-dialog";
import { api, baseName, dirName, ids } from "./lib/api";
import type {
  AppStatus,
  AudioStream,
  ChannelStrip as Strip,
  ExportProgress,
  ExportResult,
  GainReport,
  MediaInfo,
  MixSettings,
  Preset,
  Tick,
} from "./lib/types";
import { ROLES_51, ROLE_LABELS } from "./lib/types";
import { ImportPanel } from "./components/ImportPanel";
import { SourceInfo } from "./components/SourceInfo";
import { ChannelStrip, GhostStrip } from "./components/ChannelStrip";
import { RoutingMatrix } from "./components/RoutingMatrix";
import { MasterSection } from "./components/MasterSection";
import { Transport } from "./components/Transport";
import { PresetBar } from "./components/PresetBar";
import { ExportPanel, type ExportOptions } from "./components/ExportPanel";
import { Orb } from "./components/Orb";
import { Icon } from "./components/ui";
import { VideoPanel } from "./components/VideoPanel";

const MEDIA_EXTENSIONS = [
  "mkv", "mp4", "m4v", "mov", "avi", "ts", "m2ts", "mts", "webm", "wmv", "vob",
  "dts", "ac3", "eac3", "ec3", "thd", "mka", "m4a", "aac", "flac", "wav", "w64", "caf", "ogg", "opus", "mp3",
];

const EXT: Record<string, string> = { wav: "wav", flac: "flac", mp3: "mp3", aac: "m4a" };

/** Prefer the default track if it is decodable multichannel, otherwise the widest decodable one. */
function pickStream(info: MediaInfo): AudioStream | undefined {
  const ok = info.audioStreams.filter((s) => s.decoderAvailable);
  return (
    ok.find((s) => s.isDefault && s.kind === "multichannel") ??
    [...ok].sort((a, b) => b.channels - a.channels)[0] ??
    info.audioStreams[0]
  );
}

const msg = (e: unknown) => (typeof e === "string" ? e : e instanceof Error ? e.message : String(e));

export default function App() {
  const [status, setStatus] = useState<AppStatus | null>(null);
  const [media, setMedia] = useState<MediaInfo | null>(null);
  const [streamIndex, setStreamIndex] = useState<number | null>(null);
  const [mix, setMix] = useState<MixSettings | null>(null);
  const [report, setReport] = useState<GainReport | null>(null);
  const [presets, setPresets] = useState<Preset[]>([]);
  const [presetName, setPresetName] = useState<string | null>("ITU Downmix");
  const [tick, setTick] = useState<Tick | null>(null);
  const [clipLatched, setClipLatched] = useState(false);
  const [loading, setLoading] = useState(false);
  const [dragging, setDragging] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [videoOn, setVideoOn] = useState(true);
  const [videoFull, setVideoFull] = useState(true);

  const [exportOpts, setExportOpts] = useState<ExportOptions>({
    format: { kind: "wav", bits: 24 },
    sampleRate: null,
    normalize: false,
    normalizeDbfs: -1,
    align: true,
  });
  const [busy, setBusy] = useState(false);
  const [progress, setProgress] = useState<ExportProgress | null>(null);
  const [result, setResult] = useState<ExportResult | null>(null);
  const [exportError, setExportError] = useState<string | null>(null);

  const stream = media?.audioStreams.find((s) => s.index === streamIndex) ?? null;
  const presetRef = useRef(presetName);
  presetRef.current = presetName;

  // Startup + backend events.
  useEffect(() => {
    api.appStatus().then(setStatus).catch((e) => setError(msg(e)));
    api.listPresets().then(setPresets).catch((e) => setError(msg(e)));
    const unTick = listen<Tick>("preview:tick", (e) => {
      setTick(e.payload);
      if (e.payload.clipped) setClipLatched(true);
    });
    const unProg = listen<ExportProgress>("export:progress", (e) => setProgress(e.payload));
    return () => {
      unTick.then((f) => f());
      unProg.then((f) => f());
    };
  }, []);

  // Live mix -> preview engine + gain report.
  useEffect(() => {
    if (!mix) return;
    api.updateMix(mix).catch(() => {});
    api.mixReport(mix).then(setReport).catch(() => {});
  }, [mix]);

  const loadStream = useCallback(async (info: MediaInfo, s: AudioStream) => {
    await api.previewStop();
    const channelIds = ids(s);
    const preset = presetRef.current;
    const m = preset ? await api.applyPreset(preset, channelIds) : await api.defaultMix(channelIds);
    setStreamIndex(s.index);
    setMix(m);
    setTick(null);
    setClipLatched(false);
    setResult(null);
    setExportError(null);
    if (s.decoderAvailable) await api.previewLoad(info, s, m);
  }, []);

  const openPath = useCallback(
    async (path: string) => {
      setLoading(true);
      setError(null);
      try {
        const info = await api.probeFile(path);
        if (info.audioStreams.length === 0) throw new Error("This file has no audio tracks.");
        const s = pickStream(info)!;
        setMedia(info);
        setExportOpts((o) => ({ ...o, align: info.hasVideo }));
        await loadStream(info, s);
      } catch (e) {
        setError(msg(e));
      } finally {
        setLoading(false);
      }
    },
    [loadStream],
  );

  const openDialog = useCallback(async () => {
    const path = await open({
      multiple: false,
      directory: false,
      filters: [
        { name: "Video and audio", extensions: MEDIA_EXTENSIONS },
        { name: "All files", extensions: ["*"] },
      ],
    });
    if (typeof path === "string") openPath(path);
  }, [openPath]);

  // Drag & drop anywhere in the window.
  useEffect(() => {
    const un = getCurrentWebview().onDragDropEvent((e) => {
      const p = e.payload;
      if (p.type === "enter" || p.type === "over") setDragging(true);
      else if (p.type === "leave") setDragging(false);
      else if (p.type === "drop") {
        setDragging(false);
        if (p.paths[0]) openPath(p.paths[0]);
      }
    });
    return () => {
      un.then((f) => f());
    };
  }, [openPath]);

  const playing = !!tick?.playing;
  const togglePlay = useCallback(async () => {
    try {
      setError(null);
      if (playing) await api.previewPause();
      else await api.previewPlay();
    } catch (e) {
      setError(msg(e));
    }
  }, [playing]);

  // Space bar = play/pause (unless typing in a field).
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const t = e.target as HTMLElement;
      if (e.code !== "Space" || ["INPUT", "SELECT", "TEXTAREA", "BUTTON"].includes(t.tagName)) return;
      if (!stream?.decoderAvailable) return;
      e.preventDefault();
      togglePlay();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [togglePlay, stream]);

  const editMix = (next: MixSettings) => {
    setMix(next);
    setPresetName(null);
  };
  const editStrip = (i: number, patch: Partial<Strip>) => {
    if (!mix) return;
    editMix({ ...mix, channels: mix.channels.map((c, j) => (j === i ? { ...c, ...patch } : c)) });
  };
  const setAll = (enabled: boolean) => mix && editMix({ ...mix, channels: mix.channels.map((c) => ({ ...c, enabled, solo: false })) });

  const applyPreset = async (name: string) => {
    if (!stream) return;
    try {
      setMix(await api.applyPreset(name, ids(stream)));
      setPresetName(name);
    } catch (e) {
      setError(msg(e));
    }
  };
  const savePreset = async (name: string) => {
    if (!mix) return;
    try {
      setPresets(await api.savePreset(name, "", mix));
      setPresetName(name);
    } catch (e) {
      setError(msg(e));
      throw e;
    }
  };
  const deletePreset = async (name: string) => {
    try {
      setPresets(await api.deletePreset(name));
      setPresetName(null);
    } catch (e) {
      setError(msg(e));
    }
  };

  const doExport = async () => {
    if (!media || !stream || !mix) return;
    const ext = EXT[exportOpts.format.kind];
    const dir = dirName(media.path);
    const sep = media.path.includes("\\") ? "\\" : "/";
    const output = await save({
      defaultPath: `${dir}${dir ? sep : ""}${baseName(media.path)} - stereo mix.${ext}`,
      filters: [{ name: ext.toUpperCase(), extensions: [ext] }],
    });
    if (!output) return;
    setBusy(true);
    setResult(null);
    setExportError(null);
    setProgress({ phase: exportOpts.normalize ? "analyzing" : "rendering", fraction: 0 });
    try {
      const res = await api.exportAudio({
        input: media.path,
        streamIndex: stream.index,
        output,
        format: exportOpts.format,
        sampleRate: exportOpts.sampleRate,
        normalizeDbfs: exportOpts.normalize ? exportOpts.normalizeDbfs : null,
        alignToContainer: exportOpts.align,
        mix,
      });
      setResult(res);
    } catch (e) {
      const m = msg(e);
      if (m !== "cancelled") setExportError(m);
    } finally {
      setBusy(false);
      setProgress(null);
    }
  };

  const locateFfmpeg = async () => {
    const dir = await open({ directory: true, multiple: false, title: "Folder containing ffmpeg and ffprobe" });
    if (typeof dir === "string") setStatus(await api.setFfmpegDir(dir));
  };

  const anySolo = !!mix?.channels.some((c) => c.solo);
  const isHeard = useCallback((s: Strip) => (anySolo ? s.solo : s.enabled), [anySolo]);
  const heardCount = mix?.channels.filter(isHeard).length ?? 0;

  const ghostRoles = useMemo(() => {
    if (!stream || stream.kind === "multichannel") return [];
    const have = new Set(stream.channelInfo.map((c) => c.role));
    return ROLES_51.filter((r) => !have.has(r));
  }, [stream]);

  const peaks = tick?.inputPeaks ?? [];

  // Arrange the cards like the speakers around the listener when the layout is 5.1-shaped.
  const roles = stream?.channelInfo.map((c) => c.role) ?? [];
  const spatial =
    roles.length > 0 && roles.every((r) => (ROLES_51 as readonly string[]).includes(r)) && new Set(roles).size === roles.length;

  return (
    <div className="app">
      <header className="toolbar">
        <div className="brand">
          <span className="brand-glyph" aria-hidden>
            <Icon name="waveform" size={16} />
          </span>
          Channel Picker
        </div>
        {media && stream ? (
          <Transport
            key={`${media.path}#${stream.index}`}
            position={tick?.position ?? 0}
            duration={stream.duration}
            playing={playing}
            disabled={!stream.decoderAvailable}
            onToggle={togglePlay}
            onSeek={(p) => api.previewSeek(p).catch((e) => setError(msg(e)))}
          />
        ) : (
          <span />
        )}
        <div
          className={`status-chip ${status?.ffmpeg ? "" : "bad"}`}
          title={status?.ffmpeg?.version ?? status?.error ?? ""}
        >
          <span className="status-dot" />
          {status?.ffmpeg ? "ffmpeg ready" : status ? "ffmpeg missing" : "Starting…"}
        </div>
      </header>

      {!media || !stream || !mix ? (
        <ImportPanel status={status} dragging={dragging} loading={loading} error={error} onOpen={openDialog} onLocateFfmpeg={locateFfmpeg} />
      ) : (
        <div className={`shell ${dragging ? "dragging" : ""}`}>
          <aside className="pane sidebar">
            <SourceInfo
              media={media}
              stream={stream}
              onSelectStream={(i) => loadStream(media, media.audioStreams.find((s) => s.index === i)!)}
              onOpen={openDialog}
            />
            <PresetBar
              presets={presets}
              current={presetName}
              disabled={!stream}
              onApply={applyPreset}
              onSave={savePreset}
              onDelete={deletePreset}
            />
          </aside>

          <main className="pane main">
           <div className="main-inner">
            {(error || tick?.error) && (
              <div className="alert" role="alert">
                <Icon name="exclamation" className="icon" size={16} />
                <div className="grow">{error ?? tick?.error}</div>
                <button className="link" onClick={() => setError(null)}>
                  Dismiss
                </button>
              </div>
            )}
            {media.video && stream.decoderAvailable && (
              <VideoPanel
                video={media.video}
                enabled={videoOn}
                fullQuality={videoFull}
                onToggle={(on) => {
                  setVideoOn(on);
                  api.previewSetVideo(on).catch((e) => setError(msg(e)));
                }}
                onQuality={(full) => {
                  setVideoFull(full);
                  api.previewSetVideoQuality(full).catch((e) => setError(msg(e)));
                }}
              />
            )}
            <section aria-labelledby="channels-title">
              <div className="page-head" style={{ marginBottom: 10 }}>
                <div className="grow">
                  <h2 id="channels-title" className="t-title">
                    Channels
                  </h2>
                  <div className="t-caption">
                    {heardCount} of {mix.channels.length} in the mix
                    {anySolo ? " · solo is on" : ""}
                  </div>
                </div>
                <button className="btn plain" onClick={() => setAll(true)}>
                  Enable All
                </button>
                <button className="btn plain" onClick={() => setAll(false)}>
                  Mute All
                </button>
              </div>
              {stream.kind !== "multichannel" && (
                <p className="notice">
                  This track is {stream.kind}, so there are no separate Center, LFE or Surround channels to pick from.
                </p>
              )}
              {spatial && (
                <div className="room-labels" aria-hidden>
                  <span>Front</span>
                </div>
              )}
              <div className={`chan-grid ${spatial ? "spatial" : ""}`}>
                {stream.channelInfo.map((c, i) =>
                  mix.channels[i] ? (
                    <ChannelStrip
                      key={c.id}
                      info={c}
                      strip={mix.channels[i]}
                      heard={isHeard(mix.channels[i])}
                      peak={peaks[i] ?? 0}
                      area={spatial ? c.role : undefined}
                      onChange={(p) => editStrip(i, p)}
                    />
                  ) : null,
                )}
                {ghostRoles.map((r) => (
                  <GhostStrip key={r} id={r} label={ROLE_LABELS[r]} area={spatial ? r : undefined} />
                ))}
              </div>
              {spatial && (
                <div className="room-labels" aria-hidden style={{ marginTop: 6 }}>
                  <span>Rear</span>
                </div>
              )}
            </section>
            <RoutingMatrix channels={stream.channelInfo} strips={mix.channels} isHeard={isHeard} onChange={editStrip} />
           </div>
          </main>

          <aside className="pane inspector">
            <Orb tick={tick} />
            <MasterSection
              mix={mix}
              report={report}
              outputPeaks={tick?.outputPeaks ?? [0, 0]}
              clipLatched={clipLatched}
              onResetClip={() => setClipLatched(false)}
              onChange={(p) => editMix({ ...mix, ...p })}
            />
            <ExportPanel
              opts={exportOpts}
              setOpts={setExportOpts}
              sourceRate={stream.sampleRate}
              hasVideo={media.hasVideo}
              startOffset={Math.max(0, stream.startTime - media.startTime)}
              disabled={!stream.decoderAvailable || busy}
              busy={busy}
              progress={progress}
              result={result}
              error={exportError}
              onExport={doExport}
              onCancel={() => api.cancelExport()}
            />
          </aside>
        </div>
      )}
    </div>
  );
}
