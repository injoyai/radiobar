import { ipc } from "../ipc";
import type {
  PlayMode,
  Scope,
  Station,
  ToastPayload,
  UiState,
} from "../state";
import { codecLine, countryName, flagEmoji, truncateName } from "../format";
import { icons } from "./icons";

export interface BarHandle {
  render(state: UiState): void;
  showToast(toast: ToastPayload): void;
}

const SCOPE_CYCLE: Scope[] = ["favorites", "list", "all"];
const MODE_CYCLE: PlayMode[] = ["sequential", "loop", "shuffle"];

const SCOPE_LABEL: Record<Scope, string> = {
  favorites: "收藏",
  list: "列表",
  all: "全部",
};

const MODE_LABEL: Record<PlayMode, string> = {
  sequential: "顺序",
  loop: "循环",
  shuffle: "随机",
};

const SCOPE_ICON: Record<Scope, string> = {
  favorites: icons["star-filled"],
  list: icons.list,
  all: icons.globe,
};

const MODE_ICON: Record<PlayMode, string> = {
  sequential: icons["mode-sequential"],
  loop: icons["mode-loop"],
  shuffle: icons["mode-shuffle"],
};

function element<T extends Element>(root: ParentNode, selector: string): T {
  const found = root.querySelector<T>(selector);
  if (!found) throw new Error(`missing element: ${selector}`);
  return found;
}

function run(promise: Promise<unknown>): void {
  void promise.catch(() => undefined);
}

function nextOf<T>(cycle: T[], value: T): T {
  return cycle[(cycle.indexOf(value) + 1) % cycle.length] as T;
}

