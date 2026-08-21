import { platform } from "@tauri-apps/plugin-os";

export function isFloatingBarSupported(currentPlatform = platform()) {
  return (
    currentPlatform === "macos" ||
    currentPlatform === "windows" ||
    currentPlatform === "linux"
  );
}
