import { getCurrentWindow } from "@tauri-apps/api/window";
import { platform } from "@tauri-apps/plugin-os";
import { useState } from "react";

import { useMountEffect } from "~/shared/hooks/useMountEffect";

export function usesWindowsStyleTitleBar() {
  const runtimePlatform = getRuntimePlatform();

  return runtimePlatform === "windows" || runtimePlatform === "linux";
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
