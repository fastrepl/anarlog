import { relaunch as tauriRelaunch } from "@tauri-apps/plugin-process";

import { commands as store2Commands } from "@hypr/plugin-store2";

import { flushDatabaseWrites } from "~/db/write-queue";
import { commands } from "~/types/tauri.gen";

let pendingAutomaticRelaunch = false;
let automaticRelaunchTimeout: ReturnType<typeof setTimeout> | null = null;

async function saveApplicationState(): Promise<void> {
  await Promise.all([flushDatabaseWrites(), store2Commands.save()]);
}

async function relaunch(): Promise<void> {
  await saveApplicationState();
  await tauriRelaunch();
}

async function getOnboardingNeeded() {
  try {
    const result = await commands.getOnboardingNeeded();
    if (result.status !== "ok") {
      return false;
    }
    return result.data;
  } catch {
    return false;
  }
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
