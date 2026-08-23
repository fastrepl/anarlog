import { useMutation, useQuery } from "@tanstack/react-query";
import { useCallback, useMemo, useState } from "react";

import {
  commands as updaterCommands,
  events as updaterEvents,
  type Result,
} from "@anlg/plugin-updater2";

import { isAppStoreBuild } from "~/shared/app-store";
import { useMountEffect } from "~/shared/hooks/useMountEffect";
import { useDevtoolsOtaPreview } from "~/store/zustand/devtools-ota-preview";

export type UpdateBannerStatus =
  | "available"
  | "downloading"
  | "ready"
  | "failed";

export type DesktopUpdateControl = {
  status: UpdateBannerStatus | null;
  version: string | null;
  progress: number | null;
  errorMessage: string | null;
  downloadStarting: boolean;
  installing: boolean;
  downloadUpdate: () => void;
  installUpdate: () => void;
};

// One discriminated state with only variant-valid fields: updater events,
// mutations, and the periodic check all reduce into it, and the visible
// control is derived from it in a single precedence pass.
export type UpdateState =
  | { kind: "none" }
  | { kind: "available"; version: string }
  | {
      kind: "downloading";
      version: string;
      downloadedBytes: number;
      contentLength: number | null;
    }
  | { kind: "ready"; version: string }
  | { kind: "failed"; version: string; errorMessage: string };

type UpdateEvent = Exclude<UpdateState, { kind: "none" }>;

type UpdateCheckState = {
  version: string;
  ready: boolean;
} | null;

export function resolveUpdateState(
  event: UpdateEvent | null,
  check: UpdateCheckState,
  acknowledgedVersion: string | null,
): UpdateState {
  const checked = check && check.version !== acknowledgedVersion ? check : null;

  if (event) {
    if (
      event.kind === "available" &&
      checked?.version === event.version &&
      checked.ready
    ) {
      return { kind: "ready", version: event.version };
    }
    return event;
  }

  if (checked) {
    return checked.ready
      ? { kind: "ready", version: checked.version }
      : { kind: "available", version: checked.version };
  }

  return { kind: "none" };
}

const UPDATE_CHECK_QUERY_KEY = ["updater2", "check"] as const;
const UPDATE_CHECK_INTERVAL_MS = 30 * 60 * 1000;
const DISABLED_UPDATE_CONTROL: DesktopUpdateControl = {
  status: null,
  version: null,
  progress: null,
  errorMessage: null,
  downloadStarting: false,
  installing: false,
  downloadUpdate: () => undefined,
  installUpdate: () => undefined,
};

