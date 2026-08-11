import { beforeEach, describe, expect, it, vi } from "vitest";

import { requestAppAttention } from "./app-attention";

const mocks = vi.hoisted(() => ({
  requestUserAttention: vi.fn(async () => {}),
  isAppWindowInactive: vi.fn(async () => true),
  getStoredSettingValues: vi.fn(async () => ({
    values: {},
    hasValues: new Set(),
  })),
}));

vi.mock("@tauri-apps/api/window", () => ({
  UserAttentionType: { Critical: 1, Informational: 2 },
  getCurrentWindow: () => ({
    requestUserAttention: mocks.requestUserAttention,
  }),
}));

vi.mock("./window-activity", () => ({
  isAppWindowInactive: mocks.isAppWindowInactive,
}));

vi.mock("~/settings/queries", () => ({
  getStoredSettingValues: mocks.getStoredSettingValues,
}));

describe("requestAppAttention", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.isAppWindowInactive.mockResolvedValue(true);
    mocks.getStoredSettingValues.mockResolvedValue({
      values: {},
      hasValues: new Set(),
    });
  });

  it("requests attention by default when the window is inactive", async () => {
    await requestAppAttention("summary_ready");
    expect(mocks.requestUserAttention).toHaveBeenCalledWith(2);
  });

  it("skips when the event's bounce setting is disabled", async () => {
    mocks.getStoredSettingValues.mockResolvedValue({
      values: { notification_bounce_transcript: false },
      hasValues: new Set(["notification_bounce_transcript"]),
    });

    await requestAppAttention("transcript_ready");
    expect(mocks.requestUserAttention).not.toHaveBeenCalled();
  });

  it("only consults the setting for the given event", async () => {
    mocks.getStoredSettingValues.mockResolvedValue({
      values: { notification_bounce_transcript: false },
      hasValues: new Set(["notification_bounce_transcript"]),
    });

    await requestAppAttention("summary_ready");
    expect(mocks.requestUserAttention).toHaveBeenCalledWith(2);
  });

  it("skips when the window is focused and visible", async () => {
    mocks.isAppWindowInactive.mockResolvedValue(false);

    await requestAppAttention("summary_ready");
    expect(mocks.requestUserAttention).not.toHaveBeenCalled();
  });

  it("swallows attention request failures", async () => {
    mocks.requestUserAttention.mockRejectedValue(new Error("boom"));

    await expect(requestAppAttention("summary_ready")).resolves.toBeUndefined();
  });
});
