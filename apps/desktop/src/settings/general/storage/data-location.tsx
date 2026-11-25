import { Trans, useLingui } from "@lingui/react/macro";
import { useQuery } from "@tanstack/react-query";
import { homeDir } from "@tauri-apps/api/path";
import type { ReactNode } from "react";

import { commands as openerCommands } from "@anlg/plugin-opener2";
import { commands as settingsCommands } from "@anlg/plugin-settings";
import { FolderSimple } from "@anlg/ui/components/icons";
import { Button } from "@anlg/ui/components/ui/button";

import { displayPath } from "./path-utils";

import { commands as appCommands } from "~/types/tauri.gen";

function FolderLocationRow({
  title,
  path,
  home,
  disabled,
  onOpen,
}: {
  title: ReactNode;
  path: string | undefined;
  home: string | undefined;
  disabled: boolean;
  onOpen: () => void;
}) {
  return (
    <div className="grid grid-cols-[minmax(0,1fr)_9rem] items-center gap-3">
      <button
        type="button"
        className="hover:bg-muted/40 flex min-w-0 items-center gap-2 rounded-lg px-2 py-2 text-left transition-colors"
        disabled={disabled}
        onClick={onOpen}
      >
        <FolderSimple className="text-muted-foreground size-4 shrink-0" />
        <div className="min-w-0">
          <p className="text-sm font-medium">{title}</p>
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
        onClick={onOpen}
      >
        <Trans>Open folder</Trans>
      </Button>
    </div>
  );
}

export function DataLocationRow() {
  const { t } = useLingui();
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
  const databaseDir = useQuery({
    queryKey: ["database-dir"],
    queryFn: async () => {
      const result = await appCommands.getDatabaseDir();
      if (result.status === "error") {
        throw new Error(result.error);
      }
      return result.data;
    },
  });
  const path = dataDir.data;
  const databasePath = databaseDir.data;
  const disabled = dataDir.isLoading || !!dataDir.error || !path;
  const showSeparateDatabase = Boolean(
    path && databasePath && databasePath !== path,
  );
  const openFolder = (target: string | undefined) => {
    if (target) void openerCommands.openPath(target, null);
  };
  const loadError = dataDir.error
    ? t`Could not load the data folder`
    : databaseDir.error
      ? t`Could not load the notes database folder`
      : null;

  return (
    <div className="flex flex-col gap-3">
      <FolderLocationRow
        title={<Trans>Data location</Trans>}
        path={path}
        home={home}
        disabled={disabled}
        onOpen={() => openFolder(path)}
      />
      {showSeparateDatabase && (
        <FolderLocationRow
          title={<Trans>Notes database</Trans>}
          path={databasePath}
          home={home}
          disabled={!databasePath}
          onOpen={() => openFolder(databasePath)}
        />
      )}
      {loadError && (
        <p role="alert" className="px-2 text-xs text-red-500">
          {loadError}
        </p>
      )}
    </div>
  );
}
