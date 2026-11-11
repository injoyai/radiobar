import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import type {
  CountryInfo,
  ListFilter,
  PlayMode,
  Scope,
  SearchPage,
  SettingKey,
  SyncProgress,
  ToastPayload,
  UiState,
} from "./state";

/** 后端命令的类型化封装，参数名与 Rust camelCase 契约保持一致。 */
export const ipc = {
  getState: () => invoke<UiState>("get_state"),
  togglePlay: () => invoke<void>("toggle_play"),
  playStation: (id: string) => invoke<void>("play_station", { id }),
  next: () => invoke<void>("next"),
  prev: () => invoke<void>("prev"),
  setVolume: (volume: number) => invoke<void>("set_volume", { volume }),
  toggleMute: () => invoke<void>("toggle_mute"),
  setMode: (mode: PlayMode) => invoke<void>("set_mode", { mode }),
  setScope: (scope: Scope) => invoke<void>("set_scope", { scope }),
  setAlwaysOnTop: (value: boolean) => invoke<void>("set_always_on_top", { value }),
  setPanelOpen: (open: boolean) => invoke<void>("set_panel_open", { open }),
  setFilter: (filter: ListFilter) => invoke<void>("set_filter", { filter }),
  setSetting: (key: SettingKey, value: boolean) =>
    invoke<void>("set_setting", { key, value }),
  searchStations: (offset: number, limit: number) =>
    invoke<SearchPage>("search_stations", { offset, limit }),
  getCountries: () => invoke<CountryInfo[]>("get_countries"),
  toggleFavorite: (id: string) => invoke<void>("toggle_favorite", { id }),
  reportPlaybackError: (message: string) =>
    invoke<void>("report_playback_error", { message }),
  reportPlaybackStarted: () => invoke<void>("report_playback_started"),
  syncLibrary: () => invoke<void>("sync_library"),
  quit: () => invoke<void>("quit"),
};

export interface EventListeners {
  onState?: (state: UiState) => void;
  onToast?: (toast: ToastPayload) => void;
  onSyncProgress?: (progress: SyncProgress) => void;
}

/** 订阅 Rust 推送的事件，返回取消订阅函数。 */
export function subscribe(listeners: EventListeners): () => void {
  const pending: Promise<UnlistenFn>[] = [];
  if (listeners.onState) {
    const onState = listeners.onState;
    pending.push(
      listen<UiState>("state-changed", (event) => {
        onState(event.payload);
      }),
    );
  }
  if (listeners.onToast) {
    const onToast = listeners.onToast;
    pending.push(
      listen<ToastPayload>("toast", (event) => {
        onToast(event.payload);
      }),
    );
  }
  if (listeners.onSyncProgress) {
    const onSyncProgress = listeners.onSyncProgress;
    pending.push(
      listen<SyncProgress>("sync-progress", (event) => {
        onSyncProgress(event.payload);
      }),
    );
  }

  let disposed = false;
  const unlisteners: UnlistenFn[] = [];
  void Promise.allSettled(pending).then((results) => {
    const fns = results
      .filter(
        (result): result is PromiseFulfilledResult<UnlistenFn> =>
          result.status === "fulfilled",
      )
      .map((result) => result.value);
    if (disposed) {
      fns.forEach((unlisten) => unlisten());
    } else {
      unlisteners.push(...fns);
    }
  });

  return () => {
    disposed = true;
    unlisteners.forEach((unlisten) => unlisten());
    unlisteners.length = 0;
  };
}
