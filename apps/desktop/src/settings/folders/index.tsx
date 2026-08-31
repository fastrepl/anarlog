import { Trans, useLingui } from "@lingui/react/macro";
import { FolderSimplePlus, Plus, Trash, X } from "@phosphor-icons/react";
import { useRef, useState } from "react";

import { Button } from "@anlg/ui/components/ui/button";
import { cn } from "@anlg/utils";

import {
  deleteLocalFolderMaterial,
  diskAttachmentId,
  useFolderMaterials,
} from "~/session/folder-attachments";
import {
  createNamedFolder,
  deleteNamedFolder,
  renameNamedFolder,
} from "~/session/folder-catalog";
import { FolderInstructionsField } from "~/session/folder-instructions";
import { childFolderPath } from "~/session/folders";
import { useFolderPaths } from "~/session/queries";
import { SettingsPageTitle } from "~/settings/page-title";
import { useFolderMaterialUpload } from "~/shared/hooks/useFileUpload";
import { DestructiveConfirmationDialog } from "~/shared/ui/destructive-confirmation-dialog";
import { FolderNameDialog } from "~/sidebar/folder-name-dialog";

export function SettingsFolders() {
  const { t } = useLingui();
  const folders = useFolderPaths();
  const [selected, setSelected] = useState<string | null>(null);
  const [creating, setCreating] = useState(false);
  const activeFolder =
    selected !== null && folders.includes(selected)
      ? selected
      : (folders[0] ?? null);

  return (
    <div className="flex max-w-5xl flex-col gap-8">
      <div className="flex items-start justify-between gap-4">
        <div className="min-w-0">
          <SettingsPageTitle title={<Trans>Folders</Trans>} />
          <p className="text-muted-foreground mt-2 max-w-xl text-sm leading-5">
            <Trans>
              Chat follows these instructions for notes in the folder. Add a
              syllabus or other files as materials.
            </Trans>
          </p>
        </div>
        <Button
          type="button"
          size="sm"
          variant="outline"
          onClick={() => setCreating(true)}
        >
          <FolderSimplePlus className="size-3.5" />
          <Trans>New folder</Trans>
        </Button>
      </div>

      {folders.length === 0 ? (
        <p className="text-muted-foreground text-sm">
          <Trans>
            No folders yet. Create one to group notes and materials.
          </Trans>
        </p>
      ) : (
        <div className="flex flex-col gap-6 sm:flex-row sm:items-start">
          <ul className="border-border/60 flex min-w-0 flex-1 flex-col gap-0.5 rounded-2xl border p-1.5">
            {folders.map((folder) => {
              const depth = folder.split("/").length - 1;
              const isActive = folder === activeFolder;
              return (
                <li key={folder}>
                  <button
                    type="button"
                    aria-current={isActive ? "true" : undefined}
                    onClick={() => setSelected(folder)}
                    className={cn([
                      "w-full rounded-xl px-3 py-2 text-left text-sm",
                      "focus-visible:ring-ring focus-visible:ring-2 focus-visible:outline-hidden",
                      isActive
                        ? "bg-accent text-foreground font-medium"
                        : "text-muted-foreground hover:bg-accent/60 hover:text-foreground",
                    ])}
                    style={{ paddingLeft: `${12 + depth * 12}px` }}
                  >
                    <span className="min-w-0 truncate">{folder}</span>
                  </button>
                </li>
              );
            })}
          </ul>
          {activeFolder ? (
            <FolderSettingsCard
              folderPath={activeFolder}
              onFolderChanged={setSelected}
            />
          ) : null}
        </div>
      )}

      <FolderNameDialog
        open={creating}
        title={t`New folder`}
        confirmLabel={t`Create`}
        onOpenChange={setCreating}
        onSubmit={async (path) => {
          const created = await createNamedFolder(path);
          setSelected(created);
        }}
      />
    </div>
  );
}

