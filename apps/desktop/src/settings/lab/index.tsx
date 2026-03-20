import { useQuery } from "@tanstack/react-query";
import { getIdentifier } from "@tauri-apps/api/app";
import { arch, platform } from "@tauri-apps/plugin-os";

import { commands as openerCommands } from "@hypr/plugin-opener2";
import { commands as windowsCommands } from "@hypr/plugin-windows";
import { Button } from "@hypr/ui/components/ui/button";
import { cn } from "@hypr/utils";

export function SettingsLab() {
  const handleOpenControlWindow = async () => {
    await windowsCommands.windowShow({ type: "control" });
  };

  return (
    <div className="flex flex-col gap-4 pt-3">
      <div className="flex items-center justify-between gap-4">
        <div className="flex-1">
          <h3 className="mb-1 text-sm font-medium">Control Overlay</h3>
          <p className="text-xs text-neutral-600">
            Floating window for quick access to recording controls.
          </p>
        </div>
        <Button variant="outline" size="sm" onClick={handleOpenControlWindow}>
          Open
        </Button>
      </div>

      <DownloadButtons />
    </div>
  );
}

function DownloadButtons() {
  const platformName = platform();
  const archQuery = useQuery({
    queryKey: ["target-arch"],
    queryFn: () => arch(),
    staleTime: Infinity,
  });
  const identifierQuery = useQuery({
    queryKey: ["app-identifier"],
    queryFn: () => getIdentifier(),
    staleTime: Infinity,
  });

  const isDev = identifierQuery.data === "com.hyprnote.dev";
  const isNightly = identifierQuery.data === "com.hyprnote.nightly";

  const channels: Array<"stable" | "nightly"> = isDev
    ? ["stable", "nightly"]
    : isNightly
      ? ["stable"]
      : ["nightly"];

  const getDownloadUrl = (channel: "stable" | "nightly") => {
    const targetArch = archQuery.data;
    if (platformName === "macos") {
      if (targetArch === "aarch64") {
        return `https://desktop2.hyprnote.com/download/latest/dmg-aarch64?channel=${channel}`;
      }
      return `https://desktop2.hyprnote.com/download/latest/dmg-x86_64?channel=${channel}`;
    }
    if (platformName === "linux") {
      if (targetArch === "aarch64") {
        return `https://desktop2.hyprnote.com/download/latest/appimage-aarch64?channel=${channel}`;
      }
      return `https://desktop2.hyprnote.com/download/latest/appimage-x86_64?channel=${channel}`;
    }
    return null;
  };

  if (!identifierQuery.data || !getDownloadUrl(channels[0])) {
    return null;
  }

  return (
    <>
      {channels.map((channel) => {
        const downloadUrl = getDownloadUrl(channel);
        if (!downloadUrl) return null;

        return (
          <div
            key={channel}
            className="flex items-center justify-between gap-4"
          >
            <div className="flex-1">
              <h3 className="mb-1 text-sm font-medium">
                {channel === "nightly" ? "Nightly" : "Stable"} Build
              </h3>
              <p className="text-xs text-neutral-600">
                {channel === "nightly"
                  ? "Try new features early. May be less stable."
                  : "The latest stable release."}
              </p>
            </div>
            <Button
              size="sm"
              className={cn([
                "border bg-linear-to-br text-white",
                channel === "nightly"
                  ? "from-[#03BCF1] to-[#127FE5]"
                  : "from-[#535353] to-[#000000]",
              ])}
              onClick={() => openerCommands.openUrl(downloadUrl, null)}
            >
              Download
            </Button>
          </div>
        );
      })}
    </>
  );
}
