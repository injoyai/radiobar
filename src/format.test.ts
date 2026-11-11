import { describe, expect, it } from "vitest";

import {
  codecLine,
  countryName,
  flagEmoji,
  formatLastSync,
  progressText,
  truncateName,
} from "./format";

describe("flagEmoji", () => {
  it("maps CN to the Chinese flag", () => {
    expect(flagEmoji("CN")).toBe("🇨🇳");
  });

  it("maps lowercase codes", () => {
    expect(flagEmoji("jp")).toBe("🇯🇵");
  });

  it("falls back for unknown country codes", () => {
    expect(flagEmoji("ZZ")).toBe("🏳️");
  });

  it("falls back for empty or malformed codes", () => {
    expect(flagEmoji("")).toBe("🏳️");
    expect(flagEmoji("C")).toBe("🏳️");
    expect(flagEmoji("CHN")).toBe("🏳️");
  });
});

describe("truncateName", () => {
  it("keeps emoji graphemes whole", () => {
    expect(truncateName("🎵".repeat(30), 4)).toBe("🎵🎵🎵🎵…");
  });

  it("keeps short names unchanged", () => {
    expect(truncateName("KEXP", 22)).toBe("KEXP");
  });

  it("truncates long names with an ellipsis", () => {
    expect(truncateName("abcdefghij", 5)).toBe("abcde…");
  });
});

describe("codecLine", () => {
  it("shows codec and bitrate", () => {
    expect(codecLine({ codec: "MP3", bitrate: 64, hls: false })).toBe("MP3 · 64 kbps");
  });

  it("shows HLS and codec when there is no bitrate", () => {
    expect(codecLine({ codec: "AAC", bitrate: 0, hls: true })).toBe("HLS · AAC");
  });

  it("shows only the codec for plain streams", () => {
    expect(codecLine({ codec: "MP3", bitrate: 0, hls: false })).toBe("MP3");
  });

  it("falls back when codec is empty", () => {
    expect(codecLine({ codec: "", bitrate: 0, hls: true })).toBe("HLS");
    expect(codecLine({ codec: "", bitrate: 0, hls: false })).toBe("未知编码");
  });
});

describe("progressText", () => {
  it("formats sync progress", () => {
    expect(progressText(3, 12)).toBe("更新中 3/12");
  });
});

describe("countryName", () => {
  it("maps CN to Chinese", () => {
    expect(countryName("CN")).toBe("中国");
  });

  it("returns the code for unknown codes", () => {
    expect(countryName("ZZ")).toBe("ZZ");
  });
});

describe("formatLastSync", () => {
  it("handles missing and invalid timestamps", () => {
    expect(formatLastSync(null)).toBe("从未");
    expect(formatLastSync("not-a-date")).toBe("未知");
  });

  it("formats a valid timestamp", () => {
    expect(formatLastSync("2026-11-10T08:05:00")).toBe("11-10 08:05");
  });
});