function FolderSettingsCard({
  folderPath,
  onFolderChanged,
}: {
  folderPath: string;
  onFolderChanged: (folderPath: string | null) => void;
}) {
  const { t } = useLingui();
  const materials = useFolderMaterials(folderPath);
  const upload = useFolderMaterialUpload(folderPath);
  const inputRef = useRef<HTMLInputElement>(null);
  const [busy, setBusy] = useState(false);
  const [renaming, setRenaming] = useState(false);
  const [creatingChild, setCreatingChild] = useState(false);
  const [deleting, setDeleting] = useState(false);

  return (
    <section
      className="border-border/60 flex min-w-0 flex-1 flex-col gap-4 rounded-2xl border p-4"
      aria-label={folderPath}
    >
      <div className="flex items-start justify-between gap-3">
        <h3 className="min-w-0 truncate text-sm font-medium" title={folderPath}>
          {folderPath}
        </h3>
        <div className="flex shrink-0 items-center gap-1">
          <Button
            type="button"
            size="sm"
            variant="ghost"
            disabled={busy}
            onClick={() => setRenaming(true)}
          >
            <Trans>Rename</Trans>
          </Button>
          <Button
            type="button"
            size="sm"
            variant="ghost"
            disabled={busy}
            onClick={() => setCreatingChild(true)}
          >
            <Trans>New subfolder</Trans>
          </Button>
          <Button
            type="button"
            size="sm"
            variant="ghost"
            disabled={busy}
            className="text-destructive hover:text-destructive"
            onClick={() => setDeleting(true)}
          >
            <Trash className="size-3.5" />
            <Trans>Delete</Trans>
          </Button>
        </div>
      </div>

      <div className="flex flex-col gap-1.5">
        <h4 className="text-sm font-medium">
          <Trans>Instructions</Trans>
        </h4>
        <p className="text-muted-foreground text-xs">
          <Trans>
            Tell chat how to use notes and materials in this folder.
          </Trans>
        </p>
        <FolderInstructionsField folderPath={folderPath} rows={4} />
      </div>

      <div className="flex flex-col gap-1.5">
        <div className="flex items-center gap-1">
          <h4 className="min-w-0 flex-1 text-sm font-medium">
            <Trans>Materials</Trans>
          </h4>
          <button
            type="button"
            aria-label={t`Add material`}
            disabled={busy}
            className={cn([
              "text-muted-foreground hover:bg-accent hover:text-foreground flex size-7 items-center justify-center rounded-full",
              "focus-visible:ring-ring focus-visible:ring-2 focus-visible:outline-hidden",
              "disabled:opacity-50",
            ])}
            onClick={() => inputRef.current?.click()}
          >
            <Plus size={14} />
          </button>
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
        </div>
        {materials.length === 0 ? (
          <p className="text-muted-foreground text-xs">
            <Trans>Add a syllabus or PDF for this folder</Trans>
          </p>
        ) : (
          <ul className="flex flex-col gap-1">
            {materials.map((material) => (
              <li
                key={material.id}
                className="flex min-w-0 items-center gap-1 text-sm"
              >
                <span
                  className="min-w-0 flex-1 truncate"
                  title={material.filename}
                >
                  {material.filename}
                </span>
                <button
                  type="button"
                  aria-label={t`Remove ${material.filename}`}
                  disabled={busy}
                  className={cn([
                    "text-muted-foreground hover:bg-accent hover:text-foreground flex size-6 items-center justify-center rounded-full",
                    "focus-visible:ring-ring focus-visible:ring-2 focus-visible:outline-hidden",
                    "disabled:opacity-50",
                  ])}
                  onClick={async () => {
                    setBusy(true);
                    try {
                      await deleteLocalFolderMaterial({
                        folderPath,
                        attachmentId: diskAttachmentId(material.relativePath),
                      });
                    } finally {
                      setBusy(false);
                    }
                  }}
                >
                  <X size={12} />
                </button>
              </li>
            ))}
          </ul>
        )}
      </div>

      <FolderNameDialog
        open={renaming}
        title={t`Rename folder`}
        confirmLabel={t`Rename`}
        initialValue={folderPath}
        onOpenChange={setRenaming}
        onSubmit={async (nextPath) => {
          const renamed = await renameNamedFolder(folderPath, nextPath);
          onFolderChanged(renamed);
        }}
      />
      <FolderNameDialog
        open={creatingChild}
        title={t`New subfolder`}
        confirmLabel={t`Create`}
        onOpenChange={setCreatingChild}
        onSubmit={async (name) => {
          const nested = childFolderPath(folderPath, name);
          if (!nested) {
            throw new Error("invalid folder path");
          }
          const created = await createNamedFolder(nested);
          onFolderChanged(created);
        }}
      />
      <DestructiveConfirmationDialog
        open={deleting}
        onOpenChange={setDeleting}
        title={<Trans>Delete folder</Trans>}
        description={
          <Trans>
            Notes stay in All notes. Materials and subfolders will be deleted.
          </Trans>
        }
        confirmLabel={<Trans>Delete folder</Trans>}
        isPending={busy}
        onConfirm={() => {
          void (async () => {
            setBusy(true);
            try {
              await deleteNamedFolder(folderPath);
              setDeleting(false);
              onFolderChanged(null);
            } finally {
              setBusy(false);
            }
          })();
        }}
      />
    </section>
  );
}
