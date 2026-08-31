import { Trans, useLingui } from "@lingui/react/macro";
import { FolderSimple, Plus } from "@phosphor-icons/react";
import { useState } from "react";

import { Button } from "@anlg/ui/components/ui/button";
import { cn } from "@anlg/utils";

import { useActiveFolderPath, useFolderSelection } from "./selection";

import { createNamedFolder } from "~/session/folder-catalog";
import { useFolderPaths } from "~/session/queries";
import { CustomSidebarHeader } from "~/sidebar/custom-sidebar-header";
import { FolderNameDialog } from "~/sidebar/folder-name-dialog";

export function FoldersSidebar() {
  const { t } = useLingui();
  const folders = useFolderPaths();
  const setSelectedPath = useFolderSelection((state) => state.setSelectedPath);
  const activeFolder = useActiveFolderPath(folders);
  const [creating, setCreating] = useState(false);

  return (
    <div className="flex h-full flex-col overflow-hidden">
      <CustomSidebarHeader>
        <Button
          type="button"
          size="icon"
          variant="ghost"
          className="text-muted-foreground hover:text-foreground relative z-[60]"
          aria-label={t`New folder`}
          onClick={() => setCreating(true)}
        >
          <Plus size={16} />
        </Button>
      </CustomSidebarHeader>

      <div className="scrollbar-hide min-h-0 flex-1 overflow-y-auto pt-1">
        {folders.length === 0 ? (
          <div className="text-muted-foreground px-3 py-8 text-center">
            <FolderSimple
              size={32}
              className="text-muted-foreground/70 mx-auto mb-2"
            />
            <p className="text-sm">
              <Trans>No folders yet</Trans>
            </p>
          </div>
        ) : (
          <ul className="flex flex-col">
            {folders.map((folder) => {
              const depth = folder.split("/").length - 1;
              const selected = folder === activeFolder;
              return (
                <li key={folder}>
                  <button
                    type="button"
                    aria-current={selected ? "page" : undefined}
                    onClick={() => setSelectedPath(folder)}
                    className={cn([
                      "w-full rounded-lg py-2 text-left text-sm transition-colors select-none",
                      selected
                        ? "bg-accent font-medium"
                        : "text-muted-foreground hover:bg-accent/50 hover:text-foreground",
                    ])}
                    style={{ paddingLeft: `${12 + depth * 12}px` }}
                  >
                    <span className="min-w-0 truncate">{folder}</span>
                  </button>
                </li>
              );
            })}
          </ul>
        )}
      </div>

      <FolderNameDialog
        open={creating}
        title={t`New folder`}
        confirmLabel={t`Create`}
        onOpenChange={setCreating}
        onSubmit={async (path) => {
          const created = await createNamedFolder(path);
          setSelectedPath(created);
        }}
      />
    </div>
  );
}
