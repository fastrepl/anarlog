import { type UnlistenFn } from "@tauri-apps/api/event";
import { message } from "@tauri-apps/plugin-dialog";
import { XIcon } from "lucide-react";
import { useCallback, useEffect, useState } from "react";

import { commands, events } from "@hypr/plugin-updater2";
import { Button } from "@hypr/ui/components/ui/button";
import { cn } from "@hypr/utils";

export function UpdateBanner() {
  const { version } = useUpdate();
  const [dismissed, setDismissed] = useState(false);

  const handleInstallUpdate = useCallback(async () => {
    if (!version) {
      return;
    }
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
  }, [version]);

  if (!version || dismissed) {
    return null;
  }

  return (
    <div
      className={cn([
        "flex items-center justify-center gap-3 px-4 py-1.5",
        "bg-neutral-100 text-sm text-neutral-700",
      ])}
    >
      <span>v{version} available</span>
      <Button
        size="sm"
        variant="outline"
        onClick={handleInstallUpdate}
        className="h-7 px-3 text-xs font-medium"
      >
        Update & Restart
      </Button>
      <button
        type="button"
        onClick={() => setDismissed(true)}
        className="ml-1 text-neutral-400 hover:text-neutral-600"
      >
        <XIcon size={14} />
      </button>
    </div>
  );
}

function useUpdate() {
  const [version, setVersion] = useState<string | null>(null);

  useEffect(() => {
    let unlisten: UnlistenFn | null = null;

    void events.updateReadyEvent
      .listen(({ payload }) => {
        setVersion(payload.version);
      })
      .then((f) => {
        unlisten = f;
      });

    return () => {
      unlisten?.();
    };
  }, []);

  return { version };
}
