import { isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { platform } from "@tauri-apps/plugin-os";
import { useState } from "react";

import { useMountEffect } from "~/shared/hooks/useMountEffect";

export function useWindowControlsGutter() {
  const [visible, setVisible] = useState(
    () => !isTauri() || platform() === "macos",
  );

  useMountEffect(() => {
    if (!isTauri() || platform() !== "macos") {
      return;
    }

    let cancelled = false;
    let unlistenResize: (() => void) | undefined;
    const appWindow = getCurrentWindow();
    const sync = async () => {
      const [isMaximized, isFullscreen] = await Promise.all([
        appWindow.isMaximized(),
        appWindow.isFullscreen(),
      ]).catch(() => [false, false]);

      if (!cancelled) {
        setVisible(!isMaximized && !isFullscreen);
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
