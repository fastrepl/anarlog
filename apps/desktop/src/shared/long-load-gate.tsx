import { ArrowClockwise } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { useQuery } from "@tanstack/react-query";
import { relaunch } from "@tauri-apps/plugin-process";
import { useEffect, useState, type ReactNode } from "react";

import { colors } from "@anlg/design-system/tokens.stylex";
import {
  getStartupStatus,
  waitUntilReady,
  type StartupStatus,
} from "@anlg/plugin-db";
import { Button } from "@anlg/ui/components/ui/button";

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
    <div data-tauri-drag-region {...stylex.props(styles.errorRoot)}>
      <div {...stylex.props(styles.errorContent)}>
        <h1 {...stylex.props(styles.errorTitle)}>
          {needsUpdate ? "Anarlog needs an update" : "Anarlog could not start"}
        </h1>
        <p {...stylex.props(styles.errorDescription)}>
          {needsUpdate
            ? "Your data was created by a newer version of Anarlog, and this older version cannot open it. Your existing data was left unchanged. Please install the latest version of Anarlog."
            : "Your existing data was left unchanged. Please restart the app. If the problem continues, contact support."}
        </p>
        {needsUpdate ? null : (
          <Button size="sm" onClick={() => void handleRestart()}>
            <ArrowClockwise {...stylex.props(styles.buttonIcon)} />
            Restart App
          </Button>
        )}
      </div>
    </div>
  );
}

const styles = stylex.create({
  buttonIcon: {
    height: "0.875rem",
    marginRight: "0.375rem",
    width: "0.875rem",
  },
  errorContent: {
    alignItems: "center",
    display: "flex",
    flexDirection: "column",
    gap: "1rem",
    maxWidth: "24rem",
    textAlign: "center",
  },
  errorDescription: {
    color: colors.mutedForeground,
    fontSize: "0.875rem",
    lineHeight: 1.625,
  },
  errorRoot: {
    alignItems: "center",
    backgroundColor: colors.background,
    display: "flex",
    height: "100vh",
    justifyContent: "center",
    padding: "1.5rem",
    width: "100vw",
  },
  errorTitle: {
    color: colors.foreground,
    fontSize: "1rem",
    fontWeight: 600,
  },
});
