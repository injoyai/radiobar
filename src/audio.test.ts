import { describe, expect, it } from "vitest";

import {
  MAX_ENDED_RESUMES,
  isHlsUrl,
  shouldReportStall,
  shouldResumeOnEnded,
} from "./audio";

describe("isHlsUrl", () => {
  it("detects m3u8 urls", () => {
    expect(isHlsUrl("https://a.com/live.m3u8")).toBe(true);
    expect(isHlsUrl("https://a.com/live.M3U8?token=1")).toBe(true);
    expect(isHlsUrl("https://a.com/live.m3u8#frag")).toBe(true);
  });

  it("rejects plain streams", () => {
    expect(isHlsUrl("https://a.com/live.mp3")).toBe(false);
    expect(isHlsUrl("https://a.com/m3u8/stream")).toBe(false);
  });
});

describe("shouldResumeOnEnded", () => {
  it("resumes when expecting playback on a direct stream", () => {
    expect(shouldResumeOnEnded(true, true, true, 0)).toBe(true);
  });

  it("does not resume after user pause", () => {
    expect(shouldResumeOnEnded(false, true, true, 0)).toBe(false);
  });

  it("does not resume without a source", () => {
    expect(shouldResumeOnEnded(true, false, true, 0)).toBe(false);
  });

  it("does not touch hls-managed sources", () => {
    expect(shouldResumeOnEnded(true, true, false, 0)).toBe(false);
  });

  it("stops resuming after repeated immediate ends", () => {
    expect(
      shouldResumeOnEnded(true, true, true, MAX_ENDED_RESUMES - 1),
    ).toBe(true);
    expect(shouldResumeOnEnded(true, true, true, MAX_ENDED_RESUMES)).toBe(
      false,
    );
  });
});

describe("shouldReportStall", () => {
  it("reports after 8 seconds", () => {
    expect(shouldReportStall(2, 8100, false)).toBe(true);
  });

  it("does not report twice", () => {
    expect(shouldReportStall(2, 9000, true)).toBe(false);
  });

  it("does not report when data is ready or too early", () => {
    expect(shouldReportStall(3, 9000, false)).toBe(false);
    expect(shouldReportStall(1, 5000, false)).toBe(false);
  });
});
