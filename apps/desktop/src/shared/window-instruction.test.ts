import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  windowSaveFrame: vi.fn(),
  windowEmitNavigate: vi.fn(),
  windowSetFrameAnimated: vi.fn(),
}));

vi.mock("../../../../plugins/windows/js/bindings.gen", () => ({
  commands: mocks,
  events: {},
}));

import { openUrlWithInstruction } from "@anlg/plugin-windows";

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
});
