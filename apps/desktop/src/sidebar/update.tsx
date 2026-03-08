import { type UnlistenFn } from "@tauri-apps/api/event";
import { message } from "@tauri-apps/plugin-dialog";
import { relaunch } from "@tauri-apps/plugin-process";
import { useCallback, useEffect, useState } from "react";

import { commands, events } from "@hypr/plugin-updater2";
import { Button } from "@hypr/ui/components/ui/button";
import { cn } from "@hypr/utils";

export function Update() {
  const { version } = useUpdate();

  const handleInstallUpdate = useCallback(async () => {
    if (!version) {
      return;
    }
    const result = await commands.install(version);
    if (result.status === "ok") {
      await relaunch();
    } else {
      await message(`Failed to install update: ${result.error}`, {
        title: "Update Failed",
        kind: "error",
      });
    }
  }, [version]);

  if (!version) {
    return null;
  }

  return (
    <Button
      size="sm"
      onClick={handleInstallUpdate}
      className={cn([
        "rounded-full px-3",
        "bg-linear-to-t from-stone-600 to-stone-500",
        "hover:from-stone-500 hover:to-stone-400",
      ])}
    >
      Install Update
    </Button>
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
