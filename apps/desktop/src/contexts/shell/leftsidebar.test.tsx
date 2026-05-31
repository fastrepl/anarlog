import { renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  sidebarTimelineEnabled: false,
  useHotkeys: vi.fn(),
}));

vi.mock("react-hotkeys-hook", () => ({
  useHotkeys: mocks.useHotkeys,
}));

vi.mock("~/shared/config", () => ({
  useConfigValue: () => mocks.sidebarTimelineEnabled,
}));

import { useLeftSidebar } from "./leftsidebar";

describe("useLeftSidebar", () => {
  beforeEach(() => {
    mocks.sidebarTimelineEnabled = false;
    mocks.useHotkeys.mockClear();
  });

  it("disables the toggle hotkey outside sidebar timeline mode", () => {
    renderHook(() => useLeftSidebar());

    expect(mocks.useHotkeys).toHaveBeenCalledWith(
      "mod+\\",
      expect.any(Function),
      expect.objectContaining({ enabled: false }),
      expect.any(Array),
    );
  });

  it("enables the toggle hotkey in sidebar timeline mode", () => {
    mocks.sidebarTimelineEnabled = true;

    renderHook(() => useLeftSidebar());

    expect(mocks.useHotkeys).toHaveBeenCalledWith(
      "mod+\\",
      expect.any(Function),
      expect.objectContaining({ enabled: true }),
      expect.any(Array),
    );
  });
});
