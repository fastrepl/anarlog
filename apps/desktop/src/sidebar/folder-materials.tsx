import { Trans, useLingui } from "@lingui/react/macro";
import { PencilSimple, Plus, Trash, X } from "@phosphor-icons/react";
import { useRef, useState } from "react";

import { cn } from "@anlg/utils";

import { FolderNameDialog } from "./folder-name-dialog";
import { useSidebarNotes } from "./note-filter";

import { useFolderSelection } from "~/folders/selection";
import {
  deleteLocalFolderMaterial,
  diskAttachmentId,
  useFolderMaterials,
} from "~/session/folder-attachments";
import { deleteNamedFolder, renameNamedFolder } from "~/session/folder-catalog";
import { FolderInstructionsField } from "~/session/folder-instructions";
import { useFolderMaterialUpload } from "~/shared/hooks/useFileUpload";
import { DestructiveConfirmationDialog } from "~/shared/ui/destructive-confirmation-dialog";

export function FolderMaterialsPanel({ folderPath }: { folderPath: string }) {
  const { t } = useLingui();
  const materials = useFolderMaterials(folderPath);
  const upload = useFolderMaterialUpload(folderPath);
  const inputRef = useRef<HTMLInputElement>(null);
  const [busy, setBusy] = useState(false);
  const [renaming, setRenaming] = useState(false);
  const [deleting, setDeleting] = useState(false);
  const setView = useSidebarNotes((state) => state.setView);
  const markFolderDeleted = useFolderSelection(
    (state) => state.markFolderDeleted,
  );

  return (
    <>
      <div
        data-testid="folder-materials"
        className="border-border/60 mx-2 mb-1 shrink-0 rounded-lg border px-2 py-1.5"
      >
        <div className="flex items-center gap-1">
          <span
            className="text-foreground min-w-0 flex-1 truncate text-xs font-medium"
            title={folderPath}
          >
            {folderPath}
          </span>
          <button
            type="button"
            aria-label={t`Rename folder`}
            disabled={busy}
            className={cn([
              "text-muted-foreground hover:bg-accent hover:text-foreground flex size-6 items-center justify-center rounded-full",
              "focus-visible:ring-ring focus-visible:ring-2 focus-visible:outline-hidden",
              "disabled:opacity-50",
            ])}
            onClick={() => setRenaming(true)}
          >
            <PencilSimple size={14} />
          </button>
          <button
            type="button"
            aria-label={t`Delete folder`}
            disabled={busy}
            className={cn([
              "text-muted-foreground hover:bg-accent hover:text-destructive flex size-6 items-center justify-center rounded-full",
              "focus-visible:ring-ring focus-visible:ring-2 focus-visible:outline-hidden",
              "disabled:opacity-50",
            ])}
            onClick={() => setDeleting(true)}
          >
            <Trash size={14} />
          </button>
        </div>
        <div className="mt-1 mb-1.5">
          <FolderInstructionsField folderPath={folderPath} />
        </div>
        <div className="flex items-center gap-1">
          <span className="text-muted-foreground min-w-0 flex-1 truncate text-xs font-medium">
            <Trans>Materials</Trans>
          </span>
          <button
            type="button"
            aria-label={t`Add material`}
            disabled={busy}
            className={cn([
              "text-muted-foreground hover:bg-accent hover:text-foreground flex size-6 items-center justify-center rounded-full",
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
          <p className="text-muted-foreground/80 pt-1 text-[11px] leading-4">
            <Trans>Add a syllabus or PDF for this folder</Trans>
          </p>
        ) : (
          <ul className="mt-1 flex max-h-24 flex-col gap-0.5 overflow-y-auto">
            {materials.map((material) => (
              <li
                key={material.id}
                className="flex min-w-0 items-center gap-1 text-xs"
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
                    "text-muted-foreground hover:bg-accent hover:text-foreground flex size-5 items-center justify-center rounded-full",
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
          setView("mine", renamed);
        }}
      />
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
              await deleteNamedFolder(folderPath);
              setDeleting(false);
              markFolderDeleted(folderPath);
              setView("mine", null);
            } finally {
              setBusy(false);
            }
          })();
        }}
      />
    </>
  );
}
