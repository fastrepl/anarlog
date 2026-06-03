import { useMutation, useQuery } from "@tanstack/react-query";
import { DownloadIcon, RefreshCwIcon, RotateCwIcon } from "lucide-react";
import { AnimatePresence, motion } from "motion/react";
import { useCallback, useMemo, useState, type ReactNode } from "react";

import {
  commands as updaterCommands,
  events as updaterEvents,
  type Result,
} from "@hypr/plugin-updater2";
import { cn } from "@hypr/utils";

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

type UpdateEventState = {
  status: UpdateBannerStatus;
  version: string;
  downloadedBytes: number;
  contentLength: number | null;
  errorMessage: string | null;
};

type UpdateCheckState = {
  version: string;
  ready: boolean;
} | null;

const UPDATE_CHECK_QUERY_KEY = ["updater2", "check"] as const;
const UPDATE_CHECK_INTERVAL_MS = 30 * 60 * 1000;

export function useDesktopUpdateControl(): DesktopUpdateControl {
  const [eventState, setEventState] = useState<UpdateEventState | null>(null);
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
    let cancelled = false;
    const unlistenFns: Array<() => void> = [];

    const listen = async () => {
      const [
        unlistenDownloading,
        unlistenProgress,
        unlistenReady,
        unlistenFailed,
        unlistenUpdated,
      ] = await Promise.all([
        updaterEvents.updateDownloadingEvent.listen(({ payload }) => {
          setEventState({
            status: "downloading",
            version: payload.version,
            downloadedBytes: 0,
            contentLength: null,
            errorMessage: null,
          });
        }),
        updaterEvents.updateDownloadProgressEvent.listen(({ payload }) => {
          setEventState((current) => {
            const downloadedBytes =
              current?.version === payload.version
                ? current.downloadedBytes + payload.chunk_length
                : payload.chunk_length;

            return {
              status: "downloading",
              version: payload.version,
              downloadedBytes,
              contentLength: payload.content_length,
              errorMessage: null,
            };
          });
        }),
        updaterEvents.updateReadyEvent.listen(({ payload }) => {
          setEventState({
            status: "ready",
            version: payload.version,
            downloadedBytes: 0,
            contentLength: null,
            errorMessage: null,
          });
        }),
        updaterEvents.updateDownloadFailedEvent.listen(({ payload }) => {
          setEventState({
            status: "failed",
            version: payload.version,
            downloadedBytes: 0,
            contentLength: null,
            errorMessage: "Failed to download update.",
          });
        }),
        updaterEvents.updatedEvent.listen(({ payload }) => {
          setAcknowledgedVersion(payload.current);
          setEventState(null);
        }),
      ]);

      if (cancelled) {
        unlistenDownloading();
        unlistenProgress();
        unlistenReady();
        unlistenFailed();
        unlistenUpdated();
        return;
      }

      unlistenFns.push(
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

  const updateCheck = useQuery({
    queryKey: UPDATE_CHECK_QUERY_KEY,
    queryFn: async (): Promise<UpdateCheckState> => {
      const version = unwrapResult(await updaterCommands.check());

      if (!version) {
        return null;
      }

      return {
        version,
        ready: unwrapResult(await updaterCommands.isDownloaded(version)),
      };
    },
    refetchInterval: UPDATE_CHECK_INTERVAL_MS,
    retry: false,
    staleTime: UPDATE_CHECK_INTERVAL_MS,
  });

  const { mutate: downloadUpdate, isPending: downloadStarting } = useMutation({
    mutationFn: async (version: string) =>
      unwrapResult(await updaterCommands.download(version)),
    onMutate: (version) => {
      setEventState({
        status: "downloading",
        version,
        downloadedBytes: 0,
        contentLength: null,
        errorMessage: null,
      });
    },
    onError: (error, version) => {
      setEventState({
        status: "failed",
        version,
        downloadedBytes: 0,
        contentLength: null,
        errorMessage: readErrorMessage(error),
      });
    },
    onSuccess: (_data, version) => {
      setEventState((current) =>
        current?.status === "ready"
          ? current
          : {
              status: "ready",
              version,
              downloadedBytes: 0,
              contentLength: null,
              errorMessage: null,
            },
      );
    },
  });

  const { mutate: installUpdate, isPending: installing } = useMutation({
    mutationFn: async (version: string) => {
      const result = unwrapResult(await updaterCommands.install(version));
      unwrapResult(await updaterCommands.postinstall(result));
    },
    onError: (error, version) => {
      setEventState({
        status: "failed",
        version,
        downloadedBytes: 0,
        contentLength: null,
        errorMessage: readErrorMessage(error),
      });
    },
  });

  const checkedUpdate =
    updateCheck.data && updateCheck.data.version !== acknowledgedVersion
      ? updateCheck.data
      : null;
  const version = eventState?.version ?? checkedUpdate?.version ?? null;
  const status: UpdateBannerStatus | null = eventState
    ? eventState.status
    : checkedUpdate
      ? checkedUpdate.ready
        ? "ready"
        : "available"
      : null;
  const progress = useMemo(() => {
    if (
      !eventState ||
      eventState.status !== "downloading" ||
      !eventState.contentLength
    ) {
      return null;
    }

    return Math.max(
      0,
      Math.min(1, eventState.downloadedBytes / eventState.contentLength),
    );
  }, [eventState]);

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
    errorMessage: eventState?.errorMessage ?? null,
    downloadStarting,
    installing,
    downloadUpdate: handleDownload,
    installUpdate: handleInstall,
  };
}

export function TimelineUpdateBanner({
  update,
}: {
  update: DesktopUpdateControl;
}) {
  return (
    <AnimatePresence initial={false}>
      {update.status && update.version ? (
        <motion.div
          key="timeline-update-banner"
          initial={{ height: 0, opacity: 0 }}
          animate={{ height: "auto", opacity: 1 }}
          exit={{ height: 0, opacity: 0 }}
          transition={{ duration: 0.18, ease: "easeOut" }}
          className="w-full shrink-0 overflow-hidden"
        >
          <UpdateBanner
            status={update.status}
            progress={update.progress}
            errorMessage={update.errorMessage}
            downloadStarting={update.downloadStarting}
            installing={update.installing}
            actionIcon={bannerActionIcon(update.status)}
            onDownload={update.downloadUpdate}
            onInstall={update.installUpdate}
          />
        </motion.div>
      ) : null}
    </AnimatePresence>
  );
}

export function SidebarTimelineUpdateButton({
  update,
}: {
  update: DesktopUpdateControl;
}) {
  if (!update.status || !update.version) {
    return null;
  }

  const isDownloading = update.status === "downloading";
  const isReady = update.status === "ready";
  const label = sidebarUpdateLabel(update.status, update.progress);

  return (
    <button
      type="button"
      aria-label={label}
      title={label}
      data-tauri-drag-region="false"
      disabled={isDownloading || update.downloadStarting || update.installing}
      className={cn([
        "relative flex size-7 shrink-0 items-center justify-center rounded-full",
        "bg-blue-600 text-white shadow-sm transition-colors hover:bg-blue-700",
        "focus-visible:ring-2 focus-visible:ring-blue-600 focus-visible:ring-offset-2 focus-visible:outline-hidden",
        "disabled:cursor-default disabled:bg-blue-600 disabled:text-white disabled:opacity-70 disabled:hover:bg-blue-600",
      ])}
      onClick={isReady ? update.installUpdate : update.downloadUpdate}
    >
      {isDownloading ? (
        <SidebarCircularProgress progress={update.progress} />
      ) : (
        <span className="relative z-10 flex items-center justify-center">
          {sidebarActionIcon(update.status)}
        </span>
      )}
    </button>
  );
}

function UpdateBanner({
  status,
  progress,
  errorMessage,
  downloadStarting,
  installing,
  actionIcon,
  onDownload,
  onInstall,
}: {
  status: UpdateBannerStatus;
  progress: number | null;
  errorMessage: string | null;
  downloadStarting: boolean;
  installing: boolean;
  actionIcon?: ReactNode;
  onDownload: () => void;
  onInstall: () => void;
}) {
  return (
    <div
      role="status"
      aria-live="polite"
      data-testid="timeline-update-banner"
      className="flex h-9 w-full shrink-0 items-center justify-center border-y border-stone-200/70 bg-stone-100/80 px-3 font-mono text-xs text-neutral-800"
    >
      <div className="flex min-w-0 items-center justify-center gap-3">
        <BannerBody status={status} errorMessage={errorMessage} />

        <BannerAction
          status={status}
          progress={progress}
          downloadStarting={downloadStarting}
          installing={installing}
          actionIcon={actionIcon}
          onDownload={onDownload}
          onInstall={onInstall}
        />
      </div>
    </div>
  );
}

function BannerBody({
  status,
  errorMessage,
}: {
  status: UpdateBannerStatus;
  errorMessage: string | null;
}) {
  if (status === "available" || status === "downloading") {
    return <span className="shrink-0">New version available</span>;
  }

  if (status === "ready") {
    return <span className="shrink-0">Update ready</span>;
  }

  return (
    <span className="shrink-0" title={errorMessage ?? undefined}>
      Update failed
    </span>
  );
}

function BannerAction({
  status,
  progress,
  downloadStarting,
  installing,
  actionIcon,
  onDownload,
  onInstall,
}: {
  status: UpdateBannerStatus;
  progress: number | null;
  downloadStarting: boolean;
  installing: boolean;
  actionIcon?: ReactNode;
  onDownload: () => void;
  onInstall: () => void;
}) {
  if (status === "available" || status === "failed") {
    return (
      <button
        type="button"
        data-tauri-drag-region="false"
        onClick={onDownload}
        disabled={downloadStarting}
        className={cn([
          "inline-flex h-7 w-[104px] shrink-0 items-center justify-center gap-1.5 overflow-hidden rounded-full px-3 font-medium",
          "bg-neutral-950 text-white transition-colors hover:bg-neutral-800",
          "focus-visible:ring-2 focus-visible:ring-neutral-900 focus-visible:outline-hidden",
          "disabled:cursor-not-allowed disabled:opacity-60",
        ])}
      >
        <ActionIcon icon={actionIcon} />
        {downloadStarting
          ? "Starting..."
          : status === "failed"
            ? "Retry"
            : "Download"}
      </button>
    );
  }

  if (status === "downloading") {
    return <DownloadProgress progress={progress} />;
  }

  return (
    <button
      type="button"
      data-tauri-drag-region="false"
      onClick={onInstall}
      disabled={installing}
      className={cn([
        "inline-flex h-7 shrink-0 items-center justify-center gap-1.5 overflow-hidden rounded-full px-3 font-medium",
        "bg-blue-600 text-white transition-colors hover:bg-blue-700",
        "focus-visible:ring-2 focus-visible:ring-blue-600 focus-visible:ring-offset-2 focus-visible:outline-hidden",
        "disabled:cursor-not-allowed disabled:opacity-60",
      ])}
    >
      <ActionIcon icon={actionIcon} />
      {installing ? "Restarting" : "Restart"}
    </button>
  );
}

function DownloadProgress({ progress }: { progress: number | null }) {
  const pct = Math.max(0, Math.min(100, Math.round((progress ?? 0) * 100)));

  return (
    <div
      aria-label={
        progress === null
          ? "Downloading update"
          : `Downloading update, ${pct}% complete`
      }
      className="relative inline-flex h-7 w-[104px] shrink-0 items-center justify-center overflow-hidden rounded-full border border-stone-300 bg-white px-3 font-medium text-neutral-800"
    >
      {progress === null ? null : (
        <span
          aria-hidden="true"
          className="absolute inset-y-0 left-0 bg-neutral-950/15"
          style={{ width: `${pct}%` }}
        />
      )}
      <span className="relative z-10 tabular-nums">
        {progress === null ? "Downloading" : `${pct}%`}
      </span>
    </div>
  );
}

function ActionIcon({ icon }: { icon?: ReactNode }) {
  return icon ? <span aria-hidden="true">{icon}</span> : null;
}

function SidebarCircularProgress({ progress }: { progress: number | null }) {
  const pct = Math.max(0, Math.min(1, progress ?? 0));
  const radius = 7.5;
  const circumference = 2 * Math.PI * radius;

  return (
    <svg
      aria-hidden="true"
      className="pointer-events-none absolute top-1/2 left-1/2 size-[18px] -translate-x-1/2 -translate-y-1/2 -rotate-90"
      viewBox="0 0 18 18"
    >
      <circle
        cx="9"
        cy="9"
        r={radius}
        fill="none"
        stroke="currentColor"
        strokeOpacity="0.14"
        strokeWidth="1.5"
      />
      <circle
        cx="9"
        cy="9"
        r={radius}
        fill="none"
        stroke="currentColor"
        strokeLinecap="round"
        strokeWidth="1.5"
        strokeDasharray={circumference}
        strokeDashoffset={circumference * (1 - pct)}
        className="transition-[stroke-dashoffset] duration-200 ease-out"
      />
    </svg>
  );
}

function sidebarUpdateLabel(
  status: UpdateBannerStatus,
  progress: number | null,
): string {
  if (status === "ready") {
    return "Restart to update";
  }

  if (status === "downloading") {
    if (progress === null) {
      return "Downloading update";
    }

    return `Downloading update, ${Math.round(progress * 100)}% complete`;
  }

  if (status === "failed") {
    return "Retry update";
  }

  return "Download update";
}

function sidebarActionIcon(status: UpdateBannerStatus): ReactNode {
  if (status === "ready") {
    return <RotateCwIcon size={14} aria-hidden="true" />;
  }

  return <DownloadIcon size={14} aria-hidden="true" />;
}

function bannerActionIcon(status: UpdateBannerStatus): ReactNode {
  if (status === "failed") {
    return <RefreshCwIcon size={12} aria-hidden="true" />;
  }
  if (status === "ready") {
    return <RotateCwIcon size={12} aria-hidden="true" />;
  }
  return <DownloadIcon size={12} aria-hidden="true" />;
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
