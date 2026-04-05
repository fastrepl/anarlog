import { relaunch as tauriRelaunch } from "@tauri-apps/plugin-process";

import { commands as store2Commands } from "@hypr/plugin-store2";

import { commands } from "~/types/tauri.gen";

const saveHandlers = new Map<string, () => Promise<void>>();
let pendingAutomaticRelaunch = false;
let automaticRelaunchTimeout: ReturnType<typeof setTimeout> | null = null;

export function registerSaveHandler(id: string, handler: () => Promise<void>) {
  saveHandlers.set(id, handler);
  return () => {
    saveHandlers.delete(id);
  };
}

export async function save(): Promise<void> {
  await Promise.all([
    ...Array.from(saveHandlers.values()).map((handler) => handler()),
    store2Commands.save(),
  ]);
}

export async function relaunch(): Promise<void> {
  await save();
  await tauriRelaunch();
}

async function getOnboardingNeeded() {
  const result = await commands.getOnboardingNeeded().catch(() => null);
  if (result?.status !== "ok") {
    return false;
  }
  return result.data;
}

export async function scheduleAutomaticRelaunch(
  delayMs = 0,
): Promise<"scheduled" | "deferred"> {
  if (await getOnboardingNeeded()) {
    pendingAutomaticRelaunch = true;
    return "deferred";
  }

  if (automaticRelaunchTimeout) {
    return "scheduled";
  }

  automaticRelaunchTimeout = setTimeout(() => {
    automaticRelaunchTimeout = null;
    void relaunch().catch(console.error);
  }, delayMs);

  return "scheduled";
}

export async function flushAutomaticRelaunch(): Promise<boolean> {
  if (!pendingAutomaticRelaunch || (await getOnboardingNeeded())) {
    return false;
  }

  pendingAutomaticRelaunch = false;

  if (automaticRelaunchTimeout) {
    clearTimeout(automaticRelaunchTimeout);
    automaticRelaunchTimeout = null;
  }

  try {
    await relaunch();
    return true;
  } catch (error) {
    pendingAutomaticRelaunch = true;
    throw error;
  }
}
