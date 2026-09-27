import { cleanup, renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  getCurrentWindow: vi.fn(),
  isFullscreen: vi.fn(),
  isMaximized: vi.fn(),
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
  useRoundedWindowFrame,
  usesWindowsStyleTitleBar,
  useWindowControlsGutter,
} from "./useWindowControlsGutter";

describe("useWindowControlsGutter", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.isFullscreen.mockResolvedValue(false);
    mocks.isMaximized.mockResolvedValue(false);
    mocks.onResized.mockResolvedValue(vi.fn());
    mocks.getCurrentWindow.mockReturnValue({
      label: "main",
      isFullscreen: mocks.isFullscreen,
      isMaximized: mocks.isMaximized,
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

describe("useRoundedWindowFrame", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.isFullscreen.mockResolvedValue(false);
    mocks.isMaximized.mockResolvedValue(false);
    mocks.onResized.mockResolvedValue(vi.fn());
    mocks.getCurrentWindow.mockReturnValue({
      label: "main",
      isFullscreen: mocks.isFullscreen,
      isMaximized: mocks.isMaximized,
      onResized: mocks.onResized,
    });
    delete document.documentElement.dataset.roundedWindow;
    delete document.documentElement.dataset.roundedWindowOwner;
  });

  afterEach(cleanup);

  it("does nothing outside Linux", () => {
    mocks.platform.mockReturnValue("windows");

    renderHook(() => useRoundedWindowFrame());

    expect(mocks.getCurrentWindow).not.toHaveBeenCalled();
    expect(document.documentElement.dataset.roundedWindowOwner).toBeUndefined();
  });

  it("only owns the main window frame", () => {
    mocks.platform.mockReturnValue("linux");
    mocks.getCurrentWindow.mockReturnValue({ label: "note:1" });

    renderHook(() => useRoundedWindowFrame());

    expect(document.documentElement.dataset.roundedWindowOwner).toBeUndefined();
  });

  it("claims ownership on mount and releases it on unmount", async () => {
    mocks.platform.mockReturnValue("linux");

    const { unmount } = renderHook(() => useRoundedWindowFrame());

    expect(document.documentElement.dataset.roundedWindowOwner).toBe("app");
    await waitFor(() =>
      expect(document.documentElement.dataset.roundedWindow).toBe(""),
    );

    unmount();

    expect(document.documentElement.dataset.roundedWindowOwner).toBeUndefined();
  });

  it("drops the frame while maximized and restores it on resize", async () => {
    mocks.platform.mockReturnValue("linux");
    mocks.isMaximized.mockResolvedValue(true);
    let onResized: (() => void) | undefined;
    mocks.onResized.mockImplementation(async (handler: () => void) => {
      onResized = handler;
      return vi.fn();
    });
    document.documentElement.dataset.roundedWindow = "";

    renderHook(() => useRoundedWindowFrame());

    await waitFor(() =>
      expect(document.documentElement.dataset.roundedWindow).toBeUndefined(),
    );

    mocks.isMaximized.mockResolvedValue(false);
    onResized?.();

    await waitFor(() =>
      expect(document.documentElement.dataset.roundedWindow).toBe(""),
    );
  });

  it("ignores stale window-state results", async () => {
    mocks.platform.mockReturnValue("linux");
    let onResized: (() => void) | undefined;
    mocks.onResized.mockImplementation(async (handler: () => void) => {
      onResized = handler;
      return vi.fn();
    });
    let resolveStale: ((value: boolean) => void) | undefined;
    mocks.isMaximized.mockImplementationOnce(
      () =>
        new Promise<boolean>((resolve) => {
          resolveStale = resolve;
        }),
    );

    renderHook(() => useRoundedWindowFrame());
    await waitFor(() => expect(onResized).toBeDefined());

    onResized?.();
    await waitFor(() =>
      expect(document.documentElement.dataset.roundedWindow).toBe(""),
    );

    resolveStale?.(true);
    await Promise.resolve();
    await Promise.resolve();

    expect(document.documentElement.dataset.roundedWindow).toBe("");
  });
});
