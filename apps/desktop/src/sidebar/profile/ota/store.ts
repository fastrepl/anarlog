import { type Update } from "@tauri-apps/plugin-updater";
import { createStore } from "@xstate/store";

type State =
  | "idle"
  | "checking"
  | "error"
  | "noUpdate"
  | "available"
  | "downloading"
  | "ready"
  | "installing";

type Context = {
  state: State;
  update: Update | null;
  error: string | null;
  currentVersion: string | null;
  downloadProgress: {
    downloaded: number;
    total: number | null;
    percentage: number;
  };
};

export const updateStore = createStore({
  context: {
    state: "idle" as State,
    update: null,
    error: null,
    currentVersion: null,
    downloadProgress: {
      downloaded: 0,
      total: null,
      percentage: 0,
    },
  } as Context,
  on: {
    setState: (context, event: { state: State }) => ({
      ...context,
      state: event.state,
    }),
    checkSuccess: (
      context,
      event: { update: Update | null; currentVersion: string },
    ) => ({
      ...context,
      update: event.update,
      error: null,
      currentVersion: event.currentVersion,
      state: event.update ? ("available" as State) : ("noUpdate" as State),
    }),
    checkError: (context, event: { error: string }) => ({
      ...context,
      error: event.error,
      update: null,
      state: "error" as State,
    }),
    startDownload: (context) => ({
      ...context,
      downloadProgress: {
        downloaded: 0,
        total: null,
        percentage: 0,
      },
      state: "downloading" as State,
    }),
    downloadProgress: (
      context,
      event: { chunkLength: number; contentLength?: number },
    ) => ({
      ...context,
      downloadProgress: {
        downloaded: context.downloadProgress.downloaded + event.chunkLength,
        total: event.contentLength ?? context.downloadProgress.total,
        percentage:
          event.contentLength || context.downloadProgress.total
            ? Math.min(
                100,
                Math.round(
                  ((context.downloadProgress.downloaded + event.chunkLength) /
                    (event.contentLength ??
                      context.downloadProgress.total ??
                      1)) *
                    100,
                ),
              )
            : 0,
      },
    }),
    downloadFinished: (context) => ({
      ...context,
      state: "ready" as State,
    }),
    cancelDownload: (context) => ({
      ...context,
      update: null,
      downloadProgress: {
        downloaded: 0,
        total: null,
        percentage: 0,
      },
      state: "idle" as State,
    }),
    setInstalling: (context) => ({
      ...context,
      state: "installing" as State,
    }),
    reset: (context) => ({
      ...context,
      update: null,
      error: null,
      downloadProgress: {
        downloaded: 0,
        total: null,
        percentage: 0,
      },
      state: "idle" as State,
    }),
  },
});
