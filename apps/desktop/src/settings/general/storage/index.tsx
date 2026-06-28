import { Trans, useLingui } from "@lingui/react/macro";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { homeDir } from "@tauri-apps/api/path";
import { FolderIcon, type LucideIcon, Settings2Icon } from "lucide-react";
import { type ReactNode } from "react";
import { useState } from "react";

import { commands as openerCommands } from "@hypr/plugin-opener2";
import { commands as settingsCommands } from "@hypr/plugin-settings";
import { Button } from "@hypr/ui/components/ui/button";
import { Checkbox } from "@hypr/ui/components/ui/checkbox";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@hypr/ui/components/ui/dialog";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@hypr/ui/components/ui/select";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@hypr/ui/components/ui/tooltip";
import { cn } from "@hypr/utils";

import { displayPath } from "./path-utils";
import { useChangeContentPathWizard } from "./use-storage-wizard";

import { useConfigValue } from "~/shared/config";
import * as settings from "~/store/tinybase/store/settings";

const AUDIO_RETENTION_OPTIONS = [
  {
    value: "none",
    label: "Don't save",
    description: "Do not keep recordings after processing",
  },
  {
    value: "oneDay",
    label: "1 day",
    description: "Expire recordings after one day",
  },
  {
    value: "threeDays",
    label: "3 days",
    description: "Expire recordings after three days",
  },
  {
    value: "oneWeek",
    label: "1 week",
    description: "Expire recordings after one week",
  },
  {
    value: "oneMonth",
    label: "1 month",
    description: "Expire recordings after one month",
  },
  {
    value: "forever",
    label: "Forever",
    description: "Keep recordings until manually deleted",
  },
];

export function StorageSettingsView() {
  const { t } = useLingui();
  const queryClient = useQueryClient();
  const { data: home } = useQuery({ queryKey: ["home-dir"], queryFn: homeDir });
  const { data: othersBase } = useQuery({
    queryKey: ["others-base-path"],
    queryFn: async () => {
      const result = await settingsCommands.globalBase();
      if (result.status === "error") {
        throw new Error(result.error);
      }
      return result.data;
    },
  });

  const { data: contentBase } = useQuery({
    queryKey: ["content-base-path"],
    queryFn: async () => {
      const result = await settingsCommands.vaultBase();
      if (result.status === "error") {
        throw new Error(result.error);
      }
      return result.data;
    },
  });
  const [showDialog, setShowDialog] = useState(false);

  return (
    <div>
      <h2 className="mb-4 font-sans text-lg font-semibold">
        <Trans>Storage</Trans>
      </h2>
      <div className="flex flex-col gap-3">
        <AudioRetentionRow />
        <StoragePathRow
          icon={FolderIcon}
          title={t`Content`}
          description={t`Stores your notes, recordings, and session data`}
          path={contentBase}
          home={home}
          action={
            <Button
              variant="outline"
              size="sm"
              onClick={() => setShowDialog(true)}
              disabled={!contentBase}
            >
              <Trans>Customize</Trans>
            </Button>
          }
        />
        <StoragePathRow
          icon={Settings2Icon}
          title={t`Others`}
          description={t`Stores app-wide settings and configurations`}
          path={othersBase}
          home={home}
        />
      </div>
      <ChangeContentPathDialog
        open={showDialog}
        currentPath={contentBase}
        home={home}
        onOpenChange={setShowDialog}
        onSuccess={() => {
          void queryClient.invalidateQueries({
            queryKey: ["content-base-path"],
          });
        }}
      />
    </div>
  );
}

