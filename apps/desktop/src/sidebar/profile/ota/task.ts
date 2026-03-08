import { getVersion } from "@tauri-apps/api/app";
import { check } from "@tauri-apps/plugin-updater";
import { useSelector } from "@xstate/store/react";

import { updateStore } from "./store";

import { relaunch } from "~/store/tinybase/store/save";

export function useOTA() {
  const snapshot = useSelector(updateStore, (state) => state.context);

  return {
    ...snapshot,
    handleCheckForUpdate: () => checkForUpdate(),
    handleStartDownload,
    handleCancelDownload,
    handleInstall,
  };
}

const checkForUpdate = async () => {
  updateStore.trigger.setState({ state: "checking" });

  try {
    const [update, currentVersion] = await Promise.all([check(), getVersion()]);
    updateStore.trigger.checkSuccess({ update, currentVersion });

    if (!update) {
      setTimeout(() => {
        const currentState = updateStore.getSnapshot().context.state;
        if (currentState === "noUpdate") {
          updateStore.trigger.reset();
        }
      }, 2000);
    }
  } catch (err) {
    const errorMessage =
      err instanceof Error ? err.message : "Failed to check for updates";
    updateStore.trigger.checkError({ error: errorMessage });
  }
};

const handleStartDownload = async () => {
  const { update } = updateStore.getSnapshot().context;

  if (!update) {
    return;
  }

  updateStore.trigger.startDownload();

  try {
    await update.download((event) => {
      if (event.event === "Started") {
        updateStore.trigger.downloadProgress({
          chunkLength: 0,
          contentLength: event.data.contentLength,
        });
      } else if (event.event === "Progress") {
        updateStore.trigger.downloadProgress({
          chunkLength: event.data.chunkLength,
        });
      } else if (event.event === "Finished") {
        updateStore.trigger.downloadFinished();
      }
    });
  } catch (err) {
    const errorMessage = err instanceof Error ? err.message : "Download failed";
    updateStore.trigger.checkError({ error: errorMessage });
  }
};

const handleCancelDownload = async () => {
  const { update } = updateStore.getSnapshot().context;

  if (update) {
    try {
      await update.close();
    } catch (err) {
      console.error("Failed to close update:", err);
    }
  }
  updateStore.trigger.cancelDownload();
};

const handleInstall = async () => {
  const { update } = updateStore.getSnapshot().context;

  if (!update) {
    return;
  }

  updateStore.trigger.setInstalling();

  try {
    if (process.env.NODE_ENV !== "development") {
      await update.install();
      await relaunch();
    }
  } catch (err) {
    const errorMessage =
      err instanceof Error ? err.message : "Installation failed";
    updateStore.trigger.checkError({ error: errorMessage });
  }
};
