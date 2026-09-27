import { getCurrentWindow } from "@tauri-apps/api/window";
import { platform } from "@tauri-apps/plugin-os";
import { useState } from "react";

import { useMountEffect } from "~/shared/hooks/useMountEffect";

// The native macOS traffic lights live in window points, but the webview
// content scales with the zoom factor (--anlg-zoom). These classes divide the
// gutter by that factor so chrome stays aligned with the buttons at any zoom.
export const WINDOW_CONTROLS_GUTTER_CLASS =
  "pl-[calc(76px_/_var(--anlg-zoom,1))]";
export const WINDOW_CONTROLS_GUTTER_PLUS_28_CLASS =
  "left-[calc(76px_/_var(--anlg-zoom,1)_+_28px)]";
export const WINDOW_CONTROLS_GUTTER_PLUS_32_CLASS =
  "pl-[calc(76px_/_var(--anlg-zoom,1)_+_32px)]";
export const WINDOW_CONTROLS_GUTTER_PLUS_80_CLASS =
  "pl-[calc(76px_/_var(--anlg-zoom,1)_+_80px)]";
export const WINDOW_CONTROLS_SIDEBAR_MIN_WIDTH =
  "max(200px, calc(76px / var(--anlg-zoom, 1) + 124px))";
// Centers 28px (size-7) controls on the 23px baseline the traffic lights use.
export const WINDOW_CONTROLS_ROW_PADDING_TOP_CLASS =
  "pt-[max(0px,calc(23px_/_var(--anlg-zoom,1)_-_14px))]";

export function usesWindowsStyleTitleBar() {
  const runtimePlatform = getRuntimePlatform();

  return runtimePlatform === "windows" || runtimePlatform === "linux";
}

export function usesTitleBarSidebarActions() {
  return usesWindowsStyleTitleBar();
}

export function usesRoundedWindowFrame() {
  return getRuntimePlatform() === "linux";
}

export function useRoundedWindowFrame() {
  useMountEffect(() => {
    if (!usesRoundedWindowFrame()) {
      return;
    }

    const appWindow = getCurrentWindow();
    if (appWindow.label !== "main") {
      return;
    }

    let cancelled = false;
    let syncVersion = 0;
    let unlistenResize: (() => void) | undefined;
    document.documentElement.dataset.roundedWindowOwner = "app";
    const sync = async () => {
      const version = ++syncVersion;
      const [isMaximized, isFullscreen] = await Promise.all([
        appWindow.isMaximized().catch(() => false),
        appWindow.isFullscreen().catch(() => false),
      ]);

      if (cancelled || version !== syncVersion) {
        return;
      }

      if (isMaximized || isFullscreen) {
        delete document.documentElement.dataset.roundedWindow;
      } else {
        document.documentElement.dataset.roundedWindow = "";
      }
    };

    void sync();
    void appWindow
      .onResized(() => {
        void sync();
      })
      .then((unlisten) => {
        if (cancelled) {
          unlisten();
          return;
        }

        unlistenResize = unlisten;
      })
      .catch(() => {});

    return () => {
      cancelled = true;
      unlistenResize?.();
      delete document.documentElement.dataset.roundedWindowOwner;
    };
  });
}

export function useWindowControlsGutter() {
  const [visible, setVisible] = useState(() => {
    const runtimePlatform = getRuntimePlatform();

    return runtimePlatform === null || runtimePlatform === "macos";
  });

  useMountEffect(() => {
    if (getRuntimePlatform() !== "macos") {
      return;
    }

    let cancelled = false;
    let unlistenResize: (() => void) | undefined;
    const appWindow = getCurrentWindow();
    const sync = async () => {
      const isFullscreen = await appWindow.isFullscreen().catch(() => false);

      if (!cancelled) {
        setVisible(!isFullscreen);
      }
    };

    void sync();
    void appWindow
      .onResized(() => {
        void sync();
      })
      .then((unlisten) => {
        if (cancelled) {
          unlisten();
          return;
        }

        unlistenResize = unlisten;
      })
      .catch(() => {});

    return () => {
      cancelled = true;
      unlistenResize?.();
    };
  });

  return visible;
}

function getRuntimePlatform() {
  try {
    return platform();
  } catch {
    return null;
  }
}