function AudioRetentionRow() {
  const { t } = useLingui();
  const audioRetention = useConfigValue("audio_retention") || "forever";
  const setAudioRetention = settings.UI.useSetPartialValuesCallback(
    (value: string) => ({
      audio_retention: value,
      save_recordings: value !== "none",
    }),
    [],
    settings.STORE_ID,
  );
  const selectedOption =
    AUDIO_RETENTION_OPTIONS.find((option) => option.value === audioRetention) ??
    AUDIO_RETENTION_OPTIONS[AUDIO_RETENTION_OPTIONS.length - 1]!;
  const copyByValue = {
    none: {
      label: t`Don't save`,
      description: t`Do not keep recordings after processing`,
    },
    oneDay: {
      label: t`1 day`,
      description: t`Expire recordings after one day`,
    },
    threeDays: {
      label: t`3 days`,
      description: t`Expire recordings after three days`,
    },
    oneWeek: {
      label: t`1 week`,
      description: t`Expire recordings after one week`,
    },
    oneMonth: {
      label: t`1 month`,
      description: t`Expire recordings after one month`,
    },
    forever: {
      label: t`Forever`,
      description: t`Keep recordings until manually deleted`,
    },
  } as const;

  return (
    <div className="flex items-center gap-3">
      <div className="flex w-24 shrink-0 cursor-default items-center gap-2">
        <Settings2Icon className="text-muted-foreground size-4" />
        <span className="text-sm font-medium">
          <Trans>Audio</Trans>
        </span>
      </div>
      <div className="min-w-0 flex-1">
        <p className="text-muted-foreground truncate text-sm">
          <Trans>Save audio after meeting</Trans>
        </p>
        <p className="text-muted-foreground text-xs">
          {copyByValue[selectedOption.value as keyof typeof copyByValue]
            ?.description ?? selectedOption.description}
        </p>
      </div>
      <Select value={audioRetention} onValueChange={setAudioRetention}>
        <SelectTrigger className="bg-card w-36 shadow-none focus:ring-0">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          {AUDIO_RETENTION_OPTIONS.map((option) => (
            <SelectItem key={option.value} value={option.value}>
              {copyByValue[option.value as keyof typeof copyByValue]?.label ??
                option.label}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
    </div>
  );
}

function ChangeContentPathDialog({
  open,
  currentPath,
  home,
  onOpenChange,
  onSuccess,
}: {
  open: boolean;
  currentPath: string | undefined;
  home: string | undefined;
  onOpenChange: (open: boolean) => void;
  onSuccess: () => void;
}) {
  const { t } = useLingui();
  const {
    selectedPath,
    selectPath,
    moveVault,
    setMoveVault,
    chooseFolder,
    apply,
    isPending,
    error,
  } = useChangeContentPathWizard({ open, currentPath, onSuccess });

  const { data: obsidianVaults } = useQuery({
    queryKey: ["obsidian-vaults"],
    queryFn: async () => {
      const result = await settingsCommands.obsidianVaults();
      if (result.status === "error") return [];
      return result.data;
    },
  });

  const isNewPathChosen = !!selectedPath && selectedPath !== currentPath;

  const { data: isNewPathEmpty, isLoading: isCheckingNewPath } = useQuery({
    queryKey: ["path-empty-check", selectedPath],
    enabled: isNewPathChosen,
    queryFn: async () => {
      const result = await settingsCommands.isEmptyOrMissingDir(selectedPath!);
      if (result.status === "error") return true;
      return result.data;
    },
  });

  const disabledReason = (() => {
    if (!selectedPath || selectedPath === currentPath)
      return t`Select a different folder`;
    if (isCheckingNewPath) return t`Checking folder...`;
    if (moveVault && isNewPathEmpty === false) {
      return t`Moving existing data requires an empty folder. Uncheck Move to switch locations without migrating files.`;
    }
    return null;
  })();
  const showMoveToggle =
    !!selectedPath && selectedPath !== currentPath && !isCheckingNewPath;

  return (
    <Dialog
      open={open}
      onOpenChange={(nextOpen) => {
        if (isPending) return;
        onOpenChange(nextOpen);
      }}
    >
      <DialogContent>
        <DialogHeader>
          <DialogTitle>
            <Trans>Change content location</Trans>
          </DialogTitle>
          <DialogDescription>
            <Trans>
              Choose where Anarlog should store data. (notes, settings, etc)
            </Trans>
          </DialogDescription>
        </DialogHeader>

        <div className="mb-4 flex flex-col">
          <div>
            <div
              className={cn([
                "flex items-center gap-3 rounded-lg border px-3 py-2",
                isNewPathChosen && isNewPathEmpty === false
                  ? "bg-muted border-yellow-400"
                  : "border-foreground",
              ])}
            >
              <div className="min-w-0 flex-1">
                <p className="text-muted-foreground text-sm">
                  {selectedPath
                    ? displayPath(selectedPath, home)
                    : displayPath(currentPath, home)}
                </p>
                {isNewPathChosen && isNewPathEmpty === false && (
                  <p className="mt-1 text-xs text-yellow-600">
                    <Trans>
                      Folder is not empty. Uncheck Move to use it as-is, or pick
                      a dedicated empty folder (for example "meetings") for a
                      full migration.
                    </Trans>
                  </p>
                )}
              </div>
              <Button
                variant={isNewPathChosen ? "outline" : "default"}
                size="sm"
                className="shrink-0"
                onClick={() => chooseFolder()}
              >
                <Trans>Browse</Trans>
              </Button>
            </div>
            {obsidianVaults && obsidianVaults.length > 0 && (
              <div className="mt-2 flex flex-col gap-1.5">
                <span className="mt-1 text-xs">
                  <Trans>Want to use with your vault?</Trans>
                </span>
                {obsidianVaults.map((vault) => (
                  <button
                    key={vault.path}
                    onClick={() => selectPath(vault.path)}
                    className="border-border bg-muted text-muted-foreground hover:bg-accent flex cursor-pointer items-center gap-2 rounded-full border px-3 py-2 text-left text-sm transition-colors"
                  >
                    <img
                      src="/assets/obsidian-icon.svg"
                      className="size-4 shrink-0"
                      aria-hidden="true"
                    />
                    <span className="min-w-0 flex-1 truncate">
                      {displayPath(vault.path, home)}
                    </span>
                  </button>
                ))}
              </div>
            )}
          </div>
        </div>

        {error && <p className="text-sm text-red-500">{error.message}</p>}

        {isNewPathChosen && (
          <DialogFooter className="items-center">
            {showMoveToggle && (
              <label className="mr-auto flex cursor-pointer items-center gap-2">
                <Checkbox
                  checked={moveVault}
                  onCheckedChange={(v) => setMoveVault(v === true)}
                />
                <div className="flex flex-row gap-1">
                  <span className="text-muted-foreground text-sm font-semibold">
                    <Trans>Move</Trans>
                  </span>
                  <span className="text-muted-foreground text-sm">
                    <Trans>existing data to new location</Trans>
                  </span>
                </div>
              </label>
            )}
            <Tooltip>
              <TooltipTrigger asChild>
                <span
                  className={cn([
                    disabledReason ? "cursor-not-allowed" : "cursor-pointer",
                  ])}
                >
                  <Button
                    onClick={apply}
                    disabled={!!disabledReason || isPending}
                    className={cn([
                      disabledReason ? "pointer-events-none" : "",
                    ])}
                  >
                    {isPending ? t`Applying...` : t`Apply and Restart`}
                  </Button>
                </span>
              </TooltipTrigger>
              {disabledReason && (
                <TooltipContent>
                  <p className="text-xs">{disabledReason}</p>
                </TooltipContent>
              )}
            </Tooltip>
          </DialogFooter>
        )}
      </DialogContent>
    </Dialog>
  );
}

function StoragePathRow({
  icon: Icon,
  title,
  description,
  path,
  home,
  action,
}: {
  icon: LucideIcon;
  title: ReactNode;
  description: ReactNode;
  path: string | undefined;
  home: string | undefined;
  action?: ReactNode;
}) {
  return (
    <div className="flex items-center gap-3">
      <Tooltip delayDuration={0}>
        <TooltipTrigger asChild>
          <div className="flex w-24 shrink-0 cursor-default items-center gap-2">
            <Icon className="text-muted-foreground size-4" />
            <span className="text-sm font-medium">{title}</span>
          </div>
        </TooltipTrigger>
        <TooltipContent side="top">
          <p className="text-xs">{description}</p>
        </TooltipContent>
      </Tooltip>
      <button
        onClick={() => path && openerCommands.openPath(path, null)}
        className="text-muted-foreground min-w-0 flex-1 cursor-pointer truncate text-left text-sm hover:underline"
      >
        {displayPath(path, home)}
      </button>
      {action && <div className="shrink-0">{action}</div>}
    </div>
  );
}
