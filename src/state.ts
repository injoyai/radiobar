export type PlayMode = "sequential" | "loop" | "shuffle";
export type Scope = "favorites" | "list" | "all";

export interface Station {
  id: string;
  name: string;
  /** 两位国家码，如 "CN"。 */
  country: string;
  urlResolved: string;
  url: string;
  codec: string;
  bitrate: number;
  hls: boolean;
  votes: number;
  tags: string[];
  clickcount: number;
  lastOk: boolean;
}

export interface ListFilter {
  query: string;
  country: string | null;
}

export interface PlaybackRequest {
  token: number;
  url: string;
}

export interface UiState {
  current: Station | null;
  playing: boolean;
  volume: number;
  muted: boolean;
  mode: PlayMode;
  scope: Scope;
  alwaysOnTop: boolean;
  panelOpen: boolean;
  favoriteIds: string[];
  lastSync: string | null;
  filter: ListFilter;
  playback: PlaybackRequest;
  canNext: boolean;
  canPrev: boolean;
  autoplayOnLaunch: boolean;
  autoNextOnFailure: boolean;
  /** 面板展开时朝上长；前端据此反转 DOM 顺序，让折叠条钉在原地。 */
  panelGrowsUp: boolean;
  /** 动作 id -> 组合键，如 `{ prev: "control+ArrowLeft" }`。 */
  shortcuts: Record<string, string>;
}

export interface CountryInfo {
  code: string;
  nameZh: string;
  nameEn: string;
  count: number;
}

export interface SearchPage {
  total: number;
  stations: Station[];
}

export type ToastLevel = "info" | "warn" | "error";

export interface ToastPayload {
  level: ToastLevel;
  text: string;
}

export interface SyncProgress {
  phase: string;
  done: number;
  total: number;
}

export type SettingKey = "autoplayOnLaunch" | "autoNextOnFailure";

/** 可改键的动作，顺序即设置页展示顺序。 */
export type ShortcutAction = "prev" | "next" | "volumeUp" | "volumeDown" | "playPause";

export const SHORTCUT_LABELS: Record<ShortcutAction, string> = {
  prev: "上一个电台",
  next: "下一个电台",
  volumeUp: "音量 +",
  volumeDown: "音量 −",
  playPause: "播放 / 暂停",
};

let latestState: UiState | null = null;

/** 返回最近一次收到的 UiState（尚未启动时为 null）。 */
export function getState(): UiState | null {
  return latestState;
}

/** 保存最近一次收到的 UiState。 */
export function setState(state: UiState): void {
  latestState = state;
}
