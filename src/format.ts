import type { Station } from "./state";

/** ISO 3166-1 alpha-2 国家码（两位大写）。仅这些码会生成国旗 emoji。 */
const ISO_CODES = new Set(
  (
    "AD AE AF AG AI AL AM AO AQ AR AS AT AU AW AX AZ BA BB BD BE BF BG BH BI BJ BL BM BN BO BQ BR BS BT BV BW BY BZ " +
    "CA CC CD CF CG CH CI CK CL CM CN CO CR CU CV CW CX CY CZ DE DJ DK DM DO DZ EC EE EG EH ER ES ET FI FJ FK FM FO FR " +
    "GA GB GD GE GF GG GH GI GL GM GN GP GQ GR GS GT GU GW GY HK HM HN HR HT HU ID IE IL IM IN IO IQ IR IS IT JE JM JO JP " +
    "KE KG KH KI KM KN KP KR KW KY KZ LA LB LC LI LK LR LS LT LU LV LY MA MC MD ME MF MG MH MK ML MM MN MO MP MQ MR MS MT " +
    "MU MV MW MX MY MZ NA NC NE NF NG NI NL NO NP NR NU NZ OM PA PE PF PG PH PK PL PM PN PR PS PT PW PY QA RE RO RS RU RW " +
    "SA SB SC SD SE SG SH SI SJ SK SL SM SN SO SR SS ST SV SX SY SZ TC TD TF TG TH TJ TK TL TM TN TO TR TT TV TW TZ UA UG " +
    "UM US UY UZ VA VC VE VG VI VN VU WF WS YE YT ZA ZM ZW"
  ).split(" "),
);

const WHITE_FLAG = "\u{1F3F3}\u{FE0F}";

/** 两位国家码 → 国旗 emoji；非法/空返回白旗。 */
export function flagEmoji(code: string): string {
  const upper = (code ?? "").trim().toUpperCase();
  if (!ISO_CODES.has(upper)) return WHITE_FLAG;
  return String.fromCodePoint(
    0x1f1e6 + upper.charCodeAt(0) - 65,
    0x1f1e6 + upper.charCodeAt(1) - 65,
  );
}

let segmenter: Intl.Segmenter | null = null;
try {
  segmenter = new Intl.Segmenter("zh", { granularity: "grapheme" });
} catch {
  segmenter = null;
}

/** 按 grapheme（用户感知字符）截断，超出加省略号。 */
export function truncateName(name: string, max = 22): string {
  const text = name ?? "";
  if (max <= 0) return text ? "…" : "";
  const parts = segmenter
    ? Array.from(segmenter.segment(text), (s) => s.segment)
    : Array.from(text);
  if (parts.length <= max) return text;
  return parts.slice(0, max).join("") + "…";
}

let regionNames: Intl.DisplayNames | null = null;
try {
  regionNames = new Intl.DisplayNames(["zh"], { type: "region", fallback: "code" });
} catch {
  regionNames = null;
}

/** 两位国家码 → 中文国家名；翻译缺失时回退国家码本身。 */
export function countryName(code: string): string {
  const upper = (code ?? "").trim().toUpperCase();
  if (!upper) return "";
  if (!ISO_CODES.has(upper) || !regionNames) return upper;
  try {
    const name = regionNames.of(upper);
    return name && name !== upper ? name : upper;
  } catch {
    return upper;
  }
}

/** 条上第二行右侧的编码/码率小字。 */
export function codecLine(st: Pick<Station, "codec" | "bitrate" | "hls">): string {
  const codec = (st.codec ?? "").trim();
  if (!codec) return st.hls ? "HLS" : "未知编码";
  if (st.bitrate > 0) return `${codec} · ${st.bitrate} kbps`;
  if (st.hls) return `HLS · ${codec}`;
  return codec;
}

/** 同步进度文字，如「更新中 3/12」。 */
export function progressText(done: number, total: number): string {
  return `更新中 ${done}/${total}`;
}

/** 上次同步时间 →「11-10 15:30」；从未同步显示「从未」。 */
export function formatLastSync(iso: string | null): string {
  if (!iso) return "从未";
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return "未知";
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${pad(date.getMonth() + 1)}-${pad(date.getDate())} ${pad(date.getHours())}:${pad(
    date.getMinutes(),
  )}`;
}
