import { Trans, useLingui } from "@lingui/react/macro";
import { useMutation, useQuery } from "@tanstack/react-query";
import { downloadDir, homeDir } from "@tauri-apps/api/path";
import { open as selectFolder } from "@tauri-apps/plugin-dialog";

import { commands as openerCommands } from "@anlg/plugin-opener2";
import { CircleNotch, FolderSimple } from "@anlg/ui/components/icons";
import { Button } from "@anlg/ui/components/ui/button";

import { displayPath } from "./path-utils";

import {
  setSettingValue,
  useStoredSettingValuesQuery,
} from "~/settings/queries";

export function ExportLocationRow() {
  const { t } = useLingui();
  const settings = useStoredSettingValuesQuery();
  const directory = settings.data?.values.export_directory;
  const { data: home } = useQuery({ queryKey: ["home-dir"], queryFn: homeDir });
  const downloads = useQuery({
    queryKey: ["download-dir"],
    queryFn: downloadDir,
    enabled: !directory,
  });
  const path = directory || downloads.data;
  const changeMutation = useMutation({
    mutationFn: async (action: "choose" | "reset") => {
      if (action === "reset") {
        await setSettingValue("export_directory", "");
        return;
      }

      const selected = await selectFolder({
        title: t`Choose export folder`,
        directory: true,
        multiple: false,
        defaultPath: path,
      });
      if (selected && selected !== path) {
        await setSettingValue("export_directory", selected);
      }
    },
  });
  const disabled =
    settings.isLoading || !!settings.error || changeMutation.isPending;

  return (
    <div>
      <div className="grid grid-cols-[minmax(0,1fr)_9rem] items-center gap-3">
        <button
          type="button"
          className="hover:bg-muted/40 flex min-w-0 items-center gap-2 rounded-lg px-2 py-2 text-left transition-colors"
          disabled={settings.isLoading || !!settings.error || !path}
          onClick={() => {
            if (path) void openerCommands.openPath(path, null);
          }}
        >
          <FolderSimple className="text-muted-foreground size-4 shrink-0" />
          <div className="min-w-0">
            <p className="text-sm font-medium">
              <Trans>Export location</Trans>
            </p>
            <p className="text-muted-foreground truncate text-xs" title={path}>
              {displayPath(path, home)}
            </p>
          </div>
        </button>
        <Button
          type="button"
          variant="outline"
          className="h-9 w-full justify-center"
          disabled={disabled}
          onClick={() => changeMutation.mutate("choose")}
        >
          {changeMutation.isPending && (
            <CircleNotch className="size-4 animate-spin" aria-hidden="true" />
          )}
          <Trans>Choose folder</Trans>
        </Button>
      </div>
      <p className="text-muted-foreground px-2 text-xs">
        <Trans>Save PDF, text, Markdown, and Org exports to this folder.</Trans>
      </p>
      {directory && (
        <Button
          type="button"
          variant="link"
          size="sm"
          className="px-2"
          disabled={disabled}
          onClick={() => changeMutation.mutate("reset")}
        >
          <Trans>Reset to Downloads</Trans>
        </Button>
      )}
      {(settings.error || downloads.error || changeMutation.error) && (
        <p role="alert" className="mt-1 px-2 text-xs text-red-500">
          <Trans>Could not update the export folder</Trans>
        </p>
      )}
    </div>
  );
}
