import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  windowSaveFrame: vi.fn(),
  windowEmitNavigate: vi.fn(),
  windowSetFrameAnimated: vi.fn(),
  windowRestoreFrameAnimated: vi.fn(),
}));

vi.mock("../../../../plugins/windows/js/bindings.gen", () => ({
  commands: mocks,
  events: {},
}));

import {
  dismissInstruction,
  openUrlWithInstruction,
} from "@anlg/plugin-windows";

describe("openUrlWithInstruction", () => {
  beforeEach(() => {
    vi.resetAllMocks();
  });

  it("does not navigate or resize when the original frame cannot be saved", async () => {
    mocks.windowSaveFrame.mockResolvedValue({
      status: "error",
      error: "restore geometry unavailable",
    });
    const openUrl = vi.fn();

    await expect(
      openUrlWithInstruction("https://example.com", "billing", openUrl),
    ).rejects.toThrow("restore geometry unavailable");

    expect(mocks.windowEmitNavigate).not.toHaveBeenCalled();
    expect(mocks.windowSetFrameAnimated).not.toHaveBeenCalled();
    expect(openUrl).not.toHaveBeenCalled();
  });

  it("completes dismissal even if restoring the original frame fails", async () => {
    mocks.windowEmitNavigate.mockResolvedValue({ status: "ok", data: null });
    mocks.windowRestoreFrameAnimated.mockResolvedValue({
      status: "error",
      error: "window manager timed out",
    });
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});

    await expect(dismissInstruction()).resolves.toBeUndefined();
    expect(mocks.windowEmitNavigate).toHaveBeenCalledWith(
      { type: "main" },
      { path: "/app", search: null },
    );
    expect(warn).toHaveBeenCalledWith(
      "Failed to restore instruction window frame:",
      "window manager timed out",
    );
    warn.mockRestore();
  });
});
