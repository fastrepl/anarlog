import { commands } from "~/types/tauri.gen";

let appStoreBuild = false;

export async function initializeAppStoreBuild(): Promise<void> {
  appStoreBuild = await commands.isAppStoreBuild().catch((error) => {
    console.error("[startup] failed to read App Store build capability", error);
    return true;
  });
}

export function isAppStoreBuild(): boolean {
  return appStoreBuild;
}
