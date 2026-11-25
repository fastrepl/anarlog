import { Trans } from "@lingui/react/macro";
import { useQuery } from "@tanstack/react-query";
import { homeDir } from "@tauri-apps/api/path";

import { commands as openerCommands } from "@anlg/plugin-opener2";
import { commands as settingsCommands } from "@anlg/plugin-settings";
import { FolderSimple } from "@anlg/ui/components/icons";
import { Button } from "@anlg/ui/components/ui/button";

import { displayPath } from "./path-utils";

export function DataLocationRow() {
  const { data: home } = useQuery({ queryKey: ["home-dir"], queryFn: homeDir });
  const dataDir = useQuery({
    queryKey: ["vault-base"],
    queryFn: async () => {
      const result = await settingsCommands.vaultBase();
      if (result.status === "error") {
        throw new Error(result.error);
      }
      return result.data;
    },
  });
  const path = dataDir.data;
  const disabled = dataDir.isLoading || !!dataDir.error || !path;
  const openFolder = () => {
    if (path) void openerCommands.openPath(path, null);
  };

  return (
    <div>
      <div className="grid grid-cols-[minmax(0,1fr)_9rem] items-center gap-3">
        <button
          type="button"
          className="hover:bg-muted/40 flex min-w-0 items-center gap-2 rounded-lg px-2 py-2 text-left transition-colors"
          disabled={disabled}
          onClick={openFolder}
        >
          <FolderSimple className="text-muted-foreground size-4 shrink-0" />
          <div className="min-w-0">
            <p className="text-sm font-medium">
              <Trans>Data location</Trans>
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
          onClick={openFolder}
        >
          <Trans>Open folder</Trans>
        </Button>
      </div>
      {dataDir.error && (
        <p role="alert" className="mt-1 px-2 text-xs text-red-500">
          <Trans>Could not load the data folder</Trans>
        </p>
      )}
    </div>
  );
}
