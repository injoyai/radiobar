import type Hls from "hls.js";

import { ipc } from "./ipc";
import { getState, type PlaybackRequest, type Station } from "./state";
import { countryName } from "./format";

/** 去掉 query / fragment 后，路径部分以 `.m3u8` 结尾（大小写不敏感）。 */
export function isHlsUrl(url: string): boolean {
  const withoutFragment = url.split("#")[0] ?? "";
  const withoutQuery = withoutFragment.split("?")[0] ?? "";
  return withoutQuery.toLowerCase().endsWith(".m3u8");
}

/** 8 秒看门狗判定：未上报过、已超时且尚未进入 HAVE_FUTURE_DATA。 */
export function shouldReportStall(
  readyState: number,
  elapsedMs: number,
  reported: boolean,
): boolean {
  return !reported && elapsedMs >= 8000 && readyState < 3;
}

const STALL_TIMEOUT_MS = 8000;

/** 连续 ended 自动续播达到上限后不再重试，上报后端走既有失败处理。 */
export const MAX_ENDED_RESUMES = 3;

/** 直播流被服务端正常关闭（ended）时的自愈闸门：仅在期望播放且是未托管的直连流时续播。 */
export function shouldResumeOnEnded(
  wantPlaying: boolean,
  hasSource: boolean,
  directStream: boolean,
  consecutiveResumes: number,
): boolean {
  return (
    wantPlaying && hasSource && directStream && consecutiveResumes < MAX_ENDED_RESUMES
  );
}

export class AudioController {
  readonly audio: HTMLAudioElement;

  private token = -1;
  private hls: Hls | null = null;
  private loadStartedAt: number | null = null;
  private stallTimer: number | undefined;
  private stallReported = false;
  private wantPlaying = false;
  private endedResumes = 0;

  constructor() {
    this.audio = document.createElement("audio");
    this.audio.preload = "none";
    this.audio.style.display = "none";
    document.body.appendChild(this.audio);
    this.bindAudioEvents();
    this.bindMediaSession();
  }

  /** 同步播放源、播放态与音量。token 变化才换源。 */
  sync(
    playback: PlaybackRequest,
    playing: boolean,
    volume: number,
    muted: boolean,
  ): void {
    const safeVolume = Number.isFinite(volume) ? Math.max(0, Math.min(100, volume)) : 100;
    this.audio.volume = safeVolume / 100;
    this.audio.muted = muted;
    this.wantPlaying = playing;

    if (playback.token !== this.token) {
      this.token = playback.token;
      void this.switchSource(playback.url, playing);
      return;
    }

    if (playing) {
      if (this.audio.paused) this.playSafely();
    } else if (!this.audio.paused) {
      this.audio.pause();
    }
  }

  /** 尽量把当前电台信息写进系统媒体会话。 */
  updateStation(station: Station | null): void {
    if (!("mediaSession" in navigator)) return;
    try {
      navigator.mediaSession.metadata = station
        ? new MediaMetadata({
            title: station.name,
            artist: countryName(station.country),
            album: "radiobar",
          })
        : null;
    } catch {
      // 媒体会话只是尽力而为，失败不影响播放。
    }
  }

  private async switchSource(url: string, playing: boolean): Promise<void> {
    this.destroyHls();
    this.resetWatchdog();
    this.endedResumes = 0;
    this.audio.pause();

    if (!url) {
      this.audio.removeAttribute("src");
      this.audio.load();
      return;
    }

    if (!isHlsUrl(url)) {
      this.audio.src = url;
      if (playing) this.playSafely();
      return;
    }

    const token = this.token;
    try {
      const { default: HlsCtor } = await import("hls.js");
      if (token !== this.token) return;

      const hls = new HlsCtor();
      this.hls = hls;
      hls.attachMedia(this.audio);
      hls.on(HlsCtor.Events.ERROR, (_event, data) => {
        if (this.hls !== hls) return;
        const detail = data?.details ?? data?.type ?? "unknown";
        void ipc.reportPlaybackError(`hls:${String(detail)}`).catch(() => undefined);
      });
      hls.loadSource(url);
      if (playing) this.playSafely();
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      void ipc.reportPlaybackError(`hls:${message}`).catch(() => undefined);
    }
  }

