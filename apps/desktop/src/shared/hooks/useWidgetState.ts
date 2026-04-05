import { isTauri } from "@tauri-apps/api/core";
import { useCallback, useState } from "react";

const COLLAPSED_SIZE = { width: 120, height: 36 };
const EXPANDED_SIZE = { width: 320, height: 380 };

export function useWidgetState() {
  const [isExpanded, setIsExpanded] = useState(false);

  const expand = useCallback(async () => {
    if (!isTauri()) {
      setIsExpanded(true);
      return;
    }

    const [
      { LogicalPosition, LogicalSize },
      { getCurrentWebviewWindow },
      tauriWindow,
    ] = await Promise.all([
      import("@tauri-apps/api/dpi"),
      import("@tauri-apps/api/webviewWindow"),
      import("@tauri-apps/api/window"),
    ]);

    const appWindow = getCurrentWebviewWindow();
    const monitor = await tauriWindow.currentMonitor();
    if (!monitor) return;

    const scale = monitor.scaleFactor;
    const screenWidth = monitor.size.width / scale;
    const screenHeight = monitor.size.height / scale;
    const screenX = monitor.position.x / scale;
    const screenY = monitor.position.y / scale;

    const x = screenX + screenWidth - EXPANDED_SIZE.width - 20;
    const y = screenY + screenHeight - EXPANDED_SIZE.height - 20;

    await appWindow.setSize(
      new LogicalSize(EXPANDED_SIZE.width, EXPANDED_SIZE.height),
    );
    await appWindow.setPosition(new LogicalPosition(x, y));
    setIsExpanded(true);
  }, []);

  const collapse = useCallback(async () => {
    if (!isTauri()) {
      setIsExpanded(false);
      return;
    }

    const [
      { LogicalPosition, LogicalSize },
      { getCurrentWebviewWindow },
      tauriWindow,
    ] = await Promise.all([
      import("@tauri-apps/api/dpi"),
      import("@tauri-apps/api/webviewWindow"),
      import("@tauri-apps/api/window"),
    ]);

    const appWindow = getCurrentWebviewWindow();
    const monitor = await tauriWindow.currentMonitor();
    if (!monitor) {
      setIsExpanded(false);
      return;
    }

    const scale = monitor.scaleFactor;
    const screenWidth = monitor.size.width / scale;
    const screenHeight = monitor.size.height / scale;
    const screenX = monitor.position.x / scale;
    const screenY = monitor.position.y / scale;

    const x = screenX + screenWidth - COLLAPSED_SIZE.width - 20;
    const y = screenY + screenHeight - COLLAPSED_SIZE.height - 20;

    await appWindow.setSize(
      new LogicalSize(COLLAPSED_SIZE.width, COLLAPSED_SIZE.height),
    );
    await appWindow.setPosition(new LogicalPosition(x, y));
    setIsExpanded(false);
  }, []);

  return { isExpanded, expand, collapse };
}
