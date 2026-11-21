import { listen } from "@tauri-apps/api/event";

import { commands as analyticsCommands } from "@anlg/plugin-analytics";
import { commands as store2Commands } from "@anlg/plugin-store2";

import { flushDatabaseWritesWithin } from "~/db/write-queue";
import { flushAllCanonicalSessionEditors } from "~/session-sharing/editor-activity";
import { flushLocalStorageMirror } from "~/shared/local-storage-mirror";
import { confirmAllPendingDeletions } from "~/store/zustand/undo-delete";
import { commands } from "~/types/tauri.gen";

const APP_EXIT_REQUESTED_EVENT = "app-exit-requested";

let exitInProgress = false;

export async function initializeAppExitFlush(): Promise<void> {
  await listen<{ quick?: boolean } | null>(
    APP_EXIT_REQUESTED_EVENT,
    (event) => {
      if (exitInProgress) {
        return;
      }

      exitInProgress = true;
      void flushAndExit(event.payload?.quick === true);
    },
  );
}

const PENDING_DELETION_EXIT_TIMEOUT_MS = 3000;
const APPLICATION_STATE_FLUSH_TIMEOUT_MS = 5000;
// A confirmed quit (Quit Completely, a second ⌘Q, ⌘⇧Q) only saves local edits,
// so quitting stays prompt.
const QUICK_EXIT_FLUSH_TIMEOUT_MS = 2000;

async function flushApplicationStateWithin(timeoutMs: number): Promise<void> {
  let timeout: ReturnType<typeof setTimeout> | undefined;
  try {
    const results = await Promise.race([
      Promise.allSettled([
        analyticsCommands.event({ event: "app_exit_requested" }),
        flushDatabaseWritesWithin(timeoutMs),
        store2Commands.save(),
        flushLocalStorageMirror(),
      ]),
      new Promise<never>((_, reject) => {
        timeout = setTimeout(
          () =>
            reject(
              new Error(`Application state flush exceeded ${timeoutMs}ms`),
            ),
          timeoutMs,
        );
      }),
    ]);
    const failure = results.find((result) => result.status === "rejected");
    if (failure) throw failure.reason;
  } finally {
    clearTimeout(timeout);
  }
}

async function flushAndExit(quick: boolean): Promise<void> {
  try {
    // The editors save 500 ms after the last key; hand those edits to the
    // write queue before it is drained.
    flushAllCanonicalSessionEditors();
    if (quick) {
      await flushDatabaseWritesWithin(QUICK_EXIT_FLUSH_TIMEOUT_MS);
      return;
    }
    // Confirm pending undo-deletions first: quitting inside the undo window
    // must not leave a note soft-deleted with its shared link still live.
    // Bounded so a slow revoke cannot hang exit.
    await Promise.race([
      confirmAllPendingDeletions(),
      new Promise((resolve) =>
        setTimeout(resolve, PENDING_DELETION_EXIT_TIMEOUT_MS),
      ),
    ]);
    await flushApplicationStateWithin(APPLICATION_STATE_FLUSH_TIMEOUT_MS);
  } catch (error) {
    console.error("Failed to flush application data before exit", error);
  } finally {
    await commands.completeAppExit();
  }
}
