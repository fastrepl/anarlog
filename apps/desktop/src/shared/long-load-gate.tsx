import { ArrowClockwise } from "@phosphor-icons/react";
import { relaunch } from "@tauri-apps/plugin-process";
import { useEffect, useState, type ReactNode } from "react";

import { waitUntilReady } from "@anlg/plugin-db";
import { Button } from "@anlg/ui/components/ui/button";
import { cn } from "@anlg/utils";

import { BrandLoadingView } from "./brand-loading-view";

import { captureOperationalError } from "~/error-reporting";

export const LONG_LOAD_SPLASH_DELAY_MS = 400;

function dismissBootSplash() {
  document.getElementById("boot-splash")?.remove();
}

export function LongLoadGate({ children }: { children: ReactNode }) {
  const [ready, setReady] = useState(false);
  const [showSplash, setShowSplash] = useState(false);
  const [error, setError] = useState<Error | null>(null);

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
  return <BrandLoadingView />;
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
