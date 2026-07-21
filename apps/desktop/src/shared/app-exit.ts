import { listen } from "@tauri-apps/api/event";

import { commands as store2Commands } from "@hypr/plugin-store2";

import { flushDatabaseWrites } from "~/db/write-queue";
import { confirmAllPendingDeletions } from "~/store/zustand/undo-delete";
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

const PENDING_DELETION_EXIT_TIMEOUT_MS = 3000;

async function flushAndExit(): Promise<void> {
  try {
    // Confirm pending undo-deletions first: quitting inside the undo window
    // must not leave a note soft-deleted with its shared link still live.
    // Bounded so a slow revoke cannot hang exit.
    await Promise.race([
      confirmAllPendingDeletions(),
      new Promise((resolve) =>
        setTimeout(resolve, PENDING_DELETION_EXIT_TIMEOUT_MS),
      ),
    ]);
    await Promise.all([flushDatabaseWrites(), store2Commands.save()]);
  } catch (error) {
    console.error("Failed to flush application data before exit", error);
  } finally {
    await commands.completeAppExit();
  }
}
