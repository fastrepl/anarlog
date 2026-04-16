import { useRouterState } from "@tanstack/react-router";
import { type UnlistenFn } from "@tauri-apps/api/event";
import { message } from "@tauri-apps/plugin-dialog";
import { useCallback, useEffect, useRef, useState } from "react";

import { commands, events } from "@hypr/plugin-updater2";
import { Button } from "@hypr/ui/components/ui/button";
import { cn } from "@hypr/utils";

import { useTabs } from "~/store/zustand/tabs";

export function UpdateBanner() {
  const { version, progress } = useUpdate();
  const isHomeOnly = useShouldShowUpdateBanner();
  const [dismissed, setDismissed] = useState(false);
  const [installing, setInstalling] = useState(false);

  const handleInstallUpdate = useCallback(async () => {
    if (!version) {
      return;
    }
    setInstalling(true);
    const installResult = await commands.install(version);
    if (installResult.status !== "ok") {
      await message(`Failed to install update: ${installResult.error}`, {
        title: "Update Failed",
        kind: "error",
      });
      return;
    }

    const postInstallResult = await commands.postinstall(installResult.data);
    if (postInstallResult.status !== "ok") {
      await message(`Failed to apply update: ${postInstallResult.error}`, {
        title: "Update Failed",
        kind: "error",
      });
    }
    setInstalling(false);
  }, [version]);

  if (!version || dismissed || !isHomeOnly) {
    return null;
  }

  const isDownloading = progress !== null && progress < 1;

  return (
    <div
      className={cn([
        "flex items-center justify-center gap-3 px-4 py-1.5",
        "bg-neutral-50 text-sm text-neutral-700",
      ])}
    >
      <span>New version {version} is ready.</span>
      {isDownloading ? (
        <DownloadProgress progress={progress} />
      ) : (
        <Button
          size="sm"
          onClick={handleInstallUpdate}
          disabled={installing}
          className="h-7 bg-black px-3 text-xs font-medium text-white hover:bg-neutral-800"
        >
          {installing ? "Installing..." : "Update & Restart"}
        </Button>
      )}
      <Button
        type="button"
        size="sm"
        variant="ghost"
        onClick={() => setDismissed(true)}
        className="h-7 px-2 text-xs font-medium text-neutral-500 hover:bg-transparent hover:text-neutral-700"
      >
        Later
      </Button>
    </div>
  );
}

function DownloadProgress({ progress }: { progress: number }) {
  const pct = Math.round(progress * 100);
  const width = `${pct}%`;
  return (
    <div className="flex items-center gap-2">
      <div className="h-1.5 w-32 overflow-hidden rounded-full bg-neutral-200">
        <div
          className="h-full rounded-full bg-neutral-500 transition-all duration-300"
          style={{ width }}
        />
      </div>
      <span className="text-xs text-neutral-500">{pct}%</span>
    </div>
  );
}

export function useShouldShowUpdateBanner() {
  const pathname = useRouterState({
    select: (state) => state.location.pathname,
  });

  return useTabs((state) => {
    const isMain2Home = pathname === "/app/main2" || pathname === "/app/main2/";

    return isMain2Home && state.currentTab === null && state.tabs.length === 0;
  });
}

function useUpdate() {
  const [version, setVersion] = useState<string | null>(null);
  const [progress, setProgress] = useState<number | null>(null);
  const downloadedRef = useRef(0);

  useEffect(() => {
    const unlistens: UnlistenFn[] = [];

    void events.updateDownloadingEvent
      .listen(({ payload }) => {
        setVersion(payload.version);
        setProgress(0);
        downloadedRef.current = 0;
      })
      .then((f) => unlistens.push(f));

    void events.updateDownloadProgressEvent
      .listen(({ payload }) => {
        downloadedRef.current += payload.chunk_length;
        if (payload.content_length) {
          setProgress(
            Math.min(downloadedRef.current / payload.content_length, 1),
          );
        }
      })
      .then((f) => unlistens.push(f));

    void events.updateReadyEvent
      .listen(({ payload }) => {
        setVersion(payload.version);
        setProgress(null);
      })
      .then((f) => unlistens.push(f));

    void events.updateDownloadFailedEvent
      .listen(() => {
        setProgress(null);
      })
      .then((f) => unlistens.push(f));

    return () => {
      for (const unlisten of unlistens) {
        unlisten();
      }
    };
  }, []);

  return { version, progress };
}
