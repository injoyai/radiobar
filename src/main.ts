import "./style.css";

import { getAudioController } from "./audio";
import { ipc, subscribe } from "./ipc";
import { getState, setState, type UiState } from "./state";
import { mountBar } from "./ui/bar";
import { mountPanel } from "./ui/panel";

const app = document.querySelector<HTMLDivElement>("#app");
if (!app) throw new Error("missing #app");

const bar = mountBar(app);
const panel = mountPanel(app);
const audio = getAudioController();

/**
 * 自动播放策略：启动阶段不直接 play，等用户第一次手势后再按真实
 * playing 状态同步一次音频；之后的 state-changed 正常同步。
 */
let gestureSeen = false;

function render(state: UiState): void {
  setState(state);
  bar.render(state);
  panel.render(state);
  audio.updateStation(state.current);
  audio.sync(
    state.playback,
    gestureSeen ? state.playing : false,
    state.volume,
    state.muted,
  );
}

function onFirstGesture(): void {
  if (gestureSeen) return;
  gestureSeen = true;
  document.removeEventListener("pointerdown", onFirstGesture, true);
  document.removeEventListener("keydown", onFirstGesture, true);
  const state = getState();
  if (state) {
    audio.sync(state.playback, state.playing, state.volume, state.muted);
  }
}

document.addEventListener("pointerdown", onFirstGesture, true);
document.addEventListener("keydown", onFirstGesture, true);

async function boot(): Promise<void> {
  try {
    const initial = await ipc.getState();
    render(initial);
  } catch {
    bar.showToast({ level: "error", text: "无法连接播放器后端" });
  }

  subscribe({
    onState: render,
    onToast: (toast) => {
      bar.showToast(toast);
      panel.onToast(toast);
    },
    onSyncProgress: (progress) => {
      panel.onSyncProgress(progress);
    },
  });
}

void boot();
