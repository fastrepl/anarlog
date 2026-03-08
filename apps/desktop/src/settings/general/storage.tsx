import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { open as selectFolder } from "@tauri-apps/plugin-dialog";
import { FolderIcon, type LucideIcon, Settings2Icon } from "lucide-react";
import { type ReactNode, useEffect, useState } from "react";

import { commands as openerCommands } from "@hypr/plugin-opener2";
import { commands as settingsCommands } from "@hypr/plugin-settings";
import { Button } from "@hypr/ui/components/ui/button";
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

import { relaunch } from "~/store/tinybase/store/save";

export function StorageSettingsView() {
  const queryClient = useQueryClient();
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
          action={
            <Button
              variant="outline"
              size="sm"
              onClick={() => setShowDialog(true)}
              disabled={true}
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
        />
      </div>
      <ChangeContentPathDialog
        open={showDialog}
        currentPath={contentBase}
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
  onOpenChange,
  onSuccess,
}: {
  open: boolean;
  currentPath: string | undefined;
  onOpenChange: (open: boolean) => void;
  onSuccess: () => void;
}) {
  const [step, setStep] = useState<"path" | "copy">("path");
  const [selectedPath, setSelectedPath] = useState<string | null>(null);
  const [copyVault, setCopyVault] = useState(true);

  useEffect(() => {
    if (!open) return;
    setStep("path");
    setSelectedPath(currentPath ?? null);
    setCopyVault(true);
  }, [currentPath, open]);

  const applyMutation = useMutation({
    mutationFn: async ({
      newPath,
      shouldCopy,
    }: {
      newPath: string;
      shouldCopy: boolean;
    }) => {
      if (shouldCopy) {
        const copyResult = await settingsCommands.copyVault(newPath);
        if (copyResult.status === "error") {
          throw new Error(copyResult.error);
        }
      }

      const setResult = await settingsCommands.setVaultBase(newPath);
      if (setResult.status === "error") {
        throw new Error(setResult.error);
      }
    },
    onSuccess: async () => {
      onSuccess();
      await relaunch();
    },
  });

  const chooseFolder = async () => {
    const selected = await selectFolder({
      title: "Choose content location",
      directory: true,
      multiple: false,
      defaultPath: selectedPath ?? currentPath ?? undefined,
    });

    if (selected) {
      setSelectedPath(selected);
    }
  };

  const canContinue =
    !!selectedPath &&
    !!currentPath &&
    selectedPath !== currentPath &&
    !applyMutation.isPending;

  const apply = () => {
    if (!selectedPath || !currentPath || selectedPath === currentPath) {
      return;
    }

    applyMutation.mutate({
      newPath: selectedPath,
      shouldCopy: copyVault,
    });
  };

  return (
    <Dialog
      open={open}
      onOpenChange={(nextOpen) => {
        if (applyMutation.isPending) {
          return;
        }
        onOpenChange(nextOpen);
      }}
    >
      <DialogContent>
        {step === "path" ? (
          <>
            <DialogHeader>
              <DialogTitle>Change content location</DialogTitle>
              <DialogDescription>
                Choose where Char should store notes, recordings, and session
                data after restart.
              </DialogDescription>
            </DialogHeader>

            <div className="flex flex-col gap-4">
              <StoragePreview
                label="Current"
                path={currentPath ?? "Loading..."}
              />
              <StoragePreview
                label="New"
                path={selectedPath ?? "Select a folder"}
              />
              <div className="flex justify-end">
                <Button variant="outline" onClick={chooseFolder}>
                  Choose Folder
                </Button>
              </div>
            </div>

            <DialogFooter>
              <Button variant="outline" onClick={() => onOpenChange(false)}>
                Cancel
              </Button>
              <Button onClick={() => setStep("copy")} disabled={!canContinue}>
                Continue
              </Button>
            </DialogFooter>
          </>
        ) : (
          <>
            <DialogHeader>
              <DialogTitle>Move existing content?</DialogTitle>
              <DialogDescription>
                Choose whether Char should copy your current vault into the new
                location before restarting.
              </DialogDescription>
            </DialogHeader>

            <div className="flex flex-col gap-3">
              <CopyChoiceCard
                title="Copy existing content"
                description="Recommended. Notes, recordings, and session files will be copied to the new folder."
                selected={copyVault}
                onClick={() => setCopyVault(true)}
              />
              <CopyChoiceCard
                title="Start with an empty folder"
                description="Only the new location will be saved. Existing content stays where it is."
                selected={!copyVault}
                onClick={() => setCopyVault(false)}
              />
              <StoragePreview label="New location" path={selectedPath ?? ""} />
              {applyMutation.error ? (
                <p className="text-sm text-red-500">
                  {applyMutation.error.message}
                </p>
              ) : null}
            </div>

            <DialogFooter>
              <Button
                variant="outline"
                onClick={() => setStep("path")}
                disabled={applyMutation.isPending}
              >
                Back
              </Button>
              <Button onClick={apply} disabled={!canContinue}>
                {applyMutation.isPending ? "Applying..." : "Apply and Restart"}
              </Button>
            </DialogFooter>
          </>
        )}
      </DialogContent>
    </Dialog>
  );
}

function StoragePreview({ label, path }: { label: string; path: string }) {
  return (
    <div className="rounded-lg border border-neutral-200 bg-neutral-50 px-3 py-2">
      <p className="text-xs font-medium tracking-wide text-neutral-500 uppercase">
        {label}
      </p>
      <p className="mt-1 text-sm break-all text-neutral-700">{path}</p>
    </div>
  );
}

function CopyChoiceCard({
  title,
  description,
  selected,
  onClick,
}: {
  title: string;
  description: string;
  selected: boolean;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={cn([
        "rounded-lg border px-4 py-3 text-left transition-colors",
        selected
          ? "border-stone-400 bg-stone-50"
          : "border-neutral-200 bg-white hover:border-neutral-300",
      ])}
    >
      <p className="text-sm font-medium text-neutral-800">{title}</p>
      <p className="mt-1 text-sm text-neutral-500">{description}</p>
    </button>
  );
}

function StoragePathRow({
  icon: Icon,
  title,
  description,
  path,
  action,
}: {
  icon: LucideIcon;
  title: string;
  description: string;
  path: string | undefined;
  action?: ReactNode;
}) {
  const handleOpenPath = () => {
    if (path) {
      openerCommands.openPath(path, null);
    }
  };

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
        onClick={handleOpenPath}
        className="min-w-0 flex-1 cursor-pointer truncate text-left text-sm text-neutral-500 hover:underline"
      >
        {path ?? "Loading..."}
      </button>
      {action && <div className="shrink-0">{action}</div>}
    </div>
  );
}
