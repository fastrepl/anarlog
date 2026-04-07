import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { getIdentifier } from "@tauri-apps/api/app";
import { arch, platform } from "@tauri-apps/plugin-os";
import { relaunch } from "@tauri-apps/plugin-process";

import { commands as openerCommands } from "@hypr/plugin-opener2";
import { Button } from "@hypr/ui/components/ui/button";
import { Switch } from "@hypr/ui/components/ui/switch";
import { cn } from "@hypr/utils";

import { SettingsPageTitle } from "~/settings/page-title";
import { commands } from "~/types/tauri.gen";

export function SettingsLab() {
  return (
    <div className="flex flex-col gap-6">
      <SettingsPageTitle title="Lab" />
      <V1p1PreviewToggle />
      <DownloadButtons />
    </div>
  );
}

function V1p1PreviewToggle() {
  const queryClient = useQueryClient();

  const { data: enabled = false } = useQuery({
    queryKey: ["char_v1p1_preview"],
    queryFn: async () => {
      const result = await commands.getCharV1p1Preview();
      return result.status === "ok" ? result.data : false;
    },
  });

  const mutation = useMutation({
    mutationFn: async (v: boolean) => {
      await commands.setCharV1p1Preview(v);
    },
    onSuccess: async (_data, v) => {
      queryClient.setQueryData(["char_v1p1_preview"], v);
      await relaunch();
    },
  });

  return (
    <div className="flex items-center justify-between gap-4">
      <div className="flex-1">
        <h3 className="mb-1 text-sm font-medium">New Layout</h3>
        <p className="text-xs text-neutral-600">
          Try the new layout experience. The app will restart to apply.
        </p>
      </div>
      <Switch
        checked={enabled}
        onCheckedChange={(v) => mutation.mutate(v)}
        disabled={mutation.isPending}
      />
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