export function useDesktopUpdateControl(): DesktopUpdateControl {
  const updaterEnabled = !isAppStoreBuild();
  const [eventState, setEventState] = useState<UpdateEvent | null>(null);
  const [acknowledgedVersion, setAcknowledgedVersion] = useState<string | null>(
    null,
  );
  const devtoolsPreview = useDevtoolsOtaPreview((state) => state.preview);
  const showDevtoolsOtaPreview = useDevtoolsOtaPreview(
    (state) => state.showPreview,
  );
  const clearDevtoolsOtaPreview = useDevtoolsOtaPreview(
    (state) => state.clearPreview,
  );

  useMountEffect(() => {
    if (!updaterEnabled) {
      return;
    }

    let cancelled = false;
    const unlistenFns: Array<() => void> = [];

    const listen = async () => {
      const [
        unlistenAvailable,
        unlistenDownloading,
        unlistenProgress,
        unlistenReady,
        unlistenFailed,
        unlistenUpdated,
      ] = await Promise.all([
        updaterEvents.updateAvailableEvent.listen(({ payload }) => {
          setEventState((current) =>
            current?.version === payload.version && current.kind !== "available"
              ? current
              : { kind: "available", version: payload.version },
          );
        }),
        updaterEvents.updateDownloadingEvent.listen(({ payload }) => {
          setEventState({
            kind: "downloading",
            version: payload.version,
            downloadedBytes: 0,
            contentLength: null,
          });
        }),
        updaterEvents.updateDownloadProgressEvent.listen(({ payload }) => {
          setEventState((current) => {
            const downloadedBytes =
              current?.kind === "downloading" &&
              current.version === payload.version
                ? current.downloadedBytes + payload.chunk_length
                : payload.chunk_length;

            return {
              kind: "downloading",
              version: payload.version,
              downloadedBytes,
              contentLength: payload.content_length,
            };
          });
        }),
        updaterEvents.updateReadyEvent.listen(({ payload }) => {
          setEventState({ kind: "ready", version: payload.version });
        }),
        updaterEvents.updateDownloadFailedEvent.listen(({ payload }) => {
          setEventState({
            kind: "failed",
            version: payload.version,
            errorMessage: "Failed to download update.",
          });
        }),
        updaterEvents.updatedEvent.listen(({ payload }) => {
          setAcknowledgedVersion(payload.current);
          setEventState(null);
        }),
      ]);

      if (cancelled) {
        unlistenAvailable();
        unlistenDownloading();
        unlistenProgress();
        unlistenReady();
        unlistenFailed();
        unlistenUpdated();
        return;
      }

      unlistenFns.push(
        unlistenAvailable,
        unlistenDownloading,
        unlistenProgress,
        unlistenReady,
        unlistenFailed,
        unlistenUpdated,
      );
    };

    void listen();

    return () => {
      cancelled = true;
      unlistenFns.forEach((unlisten) => unlisten());
    };
  });

  // eslint-disable-next-line @tanstack/query/exhaustive-deps -- The state setter reconciles updater events and is not part of the update-check identity.
  const updateCheck = useQuery({
    queryKey: UPDATE_CHECK_QUERY_KEY,
    queryFn: async (): Promise<UpdateCheckState> => {
      const version = unwrapResult(await updaterCommands.check());

      if (!version) {
        setEventState((current) =>
          current?.kind === "available" ? null : current,
        );
        return null;
      }

      const nextUpdate = {
        version,
        ready: unwrapResult(await updaterCommands.isDownloaded(version)),
      };

      setEventState((current) =>
        current?.kind === "available" && current.version !== version
          ? null
          : current,
      );

      return nextUpdate;
    },
    refetchInterval: UPDATE_CHECK_INTERVAL_MS,
    retry: false,
    staleTime: UPDATE_CHECK_INTERVAL_MS,
    enabled: updaterEnabled,
  });

  const { mutate: downloadUpdate, isPending: downloadStarting } = useMutation({
    mutationFn: async (version: string) =>
      unwrapResult(await updaterCommands.download(version)),
    onMutate: (version) => {
      setEventState({
        kind: "downloading",
        version,
        downloadedBytes: 0,
        contentLength: null,
      });
    },
    onError: (error, version) => {
      setEventState({
        kind: "failed",
        version,
        errorMessage: readErrorMessage(error),
      });
    },
    onSuccess: (_data, version) => {
      setEventState((current) =>
        current?.kind === "ready" ? current : { kind: "ready", version },
      );
    },
  });

  const { mutate: installUpdate, isPending: installing } = useMutation({
    mutationFn: async (version: string) => {
      unwrapResult(await updaterCommands.installAndRelaunch(version));
    },
    onError: (error, version) => {
      setEventState({
        kind: "failed",
        version,
        errorMessage: readErrorMessage(error),
      });
    },
  });

  const state = useMemo(
    () =>
      resolveUpdateState(
        eventState,
        updateCheck.data ?? null,
        acknowledgedVersion,
      ),
    [eventState, updateCheck.data, acknowledgedVersion],
  );
  const status = state.kind === "none" ? null : state.kind;
  const version = state.kind === "none" ? null : state.version;
  const progress =
    state.kind === "downloading" && state.contentLength
      ? Math.max(0, Math.min(1, state.downloadedBytes / state.contentLength))
      : null;
  const errorMessage = state.kind === "failed" ? state.errorMessage : null;

  const handleDownload = useCallback(() => {
    if (!version) {
      return;
    }
    downloadUpdate(version);
  }, [downloadUpdate, version]);

  const handleInstall = useCallback(() => {
    if (!version) {
      return;
    }
    installUpdate(version);
  }, [installUpdate, version]);

  const handleDevtoolsDownload = useCallback(() => {
    showDevtoolsOtaPreview("downloading");
  }, [showDevtoolsOtaPreview]);

  const handleDevtoolsInstall = useCallback(() => {
    clearDevtoolsOtaPreview();
  }, [clearDevtoolsOtaPreview]);

  if (!updaterEnabled) {
    return DISABLED_UPDATE_CONTROL;
  }

  if (devtoolsPreview) {
    return {
      status: devtoolsPreview.status,
      version: devtoolsPreview.version,
      progress: devtoolsPreview.progress,
      errorMessage:
        devtoolsPreview.status === "failed"
          ? "Devtools OTA failure preview."
          : null,
      downloadStarting: false,
      installing: false,
      downloadUpdate: handleDevtoolsDownload,
      installUpdate: handleDevtoolsInstall,
    };
  }

  return {
    status,
    version,
    progress,
    errorMessage,
    downloadStarting,
    installing,
    downloadUpdate: handleDownload,
    installUpdate: handleInstall,
  };
}

function unwrapResult<T>(result: Result<T, string>): T {
  if (result.status === "ok") {
    return result.data;
  }

  throw new Error(result.error);
}

function readErrorMessage(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }

  if (typeof error === "string") {
    return error;
  }

  return "Unknown update error.";
}
