import { relaunch as tauriRelaunch } from "@tauri-apps/plugin-process";

import { commands as store2Commands } from "@anlg/plugin-store2";

import { flushDatabaseWritesWithin } from "~/db/write-queue";
import { commands } from "~/types/tauri.gen";

let pendingAutomaticRelaunch = false;
let automaticRelaunchTimeout: ReturnType<typeof setTimeout> | null = null;
const APPLICATION_STATE_FLUSH_TIMEOUT_MS = 5000;

export async function flushApplicationState(): Promise<void> {
  const results = await Promise.allSettled([
    flushDatabaseWritesWithin(APPLICATION_STATE_FLUSH_TIMEOUT_MS),
    store2Commands.save(),
  ]);
  const failure = results.find((result) => result.status === "rejected");
  if (failure) throw failure.reason;
}

async function relaunch(): Promise<void> {
  try {
    await flushApplicationState();
  } catch (error) {
    console.error("Failed to flush application data before relaunch", error);
  }
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
