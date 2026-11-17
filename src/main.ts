import "./style.css";

import { getAudioController } from "./audio";
import { ipc, subscribe } from "./ipc";
import { setState, type UiState } from "./state";
import { mountBar } from "./ui/bar";
import { mountPanel } from "./ui/panel";

const appRoot = document.querySelector<HTMLDivElement>("#app");
if (!appRoot) throw new Error("missing #app");
const app: HTMLDivElement = appRoot;

const bar = mountBar(app);
const panel = mountPanel(app);
const audio = getAudioController();

function render(state: UiState): void {
  setState(state);
  // 面板朝上长时反转 DOM 顺序，折叠条才会钉在原地而不是被顶到窗口顶部。
  app.classList.toggle("expand-up", state.panelOpen && state.panelGrowsUp);
  bar.render(state);
  panel.render(state);
  audio.updateStation(state.current);
  audio.sync(state.playback, state.playing, state.volume, state.muted);
}

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
