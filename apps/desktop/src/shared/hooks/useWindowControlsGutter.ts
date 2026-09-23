import { getCurrentWindow } from "@tauri-apps/api/window";
import { platform } from "@tauri-apps/plugin-os";
import { useState } from "react";

import { useMountEffect } from "~/shared/hooks/useMountEffect";

export function usesWindowsStyleTitleBar() {
  const runtimePlatform = getRuntimePlatform();

  return runtimePlatform === "windows" || runtimePlatform === "linux";
}

export function usesTitleBarSidebarActions() {
  return getRuntimePlatform() === "windows";
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
