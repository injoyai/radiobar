import { ipc } from "../ipc";
import {
  getState,
  type CountryInfo,
  type SettingKey,
  type Station,
  type SyncProgress,
  type ToastPayload,
  type UiState,
} from "../state";
import { codecLine, countryName, flagEmoji, formatLastSync, progressText, truncateName } from "../format";
import { icons } from "./icons";

export interface PanelHandle {
  render(state: UiState): void;
  onSyncProgress(progress: SyncProgress): void;
  onToast(toast: ToastPayload): void;
}

const PAGE_SIZE = 50;

function element<T extends Element>(root: ParentNode, selector: string): T {
  const found = root.querySelector<T>(selector);
  if (!found) throw new Error(`missing element: ${selector}`);
  return found;
}

function run(promise: Promise<unknown>): void {
  void promise.catch(() => undefined);
}

function message(text: string, className = "list-empty"): HTMLDivElement {
  const div = document.createElement("div");
  div.className = className;
  div.textContent = text;
  return div;
}

export function mountPanel(root: HTMLElement): PanelHandle {
  const panel = document.createElement("section");
  panel.className = "panel";
  panel.hidden = true;
  panel.innerHTML = `
    <div class="panel-view panel-browse">
      <div class="panel-head">
        <div class="search-box">
          ${icons.search}
          <input class="search-input" type="search" placeholder="搜索电台…" aria-label="搜索电台" autocomplete="off" spellcheck="false" />
        </div>
        <button class="icon-btn" type="button" data-act="close" title="关闭面板" aria-label="关闭面板">${icons.close}</button>
      </div>
      <div class="panel-filters">
        <select class="country-select" aria-label="国家筛选"></select>
        <button class="fav-toggle" type="button" data-act="favorites" aria-pressed="false">
          ${icons.star}<span>只看收藏</span>
        </button>
      </div>
      <div class="station-list" role="list"></div>
      <div class="pager">
        <button class="pager-btn" type="button" data-act="prev-page" aria-label="上一页">${icons.back}<span>上一页</span></button>
        <span class="page-label">1 / 1</span>
        <button class="pager-btn" type="button" data-act="next-page" aria-label="下一页"><span>下一页</span><span class="chevron-right">${icons.chevron}</span></button>
      </div>
      <div class="panel-foot">
        <span class="foot-info"></span>
        <div class="foot-actions">
          <button class="foot-btn sync-btn" type="button" data-act="sync" title="更新电台库">${icons.refresh}<span class="sync-label">更新</span></button>
          <button class="icon-btn" type="button" data-act="settings" title="设置" aria-label="设置">${icons.settings}</button>
        </div>
      </div>
    </div>
    <div class="panel-view panel-settings" hidden>
      <div class="settings-head">
        <button class="icon-btn" type="button" data-act="settings-back" title="返回列表" aria-label="返回列表">${icons.back}</button>
        <span class="settings-title">设置</span>
      </div>
      <label class="setting-row">
        <span class="setting-text">
          <span class="setting-name">启动时自动播放</span>
          <span class="setting-hint">启动后直接播放上次的电台</span>
        </span>
        <input class="setting-check" type="checkbox" data-setting="autoplayOnLaunch" />
      </label>
      <label class="setting-row">
        <span class="setting-text">
          <span class="setting-name">播放失败自动切台</span>
          <span class="setting-hint">当前电台播放失败时自动切到下一台</span>
        </span>
        <input class="setting-check" type="checkbox" data-setting="autoNextOnFailure" />
      </label>
    </div>
  `;

  const browseView = element<HTMLDivElement>(panel, ".panel-browse");
  const settingsView = element<HTMLDivElement>(panel, ".panel-settings");
  const searchInput = element<HTMLInputElement>(panel, ".search-input");
  const countrySelect = element<HTMLSelectElement>(panel, ".country-select");
  const favToggle = element<HTMLButtonElement>(panel, '[data-act="favorites"]');
  const list = element<HTMLDivElement>(panel, ".station-list");
  const pageLabel = element<HTMLSpanElement>(panel, ".page-label");
  const prevPageBtn = element<HTMLButtonElement>(panel, '[data-act="prev-page"]');
  const nextPageBtn = element<HTMLButtonElement>(panel, '[data-act="next-page"]');
  const footInfo = element<HTMLSpanElement>(panel, ".foot-info");
  const syncBtn = element<HTMLButtonElement>(panel, ".sync-btn");
  const syncLabel = element<HTMLSpanElement>(panel, ".sync-label");
  const autoplayCheck = element<HTMLInputElement>(panel, '[data-setting="autoplayOnLaunch"]');
  const autoNextCheck = element<HTMLInputElement>(panel, '[data-setting="autoNextOnFailure"]');

  let currentState: UiState | null = null;
  let countries: CountryInfo[] = [];
  let countriesLoaded = false;
  let stations: Station[] = [];
  let total = 0;
  let page = 0;
  let loading = false;
  let requestSeq = 0;
  let filterSig = "";
  let searchTimer: number | undefined;
  let searchDirty = false;
  let composing = false;
  let wasOpen = false;
  let view: "browse" | "settings" = "browse";
  let syncing = false;
  let syncText = "";

  function applyView(): void {
    browseView.hidden = view !== "browse";
    settingsView.hidden = view !== "settings";
  }

  function updateFooter(): void {
    const sync = currentState?.lastSync ?? null;
    footInfo.textContent = `共 ${total} 台 · 上次同步 ${formatLastSync(sync)}`;
  }

  function updatePager(): void {
    const pages = Math.max(1, Math.ceil(total / PAGE_SIZE));
    pageLabel.textContent = `${page + 1} / ${pages}`;
    prevPageBtn.disabled = page <= 0;
    nextPageBtn.disabled = (page + 1) * PAGE_SIZE >= total;
  }

  function buildRow(station: Station, favoriteIds: Set<string>, currentId: string | undefined): HTMLElement {
    const row = document.createElement("div");
    row.className = "station-row";
    row.dataset.id = station.id;
    row.title = `${countryName(station.country)} · ${station.name}`;
    if (station.id === currentId) row.classList.add("is-current");
    if (!station.lastOk) row.classList.add("is-unavailable");

    const eq = document.createElement("span");
    eq.className = "row-eq";
    eq.setAttribute("aria-hidden", "true");

    const flag = document.createElement("span");
    flag.className = "row-flag";
    flag.textContent = flagEmoji(station.country);

    const main = document.createElement("div");
    main.className = "row-main";

    const name = document.createElement("span");
    name.className = "row-name";
    name.textContent = truncateName(station.name, 28);
    main.appendChild(name);

    const tags = document.createElement("span");
    tags.className = "row-tags";
    const tagText = station.tags
      .slice(0, 2)
      .map((tag) => `#${tag}`)
      .join(" ");
    tags.textContent = tagText || countryName(station.country);
    main.appendChild(tags);

    const meta = document.createElement("div");
    meta.className = "row-meta";
    const codec = document.createElement("span");
    codec.className = "row-codec";
    codec.textContent = codecLine(station);
    meta.appendChild(codec);
    if (!station.lastOk) {
      const unavailable = document.createElement("span");
      unavailable.className = "row-unavailable";
      unavailable.textContent = "暂不可用";
      meta.appendChild(unavailable);
    }

    const favorite = favoriteIds.has(station.id);
    const star = document.createElement("button");
    star.type = "button";
    star.className = "star-btn";
    star.dataset.id = station.id;
    star.classList.toggle("is-active", favorite);
    star.title = favorite ? "取消收藏" : "收藏";
    star.setAttribute("aria-label", favorite ? "取消收藏" : "收藏");
    star.setAttribute("aria-pressed", String(favorite));
    star.innerHTML = favorite ? icons["star-filled"] : icons.star;

    row.append(eq, flag, main, meta, star);
    return row;
  }

  function renderRows(): void {
    list.replaceChildren();
    if (loading) {
      list.appendChild(message("加载中…"));
      return;
    }
    if (stations.length === 0) {
      list.appendChild(message("没有匹配的电台"));
      return;
    }
    const favoriteIds = new Set(currentState?.favoriteIds ?? []);
    const currentId = currentState?.current?.id;
    for (const station of stations) {
      list.appendChild(buildRow(station, favoriteIds, currentId));
    }
  }

  function renderCountries(): void {
    const selected = getState()?.filter.country ?? "";
    countrySelect.replaceChildren();
    const all = document.createElement("option");
    all.value = "";
    all.textContent = "全部";
    countrySelect.appendChild(all);

    for (const country of countries) {
      const option = document.createElement("option");
      option.value = country.code;
      option.textContent = `${country.nameZh || country.nameEn || country.code} (${country.count})`;
      countrySelect.appendChild(option);
    }
    countrySelect.value = selected;
    if (countrySelect.value !== selected) countrySelect.value = "";
  }

  async function loadCountries(): Promise<void> {
    try {
      const result = await ipc.getCountries();
      countries = [...result].sort((a, b) => b.count - a.count);
      countriesLoaded = true;
      renderCountries();
    } catch {
      // 国家列表加载失败不阻塞搜索；下次打开面板再试。
    }
  }

  async function loadPage(target: number): Promise<void> {
    const seq = ++requestSeq;
    loading = true;
    renderRows();
    try {
      const result = await ipc.searchStations(target * PAGE_SIZE, PAGE_SIZE);
      if (seq !== requestSeq) return;
      page = target;
      total = result.total;
      stations = result.stations;
      loading = false;
      renderRows();
      updatePager();
      updateFooter();
      list.scrollTop = 0;
    } catch {
      if (seq !== requestSeq) return;
      loading = false;
      stations = [];
      total = 0;
      renderRows();
      updatePager();
      updateFooter();
    }
  }

  async function applyFilter(): Promise<void> {
    const state = getState();
    if (!state) return;
    const query = searchInput.value.trim();
    const country = countrySelect.value || null;
    filterSig = `${query}\u0000${country ?? ""}`;
    page = 0;
    try {
      await ipc.setFilter({ query, country });
    } catch {
      return;
    }
    searchDirty = false;
    await loadPage(0);
  }

  function scheduleSearch(): void {
    if (searchTimer !== undefined) window.clearTimeout(searchTimer);
    searchTimer = window.setTimeout(() => {
      void applyFilter();
    }, 200);
  }

  function updateSyncButton(): void {
    syncBtn.disabled = syncing;
    syncBtn.classList.toggle("is-syncing", syncing);
    syncLabel.textContent = syncing ? syncText || "更新中…" : "更新";
  }

  function finishSync(): void {
    if (!syncing) return;
    syncing = false;
    syncText = "";
    updateSyncButton();
    if (currentState?.panelOpen) void loadPage(page);
  }

  function startSync(): void {
    if (syncing) return;
    syncing = true;
    syncText = "";
    updateSyncButton();
    void ipc
      .syncLibrary()
      .catch(() => undefined)
      .finally(() => {
        if (syncing) finishSync();
      });
  }

  function render(state: UiState): void {
    currentState = state;
    panel.hidden = !state.panelOpen;

    // scope 决定后端返回哪个集合；收藏范围内取消收藏也要让该行立刻消失，
    // 所以把收藏 id 一并纳入签名（仅收藏范围需要，其它范围不必因收藏变动重排列表）。
    const favSig = state.scope === "favorites" ? state.favoriteIds.join(",") : "";
    const sig = `${state.filter.query}\u0000${state.filter.country ?? ""}\u0000${state.scope}\u0000${favSig}`;
    const filterChanged = sig !== filterSig;
    filterSig = sig;
    if (document.activeElement !== searchInput && !searchDirty) {
      searchInput.value = state.filter.query;
    }
    if (document.activeElement !== countrySelect) {
      countrySelect.value = state.filter.country ?? "";
    }

    let reload = filterChanged;
    if (filterChanged) page = 0;

    if (state.panelOpen) {
      if (!wasOpen) {
        wasOpen = true;
        reload = true;
        if (!countriesLoaded) void loadCountries();
      }
      if (reload) void loadPage(page);
    } else {
      wasOpen = false;
      view = "browse";
    }

    const favoriteOnly = state.scope === "favorites";
    favToggle.classList.toggle("is-active", favoriteOnly);
    favToggle.setAttribute("aria-pressed", String(favoriteOnly));
    favToggle.title = favoriteOnly ? "显示全部范围" : "只看收藏范围";

    autoplayCheck.checked = state.autoplayOnLaunch;
    autoNextCheck.checked = state.autoNextOnFailure;

    applyView();
    updateSyncButton();
    updateFooter();
    updatePager();
    renderRows();
  }

  panel.addEventListener("click", (event) => {
    const target = (event.target as Element).closest<HTMLElement>("[data-act]");
    if (!target) return;
    switch (target.dataset.act) {
      case "close":
        run(ipc.setPanelOpen(false));
        break;
      case "favorites":
        if (currentState) {
          run(ipc.setScope(currentState.scope === "favorites" ? "list" : "favorites"));
        }
        break;
      case "prev-page":
        if (page > 0) void loadPage(page - 1);
        break;
      case "next-page": {
        const pages = Math.max(1, Math.ceil(total / PAGE_SIZE));
        if (page + 1 < pages) void loadPage(page + 1);
        break;
      }
      case "sync":
        startSync();
        break;
      case "settings":
        view = "settings";
        applyView();
        break;
      case "settings-back":
        view = "browse";
        applyView();
        break;
      default:
        break;
    }
  });

  list.addEventListener("click", (event) => {
    const star = (event.target as Element).closest<HTMLElement>(".star-btn");
    if (star) {
      event.stopPropagation();
      const id = star.dataset.id;
      if (id) run(ipc.toggleFavorite(id));
      return;
    }
    const row = (event.target as Element).closest<HTMLElement>(".station-row");
    const id = row?.dataset.id;
    if (id) run(ipc.playStation(id));
  });

  searchInput.addEventListener("input", () => {
    if (composing) return;
    searchDirty = true;
    scheduleSearch();
  });

  searchInput.addEventListener("compositionstart", () => {
    composing = true;
  });

  searchInput.addEventListener("compositionend", () => {
    composing = false;
    searchDirty = true;
    scheduleSearch();
  });

  countrySelect.addEventListener("change", () => {
    if (searchTimer !== undefined) window.clearTimeout(searchTimer);
    void applyFilter();
  });

  settingsView.addEventListener("change", (event) => {
    const input = event.target as HTMLInputElement;
    if (!input.matches('input[type="checkbox"][data-setting]')) return;
    const key = input.dataset.setting as SettingKey | undefined;
    if (key === "autoplayOnLaunch" || key === "autoNextOnFailure") {
      run(ipc.setSetting(key, input.checked));
    }
  });

  root.appendChild(panel);

  return {
    render,
    onSyncProgress(progress) {
      syncText = progressText(progress.done, progress.total);
      if (!syncing) {
        syncing = true;
        updateSyncButton();
      } else {
        syncLabel.textContent = syncText;
      }
      const phase = (progress.phase ?? "").toLowerCase();
      if (
        /done|complete|finish|error|fail|完成|失败/.test(phase) ||
        (progress.total > 0 && progress.done >= progress.total)
      ) {
        finishSync();
      }
    },
    onToast(toast) {
      if (syncing && /同步|更新|电台库|sync|update/i.test(toast.text)) {
        finishSync();
      }
    },
  };
}