export function mountBar(root: HTMLElement): BarHandle {
  const bar = document.createElement("header");
  bar.className = "bar";
  bar.innerHTML = `
    <div class="bar-top" data-tauri-drag-region>
      <div class="bar-info" data-tauri-drag-region>
        <span class="bar-flag"></span>
        <span class="bar-country"></span>
        <span class="bar-dot">·</span>
        <span class="bar-name"></span>
      </div>
      <span class="bar-codec"></span>
    </div>
    <div class="bar-controls">
      <button class="icon-btn" type="button" data-act="prev" title="上一台" aria-label="上一台">${icons.prev}</button>
      <button class="play-btn" type="button" data-act="play" title="播放" aria-label="播放">${icons.play}</button>
      <button class="icon-btn" type="button" data-act="next" title="下一台" aria-label="下一台">${icons.next}</button>
      <button class="icon-btn fav-btn" type="button" data-act="favorite" title="收藏" aria-label="收藏">${icons.star}</button>
      <div class="volume-wrap">
        <button class="icon-btn" type="button" data-act="mute" title="静音" aria-label="静音">${icons.volume}</button>
        <input class="volume-slider" type="range" min="0" max="100" step="1" value="70" aria-label="音量" />
      </div>
      <button class="icon-btn" type="button" data-act="scope" title="切换范围" aria-label="切换范围">${icons.list}</button>
      <button class="icon-btn" type="button" data-act="mode" title="切换模式" aria-label="切换模式">${icons["mode-sequential"]}</button>
      <button class="icon-btn" type="button" data-act="panel" title="展开面板" aria-label="展开面板">${icons["panel-toggle"]}</button>
      <button class="icon-btn pin-btn" type="button" data-act="pin" title="置顶" aria-label="置顶">${icons.pin}</button>
    </div>
  `;

  const topRow = element<HTMLDivElement>(bar, ".bar-top");
  const flagEl = element<HTMLSpanElement>(bar, ".bar-flag");
  const countryEl = element<HTMLSpanElement>(bar, ".bar-country");
  const dotEl = element<HTMLSpanElement>(bar, ".bar-dot");
  const nameEl = element<HTMLSpanElement>(bar, ".bar-name");
  const codecEl = element<HTMLSpanElement>(bar, ".bar-codec");
  const prevBtn = element<HTMLButtonElement>(bar, '[data-act="prev"]');
  const playBtn = element<HTMLButtonElement>(bar, '[data-act="play"]');
  const nextBtn = element<HTMLButtonElement>(bar, '[data-act="next"]');
  const favBtn = element<HTMLButtonElement>(bar, '[data-act="favorite"]');
  const muteBtn = element<HTMLButtonElement>(bar, '[data-act="mute"]');
  const volumeWrap = element<HTMLDivElement>(bar, ".volume-wrap");
  const slider = element<HTMLInputElement>(bar, ".volume-slider");
  const scopeBtn = element<HTMLButtonElement>(bar, '[data-act="scope"]');
  const modeBtn = element<HTMLButtonElement>(bar, '[data-act="mode"]');
  const panelBtn = element<HTMLButtonElement>(bar, '[data-act="panel"]');
  const pinBtn = element<HTMLButtonElement>(bar, '[data-act="pin"]');

  const toast = document.createElement("div");
  toast.className = "toast";
  root.appendChild(toast);
  let toastTimer: number | undefined;

  let currentState: UiState | null = null;
  let volumeTimer: number | undefined;

  function updateSliderFill(value: number): void {
    slider.style.setProperty("--fill", `${value}%`);
  }

  function applyVolume(raw: number): void {
    const value = Math.max(0, Math.min(100, Math.round(raw)));
    slider.value = String(value);
    updateSliderFill(value);
    if (volumeTimer !== undefined) window.clearTimeout(volumeTimer);
    volumeTimer = window.setTimeout(() => {
      run(ipc.setVolume(value));
    }, 100);
  }

  function renderStation(station: Station | null): void {
    const hasStation = station !== null;
    flagEl.hidden = !hasStation;
    countryEl.hidden = !hasStation;
    dotEl.hidden = !hasStation;

    if (!station) {
      nameEl.textContent = "未选择电台";
      nameEl.title = "";
      codecEl.textContent = "";
      codecEl.hidden = true;
      return;
    }

    flagEl.textContent = flagEmoji(station.country);
    countryEl.textContent = countryName(station.country);
    nameEl.textContent = truncateName(station.name, 22);
    nameEl.title = station.name;
    const codec = codecLine(station);
    codecEl.textContent = codec;
    codecEl.hidden = !codec;
  }

  function render(state: UiState): void {
    currentState = state;
    renderStation(state.current);

    playBtn.innerHTML = state.playing ? icons.pause : icons.play;
    playBtn.title = state.playing ? "暂停" : "播放";
    playBtn.setAttribute("aria-label", state.playing ? "暂停" : "播放");
    playBtn.classList.toggle("is-playing", state.playing);

    prevBtn.disabled = !state.canPrev;
    nextBtn.disabled = !state.canNext;

    const favId = state.current?.id;
    const isFav = !!favId && state.favoriteIds.includes(favId);
    favBtn.disabled = !favId;
    favBtn.innerHTML = isFav ? icons["star-filled"] : icons.star;
    favBtn.title = isFav ? "取消收藏" : "收藏";
    favBtn.setAttribute("aria-label", isFav ? "取消收藏" : "收藏");
    favBtn.setAttribute("aria-pressed", String(isFav));
    favBtn.classList.toggle("is-active", isFav);

    muteBtn.innerHTML = state.muted ? icons.muted : icons.volume;
    muteBtn.title = state.muted ? "取消静音" : "静音";
    muteBtn.setAttribute("aria-label", state.muted ? "取消静音" : "静音");
    muteBtn.classList.toggle("is-active", state.muted);

    if (document.activeElement !== slider) {
      slider.value = String(state.volume);
      updateSliderFill(state.volume);
    }

    const scopeLabel = SCOPE_LABEL[state.scope];
    scopeBtn.innerHTML = SCOPE_ICON[state.scope];
    scopeBtn.title = `范围：${scopeLabel}（点击切换）`;
    scopeBtn.setAttribute("aria-label", `切换范围，当前${scopeLabel}`);
    scopeBtn.dataset.scope = state.scope;

    const modeLabel = MODE_LABEL[state.mode];
    modeBtn.innerHTML = MODE_ICON[state.mode];
    modeBtn.title = `模式：${modeLabel}（点击切换）`;
    modeBtn.setAttribute("aria-label", `切换模式，当前${modeLabel}`);
    modeBtn.dataset.mode = state.mode;

    panelBtn.innerHTML = state.panelOpen ? icons.chevron : icons["panel-toggle"];
    panelBtn.title = state.panelOpen ? "收起面板" : "展开面板";
    panelBtn.setAttribute("aria-label", state.panelOpen ? "收起面板" : "展开面板");
    panelBtn.setAttribute("aria-expanded", String(state.panelOpen));

    pinBtn.classList.toggle("is-active", state.alwaysOnTop);
    pinBtn.title = state.alwaysOnTop ? "置顶：开" : "置顶：关";
    pinBtn.setAttribute("aria-pressed", String(state.alwaysOnTop));
  }

  bar.addEventListener("click", (event) => {
    const target = (event.target as Element).closest<HTMLElement>("[data-act]");
    if (!target) return;
    const state = currentState;
    const act = target.dataset.act;

    switch (act) {
      case "prev":
        run(ipc.prev());
        break;
      case "play":
        run(ipc.togglePlay());
        break;
      case "next":
        run(ipc.next());
        break;
      case "favorite":
        if (state?.current) run(ipc.toggleFavorite(state.current.id));
        break;
      case "mute":
        run(ipc.toggleMute());
        break;
      case "scope":
        if (state) run(ipc.setScope(nextOf(SCOPE_CYCLE, state.scope)));
        break;
      case "mode":
        if (state) run(ipc.setMode(nextOf(MODE_CYCLE, state.mode)));
        break;
      case "panel":
        if (state) run(ipc.setPanelOpen(!state.panelOpen));
        break;
      case "pin":
        if (state) run(ipc.setAlwaysOnTop(!state.alwaysOnTop));
        break;
      default:
        break;
    }
  });

  slider.addEventListener("input", () => {
    applyVolume(Number(slider.value));
  });

  volumeWrap.addEventListener(
    "wheel",
    (event) => {
      event.preventDefault();
      const step = event.deltaY < 0 ? 2 : -2;
      applyVolume(Number(slider.value) + step);
    },
    { passive: false },
  );

  topRow.addEventListener("dblclick", (event) => {
    if ((event.target as Element).closest("[data-act]")) return;
    if (currentState) run(ipc.setPanelOpen(!currentState.panelOpen));
  });

  function showToast(payload: ToastPayload): void {
    const level = payload.level ?? "info";
    toast.className = `toast toast-${level}`;
    toast.textContent = payload.text;
    // 强制回流，确保入场动画在连续 toast 时也能重播。
    void toast.offsetWidth;
    toast.classList.add("is-visible");
    if (toastTimer !== undefined) window.clearTimeout(toastTimer);
    toastTimer = window.setTimeout(() => {
      toast.classList.remove("is-visible");
    }, 2000);
  }

  root.appendChild(bar);

  return { render, showToast };
}
