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
