import { beforeEach, describe, expect, test, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  deleteProcessedSessionAudio: vi.fn(),
  getSessionMode: vi.fn(),
  live: { loading: false, sessionId: null as string | null },
}));

vi.mock("@anlg/plugin-transcription", () => ({
  commands: { deleteProcessedSessionAudio: mocks.deleteProcessedSessionAudio },
  events: {},
}));

vi.mock("~/store/zustand/listener/instance", () => ({
  listenerStore: {
    getState: () => ({
      getSessionMode: mocks.getSessionMode,
      live: mocks.live,
    }),
  },
}));

import {
  deleteProcessedAudioForRetention,
  normalizeAudioRetention,
} from "./audio-retention";

describe("audio retention", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.deleteProcessedSessionAudio.mockResolvedValue({
      status: "ok",
      data: true,
    });
    mocks.getSessionMode.mockReturnValue("inactive");
    mocks.live.loading = false;
    mocks.live.sessionId = null;
  });

  test("normalizes current and legacy values", () => {
    expect(normalizeAudioRetention("none")).toBe("none");
    expect(normalizeAudioRetention("oneWeek")).toBe("oneWeek");
    expect(normalizeAudioRetention("forever")).toBe("forever");
    expect(normalizeAudioRetention(false)).toBe("none");
    expect(normalizeAudioRetention(true)).toBe("forever");
    expect(normalizeAudioRetention("invalid")).toBe("forever");
    expect(normalizeAudioRetention("invalid", undefined)).toBeUndefined();
  });

  test("does not delete audio while capture startup is loading", async () => {
    mocks.live.loading = true;
    mocks.live.sessionId = "session-1";

    await expect(
      deleteProcessedAudioForRetention("none", "session-1"),
    ).resolves.toBe(false);
    expect(mocks.deleteProcessedSessionAudio).not.toHaveBeenCalled();
  });

  test("does not report a failed native deletion as deleted", async () => {
    mocks.deleteProcessedSessionAudio.mockResolvedValue({
      status: "error",
      error: "disk busy",
    });
    vi.spyOn(console, "error").mockImplementation(() => {});

    await expect(
      deleteProcessedAudioForRetention("none", "session-1"),
    ).resolves.toBe(false);
  });
});
