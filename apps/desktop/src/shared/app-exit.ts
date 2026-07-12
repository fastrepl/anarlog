import { listen } from "@tauri-apps/api/event";

import { commands as store2Commands } from "@hypr/plugin-store2";

import { flushDatabaseWrites } from "~/db/write-queue";
import { commands } from "~/types/tauri.gen";

const APP_EXIT_REQUESTED_EVENT = "app-exit-requested";

let exitInProgress = false;

export async function initializeAppExitFlush(): Promise<void> {
  await listen(APP_EXIT_REQUESTED_EVENT, () => {
    if (exitInProgress) {
      return;
    }

    exitInProgress = true;
    void flushAndExit();
  });
}

async function flushAndExit(): Promise<void> {
  try {
    await Promise.all([flushDatabaseWrites(), store2Commands.save()]);
  } catch (error) {
    console.error("Failed to flush application data before exit", error);
  } finally {
    await commands.completeAppExit();
  }
}