  private playSafely(): void {
    const result = this.audio.play();
    if (result) {
      void result.catch(() => {
        // 自动播放策略拒绝时不视为播放错误，等待下一次用户手势。
      });
    }
  }

  private destroyHls(): void {
    if (this.hls) {
      this.hls.destroy();
      this.hls = null;
    }
  }

  private resetWatchdog(): void {
    if (this.stallTimer !== undefined) {
      window.clearTimeout(this.stallTimer);
      this.stallTimer = undefined;
    }
    this.loadStartedAt = null;
    this.stallReported = false;
  }

  private bindAudioEvents(): void {
    this.audio.addEventListener("playing", () => {
      this.endedResumes = 0;
      if ("mediaSession" in navigator) {
        navigator.mediaSession.playbackState = "playing";
      }
      void ipc.reportPlaybackStarted().catch(() => undefined);
    });

    this.audio.addEventListener("pause", () => {
      if ("mediaSession" in navigator) {
        navigator.mediaSession.playbackState = "paused";
      }
    });

    this.audio.addEventListener("error", () => {
      const error = this.audio.error;
      void ipc
        .reportPlaybackError(error?.message || "audio error")
        .catch(() => undefined);
    });

    // 服务端关闭直播连接时元素会静默进入 ended，此前无监听导致永久无声。
    this.audio.addEventListener("ended", () => {
      if (
        shouldResumeOnEnded(
          this.wantPlaying,
          Boolean(this.audio.src),
          this.hls === null,
          this.endedResumes,
        )
      ) {
        this.endedResumes += 1;
        this.playSafely();
        return;
      }
      // 多次续播仍立即结束：上报后端，由既有失败处理（按设置重试/切台/暂停）。
      if (this.wantPlaying && this.hls === null) {
        void ipc.reportPlaybackError("stream ended").catch(() => undefined);
      }
    });

    this.audio.addEventListener("loadstart", () => this.armStallWatchdog());
    // 中途断流不会再次触发 loadstart，waiting/stalled 时重装看门狗才能被发现。
    this.audio.addEventListener("waiting", () => this.armStallWatchdog());
    this.audio.addEventListener("stalled", () => this.armStallWatchdog());
  }

  /** 8 秒看门狗：装表点为 loadstart 与中途 waiting/stalled，恢复后自然失效。 */
  private armStallWatchdog(): void {
    this.loadStartedAt = Date.now();
    this.stallReported = false;
    if (this.stallTimer !== undefined) window.clearTimeout(this.stallTimer);
    this.stallTimer = window.setTimeout(() => {
      if (this.loadStartedAt === null) return;
      const elapsed = Date.now() - this.loadStartedAt;
      if (this.audio.paused || !this.wantPlaying) return;
      if (
        !shouldReportStall(this.audio.readyState, elapsed, this.stallReported)
      ) {
        return;
      }
      if (this.hls) {
        // hls.js 有自己的恢复流程，这里只上报，由后端决定切台。
        this.stallReported = true;
        void ipc.reportPlaybackError("stalled").catch(() => undefined);
        return;
      }
      // 直连直播流中途断流：静默重连，等效用户手动「暂停再继续」；
      // 重连失败会自然触发 error / ended，交给既有失败处理。
      this.resetWatchdog();
      this.audio.load();
      this.playSafely();
    }, STALL_TIMEOUT_MS);
  }

  private bindMediaSession(): void {
    if (!("mediaSession" in navigator)) return;
    const toggleIf = (shouldBePlaying: boolean) => {
      const state = getState();
      if (state && state.playing !== shouldBePlaying) {
        void ipc.togglePlay().catch(() => undefined);
      }
    };
    const actions: Array<[MediaSessionAction, () => void]> = [
      ["play", () => toggleIf(true)],
      ["pause", () => toggleIf(false)],
      ["nexttrack", () => void ipc.next().catch(() => undefined)],
      ["previoustrack", () => void ipc.prev().catch(() => undefined)],
    ];
    for (const [action, handler] of actions) {
      try {
        navigator.mediaSession.setActionHandler(action, handler);
      } catch {
        // 某些平台不支持个别 action，忽略即可。
      }
    }
  }
}

let controller: AudioController | null = null;

/** 单例音频控制器（首次调用时创建隐藏的 <audio>）。 */
export function getAudioController(): AudioController {
  if (!controller) controller = new AudioController();
  return controller;
}
