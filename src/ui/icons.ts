function svg(body: string): string {
  return `<svg class="icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true" focusable="false">${body}</svg>`;
}

const filled = (body: string): string =>
  `<svg class="icon" viewBox="0 0 24 24" fill="currentColor" stroke="none" aria-hidden="true" focusable="false">${body}</svg>`;

const STAR_PATH =
  "M12 3.4l2.7 5.5 6 .9-4.35 4.25 1.03 6-5.38-2.83-5.38 2.83 1.03-6L3.3 9.8l6-.9L12 3.4Z";

/** 全部图标为内联 SVG，颜色继承 currentColor。 */
export const icons = {
  prev: filled('<path d="M6 4.5v15"/><path d="M18.5 4.5L9 12l9.5 7.5V4.5Z"/>'),
  play: filled('<path d="M8 4.8L19.5 12 8 19.2V4.8Z"/>'),
  pause: filled(
    '<rect x="6.8" y="4.6" width="3.6" height="14.8" rx="1.2"/><rect x="13.6" y="4.6" width="3.6" height="14.8" rx="1.2"/>',
  ),
  next: filled('<path d="M18 4.5v15"/><path d="M5.5 4.5L15 12l-9.5 7.5V4.5Z"/>'),
  volume: svg(
    '<path d="M4 9.2v5.6h3.2L12 18.5V5.5L7.2 9.2H4Z"/>' +
      '<path d="M15.2 8.7a4.6 4.6 0 0 1 0 6.6"/><path d="M17.9 6a8.4 8.4 0 0 1 0 12"/>',
  ),
  muted: svg(
    '<path d="M4 9.2v5.6h3.2L12 18.5V5.5L7.2 9.2H4Z"/>' +
      '<path d="m15.2 9 5 6"/><path d="m20.2 9-5 6"/>',
  ),
  list: svg(
    '<path d="M8.5 6.6h11"/><path d="M8.5 12h11"/><path d="M8.5 17.4h11"/>' +
      '<circle cx="4.6" cy="6.6" r="1.1"/><circle cx="4.6" cy="12" r="1.1"/><circle cx="4.6" cy="17.4" r="1.1"/>',
  ),
  globe: svg(
    '<circle cx="12" cy="12" r="8.2"/><path d="M3.9 12h16.2"/>' +
      '<path d="M12 3.8c2.2 2.3 3.4 5.2 3.4 8.2s-1.2 5.9-3.4 8.2c-2.2-2.3-3.4-5.2-3.4-8.2s1.2-5.9 3.4-8.2Z"/>',
  ),
  "mode-sequential": svg(
    '<path d="M4 12h13.5"/><path d="m13.8 7.2 4.8 4.8-4.8 4.8"/>',
  ),
  "mode-loop": svg(
    '<path d="M4 9V8a3 3 0 0 1 3-3h12"/><path d="m16 2.5 3 2.5-3 2.5"/>' +
      '<path d="M20 15v1a3 3 0 0 1-3 3H5"/><path d="m8 21.5-3-2.5 3-2.5"/>',
  ),
  "mode-shuffle": svg(
    '<path d="M4 7h2.6c1.5 0 2.9.7 3.8 2l3.2 6c.9 1.3 2.3 2 3.8 2h2.1"/>' +
      '<path d="m17.2 14.3 3.3 2.7-3.3 2.7"/>' +
      '<path d="M4 17h2.6c1.5 0 2.9-.7 3.8-2l3.2-6c.9-1.3 2.3-2 3.8-2h2.1"/>' +
      '<path d="m17.2 4.3 3.3 2.7-3.3 2.7"/>',
  ),
  pin: svg(
    '<path d="M9 3.8h6"/><path d="M10.2 3.8v5.4l-2.7 3.2v1.2h9v-1.2L13.8 9.2V3.8"/><path d="M12 13.6v6.6"/>',
  ),
  search: svg('<circle cx="10.8" cy="10.8" r="6.4"/><path d="m15.6 15.6 4.6 4.6"/>'),
  close: svg('<path d="m6 6 12 12"/><path d="m18 6-12 12"/>'),
  star: svg(`<path d="${STAR_PATH}"/>`),
  "star-filled": filled(`<path d="${STAR_PATH}"/>`),
  refresh: svg(
    '<path d="M20 11.2a8.2 8.2 0 1 0-2.4 6.6"/><path d="M20 5.4v5.8h-5.8"/>',
  ),
  settings: svg(
    '<circle cx="12" cy="12" r="3.2"/>' +
      '<path d="M12 3.2v2.4M12 18.4v2.4M3.2 12h2.4M18.4 12h2.4M5.8 5.8l1.7 1.7M16.5 16.5l1.7 1.7M18.2 5.8l-1.7 1.7M7.5 16.5l-1.7 1.7"/>',
  ),
  back: svg('<path d="M19 12H5"/><path d="m10.5 6.5-5.5 5.5 5.5 5.5"/>'),
  chevron: svg('<path d="m7 9.5 5 5 5-5"/>'),
} as const;

export type IconName = keyof typeof icons;
