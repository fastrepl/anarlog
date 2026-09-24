import { Trans, useLingui } from "@lingui/react/macro";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useCallback, useRef, useState } from "react";

import { DotsThree, File, Plus, X } from "@anlg/ui/components/icons";
import { Button } from "@anlg/ui/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@anlg/ui/components/ui/dialog";
import {
  AppFloatingPanel,
  appFloatingMenuPanelClassName,
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@anlg/ui/components/ui/dropdown-menu";
import { Input } from "@anlg/ui/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@anlg/ui/components/ui/select";
import { cn } from "@anlg/utils";

import { useFolderSelection } from "./selection";

import { useOptionalAuth } from "~/auth";
import { ResourceShareButton, sharedFolderPayload } from "~/resource-sharing";
import {
  deleteSharedResource,
  moveSharedResource,
  requireResourceSharingContext,
} from "~/resource-sharing/client";
import {
  sharedResourcesQueryKey,
  useSharedResources,
} from "~/resource-sharing/hooks";
import {
  useAvailableShareWorkspaces,
  usePersonalWorkspaceId,
} from "~/session-sharing/source";
import {
  deleteLocalFolderMaterial,
  diskAttachmentId,
  useFolderMaterials,
} from "~/session/folder-attachments";
import {
  deleteNamedFolder,
  renameNamedFolder,
  updateFolderIcon,
  updateFolderWorkspace,
  useFolderWorkspaceId,
} from "~/session/folder-catalog";
import { resolvedFolderIcon } from "~/session/folder-icon";
import { FolderInstructionsField } from "~/session/folder-instructions";
import { folderDisplayName, normalizeFolderPath } from "~/session/folders";
import { useFolderIcons } from "~/session/queries";
import { useFolderMaterialUpload } from "~/shared/hooks/useFileUpload";
import { DestructiveConfirmationDialog } from "~/shared/ui/destructive-confirmation-dialog";
import { TemplateIconPicker } from "~/templates/template-icon-picker";

const PERSONAL_WORKSPACE_VALUE = "__personal__";

export function FolderEditor({ folderPath }: { folderPath: string }) {
  const { t } = useLingui();
  const auth = useOptionalAuth();
  const accountUserId = auth?.session?.user.id ?? null;
  const availableWorkspaces = useAvailableShareWorkspaces(accountUserId);
  const personalWorkspaceId = usePersonalWorkspaceId(accountUserId);
  const queryClient = useQueryClient();
  const sharedFolders = useSharedResources("folder");
  const ownedShare = sharedFolders.data?.find(
    (resource) =>
      resource.accessKind === "owner" && resource.sourceId === folderPath,
  );
  const setSelectedPath = useFolderSelection((state) => state.setSelectedPath);
  const markFolderDeleted = useFolderSelection(
    (state) => state.markFolderDeleted,
  );
  const persistedIcons = useFolderIcons();
  const iconOverrides = useFolderSelection((state) => state.iconOverrides);
  const setIconOverride = useFolderSelection((state) => state.setIconOverride);
  const clearIconOverride = useFolderSelection(
    (state) => state.clearIconOverride,
  );
  const rekeyIconOverride = useFolderSelection(
    (state) => state.rekeyIconOverride,
  );
  const icon = resolvedFolderIcon(folderPath, persistedIcons, iconOverrides);
  const materials = useFolderMaterials(folderPath);
  const upload = useFolderMaterialUpload(folderPath);
  const inputRef = useRef<HTMLInputElement>(null);
  const skipTitleCommit = useRef(false);
  const [busy, setBusy] = useState(false);
  const [actionsOpen, setActionsOpen] = useState(false);
  const [deleting, setDeleting] = useState(false);
  const [workspaceConfirmation, setWorkspaceConfirmation] = useState<{
    id: string;
    name: string;
  } | null>(null);
  const folderWorkspaceId = useFolderWorkspaceId(folderPath);
  const selectedWorkspaceValue = availableWorkspaces.some(
    (workspace) => workspace.id === folderWorkspaceId,
  )
    ? folderWorkspaceId
    : PERSONAL_WORKSPACE_VALUE;
  const workspaceMutation = useMutation({
    mutationFn: (workspaceId: string) =>
      updateFolderWorkspace(folderPath, workspaceId),
    onSuccess: () => setWorkspaceConfirmation(null),
  });
  const displayName = folderDisplayName(folderPath);
  const [draft, setDraft] = useState(displayName);

  const commitTitle = useCallback(async () => {
    if (skipTitleCommit.current) {
      skipTitleCommit.current = false;
      setDraft(displayName);
      return;
    }

    const normalizedName = normalizeFolderPath(draft.trim());
    if (!normalizedName || normalizedName.includes("/")) {
      setDraft(displayName);
      return;
    }

    const separatorIndex = folderPath.lastIndexOf("/");
    const parentPath =
      separatorIndex === -1 ? "" : folderPath.slice(0, separatorIndex);
    const renamedPath = parentPath
      ? `${parentPath}/${normalizedName}`
      : normalizedName;
    if (renamedPath === folderPath) {
      setDraft(displayName);
      return;
    }

    setBusy(true);
    try {
      const renamed = await renameNamedFolder(folderPath, renamedPath);
      if (ownedShare && auth) {
        try {
          await moveSharedResource(requireResourceSharingContext(auth), {
            shareId: ownedShare.shareId,
            sourceId: renamed,
            title: folderDisplayName(renamed),
            payload: await sharedFolderPayload(renamed),
          });
        } catch (error) {
          await renameNamedFolder(renamed, folderPath);
          throw error;
        }
        void queryClient.invalidateQueries({
          queryKey: sharedResourcesQueryKey(auth.session?.user.id, "folder"),
        });
      }
      rekeyIconOverride(folderPath, renamed);
      setSelectedPath(renamed);
    } catch {
      setDraft(displayName);
    } finally {
      setBusy(false);
    }
  }, [
    auth,
    displayName,
    draft,
    folderPath,
    ownedShare,
    queryClient,
    rekeyIconOverride,
    setSelectedPath,
  ]);

  return (
    <section className="flex h-full flex-1 flex-col" aria-label={folderPath}>
      <div className="flex h-12 items-center justify-between gap-3 pr-1 pl-3">
        <div className="flex min-w-0 flex-1 items-center gap-2">
          <TemplateIconPicker
            size="sm"
            label={t`Choose folder icon`}
            value={icon}
            onChange={(nextIcon) => {
              setIconOverride(folderPath, nextIcon);
              void updateFolderIcon(folderPath, nextIcon).catch((error) => {
                clearIconOverride(folderPath, nextIcon);
                console.error("[folder-editor] failed to update icon", error);
              });
            }}
          />
          <div className="relative max-w-full min-w-0">
            <span
              aria-hidden="true"
              className="invisible block px-0 py-0 text-sm font-semibold whitespace-pre"
            >
              {(draft || t`Folder name`) + " "}
            </span>
            <Input
              value={draft}
              disabled={busy}
              aria-label={t`Folder name`}
              onChange={(event) => setDraft(event.target.value)}
              onBlur={() => {
                void commitTitle();
              }}
              onKeyDown={(event) => {
                if (event.key === "Enter") {
                  event.currentTarget.blur();
                }
                if (event.key === "Escape") {
                  skipTitleCommit.current = true;
                  setDraft(displayName);
                  event.currentTarget.blur();
                }
              }}
              placeholder={t`Folder name`}
              className="absolute inset-0 h-auto w-full max-w-full min-w-0 border-0 px-0 py-0 text-sm font-semibold shadow-none focus-visible:ring-0 md:text-sm"
            />
          </div>
        </div>
        <div className="flex items-center gap-0.5">
          <ResourceShareButton
            resourceType="folder"
            sourceId={folderPath}
            title={displayName}
            buildPayload={() => sharedFolderPayload(folderPath)}
          />
          <DropdownMenu open={actionsOpen} onOpenChange={setActionsOpen}>
            <DropdownMenuTrigger asChild>
              <Button
                type="button"
                size="icon"
                variant="ghost"
                disabled={busy}
                className={cn([
                  "text-muted-foreground hover:text-foreground",
                  actionsOpen && "bg-muted text-foreground hover:bg-accent",
                ])}
                aria-label={t`Folder actions`}
              >
                <DotsThree className="size-4" />
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent variant="app" align="end">
              <AppFloatingPanel className={appFloatingMenuPanelClassName}>
                <DropdownMenuItem
                  disabled={busy}
                  onClick={() => setDeleting(true)}
                  className="cursor-pointer text-red-600 focus:text-red-600"
                >
                  <Trans>Delete</Trans>
                </DropdownMenuItem>
              </AppFloatingPanel>
            </DropdownMenuContent>
          </DropdownMenu>
        </div>
      </div>

      <div className="scrollbar-hide flex-1 overflow-y-auto px-3 pt-3 pb-6">
        <div className="flex max-w-2xl flex-col gap-6">
          {auth?.session?.user.id && availableWorkspaces.length > 0 ? (
            <div className="flex items-start justify-between gap-4">
              <div className="flex flex-col gap-1.5">
                <h4 className="text-sm font-medium">
                  <Trans>Team folder</Trans>
                </h4>
                <p className="text-muted-foreground text-xs">
                  <Trans>
                    Choose who receives new notes created in this folder.
                  </Trans>
                </p>
              </div>
              <Select
                value={selectedWorkspaceValue}
                disabled={workspaceMutation.isPending}
                onValueChange={(workspaceId) => {
                  if (workspaceId === PERSONAL_WORKSPACE_VALUE) {
                    workspaceMutation.mutate(personalWorkspaceId);
                    return;
                  }
                  const workspace = availableWorkspaces.find(
                    (candidate) => candidate.id === workspaceId,
                  );
                  if (workspace) {
                    setWorkspaceConfirmation(workspace);
                  }
                }}
              >
                <SelectTrigger
                  aria-label={t`Team folder workspace`}
                  className="h-8 w-44 text-xs"
                >
                  <SelectValue placeholder={t`Only me`} />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value={PERSONAL_WORKSPACE_VALUE}>
                    <Trans>Only me</Trans>
                  </SelectItem>
                  {availableWorkspaces.map((workspace) => (
                    <SelectItem key={workspace.id} value={workspace.id}>
                      {workspace.name}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
          ) : null}

          <div className="flex flex-col gap-1.5">
            <h4 className="text-sm font-medium">
              <Trans>Context</Trans>
            </h4>
            <p className="text-muted-foreground text-xs">
              <Trans>What these notes are usually about</Trans>
            </p>
            <FolderInstructionsField folderPath={folderPath} rows={4} />
          </div>

          <div className="flex flex-col gap-2">
            <h4 className="text-sm font-medium">
              <Trans>Materials</Trans>
            </h4>
            <input
              ref={inputRef}
              type="file"
              className="hidden"
              onChange={async (event) => {
                const file = event.target.files?.[0];
                event.target.value = "";
                if (!file) {
                  return;
                }
                setBusy(true);
                try {
                  await upload(file);
                } finally {
                  setBusy(false);
                }
              }}
            />
            <ul className="grid grid-cols-2 gap-2 sm:grid-cols-3">
              <li>
                <button
                  type="button"
                  disabled={busy}
                  className={cn([
                    "border-border text-muted-foreground hover:bg-accent hover:text-foreground",
                    "flex aspect-[4/3] w-full flex-col items-center justify-center gap-1.5 rounded-lg border border-dashed",
                    "focus-visible:ring-ring focus-visible:ring-2 focus-visible:outline-hidden",
                    "disabled:opacity-50",
                  ])}
                  onClick={() => inputRef.current?.click()}
                >
                  <Plus className="size-6" />
                  <span className="text-xs font-medium">
                    <Trans>Add file</Trans>
                  </span>
                </button>
              </li>
              {materials.map((material) => (
                <li key={material.id}>
                  <div className="border-border relative flex aspect-[4/3] flex-col items-center justify-center gap-1.5 rounded-lg border px-3 py-2">
                    <File
                      className="text-muted-foreground size-6 shrink-0"
                      aria-hidden
                    />
                    <span
                      className="w-full truncate text-center text-xs"
                      title={material.filename}
                    >
                      {material.filename}
                    </span>
                    <button
                      type="button"
                      aria-label={t`Remove ${material.filename}`}
                      disabled={busy}
                      className={cn([
                        "text-muted-foreground hover:bg-accent hover:text-foreground",
                        "absolute top-1.5 right-1.5 flex size-6 items-center justify-center rounded-full",
                        "focus-visible:ring-ring focus-visible:ring-2 focus-visible:outline-hidden",
                        "disabled:opacity-50",
                      ])}
                      onClick={async () => {
                        setBusy(true);
                        try {
                          await deleteLocalFolderMaterial({
                            folderPath,
                            attachmentId: diskAttachmentId(
                              material.relativePath,
                            ),
                          });
                        } finally {
                          setBusy(false);
                        }
                      }}
                    >
                      <X size={12} />
                    </button>
                  </div>
                </li>
              ))}
            </ul>
          </div>
        </div>
      </div>

      <Dialog
        open={workspaceConfirmation !== null}
        onOpenChange={(open) => {
          if (!open && !workspaceMutation.isPending) {
            setWorkspaceConfirmation(null);
          }
        }}
      >
        <DialogContent>
          <DialogHeader>
            <DialogTitle>
              <Trans>Share this folder?</Trans>
            </DialogTitle>
            <DialogDescription>
              <Trans>
                New notes in this folder will sync to everyone in{" "}
                {workspaceConfirmation?.name}. Notes already in the folder are
                not moved.
              </Trans>
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button
              variant="outline"
              disabled={workspaceMutation.isPending}
              onClick={() => setWorkspaceConfirmation(null)}
            >
              <Trans>Cancel</Trans>
            </Button>
            <Button
              disabled={workspaceMutation.isPending}
              onClick={() => {
                if (workspaceConfirmation) {
                  workspaceMutation.mutate(workspaceConfirmation.id);
                }
              }}
            >
              <Trans>Confirm</Trans>
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <DestructiveConfirmationDialog
        open={deleting}
        onOpenChange={setDeleting}
        title={<Trans>Delete folder</Trans>}
        description={
          <Trans id="Notes stay in All notes. Materials in this folder will be deleted.">
            Notes stay in All notes. This folder, its nested folders, and all
            their materials will be deleted.
          </Trans>
        }
        confirmLabel={<Trans>Delete folder</Trans>}
        isPending={busy}
        onConfirm={() => {
          void (async () => {
            setBusy(true);
            try {
              if (ownedShare && auth) {
                await deleteSharedResource(
                  requireResourceSharingContext(auth),
                  ownedShare.shareId,
                );
              }
              await deleteNamedFolder(folderPath);
              setDeleting(false);
              markFolderDeleted(folderPath);
              if (ownedShare) {
                void queryClient.invalidateQueries({
                  queryKey: sharedResourcesQueryKey(
                    auth?.session?.user.id,
                    "folder",
                  ),
                });
              }
            } finally {
              setBusy(false);
            }
          })();
        }}
      />
    </section>
  );
}
