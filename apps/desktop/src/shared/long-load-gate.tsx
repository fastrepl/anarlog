import { useQuery } from "@tanstack/react-query";
import { relaunch } from "@tauri-apps/plugin-process";
import { useEffect, useState, type ReactNode } from "react";

import {
  getStartupStatus,
  waitUntilReady,
  type StartupStatus,
} from "@anlg/plugin-db";
import { ArrowClockwise } from "@anlg/ui/components/icons";
import { Button } from "@anlg/ui/components/ui/button";
import { cn } from "@anlg/utils";

import { BrandLoadingView } from "./brand-loading-view";

import { captureOperationalError } from "~/error-reporting";

export const LONG_LOAD_SPLASH_DELAY_MS = 400;
const STARTUP_STATUS_REFETCH_INTERVAL_MS = 250;

function dismissBootSplash() {
  document.getElementById("boot-splash")?.remove();
}

export function LongLoadGate({ children }: { children: ReactNode }) {
  const [ready, setReady] = useState(false);
  const [showSplash, setShowSplash] = useState(false);
  const [error, setError] = useState<Error | null>(null);
  const { data: startupStatus } = useQuery({
    queryKey: ["database-startup-status"],
    queryFn: getStartupStatus,
    enabled: !ready && !error,
    refetchInterval: ready ? false : STARTUP_STATUS_REFETCH_INTERVAL_MS,
    retry: false,
  });

  useEffect(() => {
    if (ready || showSplash || error) {
      dismissBootSplash();
    }
  }, [ready, showSplash, error]);

  useEffect(() => {
    let cancelled = false;
    const splashTimer = window.setTimeout(() => {
      if (!cancelled) {
        setShowSplash(true);
      }
    }, LONG_LOAD_SPLASH_DELAY_MS);

    void waitUntilReady()
      .then(() => {
        if (!cancelled) {
          setReady(true);
        }
      })
      .catch((cause) => {
        if (!cancelled) {
          setError(cause instanceof Error ? cause : new Error(String(cause)));
        }
      })
      .finally(() => {
        window.clearTimeout(splashTimer);
      });

    return () => {
      cancelled = true;
      window.clearTimeout(splashTimer);
    };
  }, []);

  if (error) {
    return <StartupErrorView error={error} />;
  }
  if (ready) {
    return children;
  }
  if (!showSplash) {
    return null;
  }
  return <BrandLoadingView detail={getStartupDetail(startupStatus)} />;
}

function getStartupDetail(status: StartupStatus | undefined) {
  switch (status?.phase) {
    case "preparing_database":
      return "Checking your local database. This is taking longer than expected.";
    case "migrating_database": {
      const progress =
        status.migrationCurrent && status.migrationTotal
          ? ` (${status.migrationCurrent} of ${status.migrationTotal})`
          : "";
      return `Migrating your local database${progress}. This may take a few minutes.`;
    }
    case "importing_legacy_data":
      return "Importing your existing notes. This may take a few minutes.";
    case "configuring_cloudsync":
      return "Preparing sync. This should only take a moment.";
    case "ready":
    case "failed":
    case undefined:
      return undefined;
  }
}

function StartupErrorView({ error }: { error: Error }) {
  const needsUpdate = error.message.includes(
    "created by a newer version of Anarlog",
  );

  const handleRestart = async () => {
    try {
      await relaunch();
    } catch (cause) {
      captureOperationalError(cause, {
        operation: "app_restart",
      });
    }
  };

  return (
    <div
      data-tauri-drag-region
      className={cn([
        "bg-background flex h-screen w-screen items-center justify-center p-6",
      ])}
    >
      <div className="flex max-w-sm flex-col items-center gap-4 text-center">
        <h1 className="text-foreground text-base font-semibold">
          {needsUpdate ? "Anarlog needs an update" : "Anarlog could not start"}
        </h1>
        <p className="text-muted-foreground text-sm leading-relaxed">
          {needsUpdate
            ? "Your data was created by a newer version of Anarlog, and this older version cannot open it. Your existing data was left unchanged. Please install the latest version of Anarlog."
            : "Your existing data was left unchanged. Please restart the app. If the problem continues, contact support."}
        </p>
        {needsUpdate ? null : (
          <Button size="sm" onClick={() => void handleRestart()}>
            <ArrowClockwise className="mr-1.5 h-3.5 w-3.5" />
            Restart App
          </Button>
        )}
      </div>
    </div>
  );
}
