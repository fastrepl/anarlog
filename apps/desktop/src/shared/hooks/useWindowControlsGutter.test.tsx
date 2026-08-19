import { cleanup, renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  getCurrentWindow: vi.fn(),
  isFullscreen: vi.fn(),
  onResized: vi.fn(),
  platform: vi.fn(),
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: mocks.getCurrentWindow,
}));

vi.mock("@tauri-apps/plugin-os", () => ({
  platform: mocks.platform,
}));

import {
  usesWindowsStyleTitleBar,
  useWindowControlsGutter,
} from "./useWindowControlsGutter";

describe("useWindowControlsGutter", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.isFullscreen.mockResolvedValue(false);
    mocks.onResized.mockResolvedValue(vi.fn());
    mocks.getCurrentWindow.mockReturnValue({
      isFullscreen: mocks.isFullscreen,
      onResized: mocks.onResized,
    });
  });

  afterEach(cleanup);

  it.each(["windows", "linux"])(
    "does not reserve macOS window controls on %s",
    (runtimePlatform) => {
      mocks.platform.mockReturnValue(runtimePlatform);

      const { result } = renderHook(() => useWindowControlsGutter());

      expect(result.current).toBe(false);
      expect(usesWindowsStyleTitleBar()).toBe(true);
      expect(mocks.getCurrentWindow).not.toHaveBeenCalled();
    },
  );

  it("keeps the preview gutter when the runtime platform is unavailable", () => {
    mocks.platform.mockImplementation(() => {
      throw new Error("Tauri runtime unavailable");
    });

    const { result } = renderHook(() => useWindowControlsGutter());

    expect(result.current).toBe(true);
    expect(usesWindowsStyleTitleBar()).toBe(false);
    expect(mocks.getCurrentWindow).not.toHaveBeenCalled();
  });

  it("keeps the macOS gutter outside fullscreen", async () => {
    mocks.platform.mockReturnValue("macos");

    const { result } = renderHook(() => useWindowControlsGutter());

    expect(result.current).toBe(true);
    expect(usesWindowsStyleTitleBar()).toBe(false);
    await waitFor(() => expect(mocks.isFullscreen).toHaveBeenCalledOnce());
    expect(result.current).toBe(true);
  });

  it("removes the macOS gutter in fullscreen", async () => {
    mocks.platform.mockReturnValue("macos");
    mocks.isFullscreen.mockResolvedValue(true);

    const { result } = renderHook(() => useWindowControlsGutter());

    await waitFor(() => expect(result.current).toBe(false));
  });
});
