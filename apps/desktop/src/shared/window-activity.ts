import { getCurrentWindow } from "@tauri-apps/api/window";

export async function isAppWindowInactive(): Promise<boolean> {
  try {
    const window = getCurrentWindow();
    const [focused, visible] = await Promise.all([
      window.isFocused(),
      window.isVisible(),
    ]);

    return !focused || !visible;
  } catch (error) {
    console.error("[window-activity] failed to inspect window state", error);
    return true;
  }
}
