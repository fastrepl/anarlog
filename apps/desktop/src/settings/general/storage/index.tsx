import { useQuery, useQueryClient } from "@tanstack/react-query";
import { homeDir } from "@tauri-apps/api/path";
import { FolderIcon, type LucideIcon, Settings2Icon } from "lucide-react";
import { type ReactNode } from "react";
import { useState } from "react";

import { commands as fsSyncCommands } from "@hypr/plugin-fs-sync";
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
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@hypr/ui/components/ui/tooltip";
import { cn } from "@hypr/utils";

import { displayPath } from "./path-utils";
import { useChangeContentPathWizard } from "./use-storage-wizard";

export function StorageSettingsView() {
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
      <h2 className="mb-4 font-serif text-lg font-semibold">Storage</h2>
      <div className="flex flex-col gap-3">
        <StoragePathRow
          icon={FolderIcon}
          title="Content"
          description="Stores your notes, recordings, and session data"
          path={contentBase}
          home={home}
          action={
            <Button
              variant="outline"
              size="sm"
              onClick={() => setShowDialog(true)}
              disabled={!contentBase}
            >
              Customize
            </Button>
          }
        />
        <StoragePathRow
          icon={Settings2Icon}
          title="Others"
          description="Stores app-wide settings and configurations"
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
      const result = await fsSyncCommands.scanAndRead(
        selectedPath!,
        ["*"],
        false,
        null,
      );
      if (result.status === "error") return true; // dir doesn't exist yet → trivially empty, Rust will create it
      return (
        Object.keys(result.data.files).length === 0 &&
        result.data.dirs.length === 0
      );
    },
  });

  const disabledReason = (() => {
    if (!selectedPath || selectedPath === currentPath)
      return "Select a different folder";
    if (isCheckingNewPath) return "Checking folder...";
    if (moveVault && isNewPathEmpty === false) {
      return "Moving existing data requires an empty folder. Uncheck Move to switch locations without migrating files.";
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
          <DialogTitle>Change content location</DialogTitle>
          <DialogDescription>
            Choose where Char should store data. (notes, settings, etc)
          </DialogDescription>
        </DialogHeader>

        <div className="mb-4 flex flex-col">
          <div>
            <div
              className={cn([
                "flex items-center gap-3 rounded-lg border px-3 py-2",
                isNewPathChosen && isNewPathEmpty === false
                  ? "border-yellow-400 bg-neutral-50"
                  : "border-neutral-900",
              ])}
            >
              <div className="min-w-0 flex-1">
                <p className="text-sm text-neutral-700">
                  {selectedPath
                    ? displayPath(selectedPath, home)
                    : displayPath(currentPath, home)}
                </p>
                {isNewPathChosen && isNewPathEmpty === false && (
                  <p className="mt-1 text-xs text-yellow-600">
                    Folder is not empty. Uncheck Move to use it as-is, or pick a
                    dedicated empty folder (for example "meetings") for a full
                    migration.
                  </p>
                )}
              </div>
              <Button
                variant={isNewPathChosen ? "outline" : "default"}
                size="sm"
                className="shrink-0"
                onClick={() => chooseFolder()}
              >
                Browse
              </Button>
            </div>
            {obsidianVaults && obsidianVaults.length > 0 && (
              <div className="mt-2 flex flex-col gap-1.5">
                <span className="mt-1 text-xs">
                  Want to use with your vault?
                </span>
                {obsidianVaults.map((vault) => (
                  <button
                    key={vault.path}
                    onClick={() => selectPath(vault.path)}
                    className="flex cursor-pointer items-center gap-2 rounded-lg border border-neutral-200 bg-neutral-50 px-3 py-2 text-left text-sm text-neutral-500 transition-colors hover:bg-neutral-100"
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
                  <span className="text-sm font-semibold text-neutral-600">
                    Move
                  </span>
                  <span className="text-sm text-neutral-600">
                    existing data to new location
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
                    {isPending ? "Applying..." : "Apply and Restart"}
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
  title: string;
  description: string;
  path: string | undefined;
  home: string | undefined;
  action?: ReactNode;
}) {
  return (
    <div className="flex items-center gap-3">
      <Tooltip delayDuration={0}>
        <TooltipTrigger asChild>
          <div className="flex w-24 shrink-0 cursor-default items-center gap-2">
            <Icon className="size-4 text-neutral-500" />
            <span className="text-sm font-medium">{title}</span>
          </div>
        </TooltipTrigger>
        <TooltipContent side="top">
          <p className="text-xs">{description}</p>
        </TooltipContent>
      </Tooltip>
      <button
        onClick={() => path && openerCommands.openPath(path, null)}
        className="min-w-0 flex-1 cursor-pointer truncate text-left text-sm text-neutral-500 hover:underline"
      >
        {displayPath(path, home)}
      </button>
      {action && <div className="shrink-0">{action}</div>}
    </div>
  );
}
