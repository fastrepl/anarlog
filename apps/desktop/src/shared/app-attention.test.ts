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
    await requestAppAttention();
    expect(mocks.requestUserAttention).toHaveBeenCalledWith(2);
  });

  it("skips when app icon bouncing is disabled", async () => {
    mocks.getStoredSettingValues.mockResolvedValue({
      values: { notification_bounce: false },
      hasValues: new Set(["notification_bounce"]),
    });

    await requestAppAttention();
    expect(mocks.requestUserAttention).not.toHaveBeenCalled();
  });

  it("skips when all notifications are disabled", async () => {
    mocks.getStoredSettingValues.mockResolvedValue({
      values: { notification_disabled: true },
      hasValues: new Set(["notification_disabled"]),
    });

    await requestAppAttention();
    expect(mocks.requestUserAttention).not.toHaveBeenCalled();
  });

  it("skips when the app is hidden from the Dock", async () => {
    mocks.getStoredSettingValues.mockResolvedValue({
      values: { show_app_in_dock: false },
      hasValues: new Set(["show_app_in_dock"]),
    });

    await requestAppAttention();
    expect(mocks.requestUserAttention).not.toHaveBeenCalled();
  });

  it("skips when the window is focused and visible", async () => {
    mocks.isAppWindowInactive.mockResolvedValue(false);

    await requestAppAttention();
    expect(mocks.requestUserAttention).not.toHaveBeenCalled();
  });

  it("swallows attention request failures", async () => {
    mocks.requestUserAttention.mockRejectedValue(new Error("boom"));

    await expect(requestAppAttention()).resolves.toBeUndefined();
  });
});
