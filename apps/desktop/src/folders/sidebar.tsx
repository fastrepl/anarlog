import { Trans, useLingui } from "@lingui/react/macro";
import { FolderSimple, MagnifyingGlass, Plus, X } from "@phosphor-icons/react";
import { useMemo, useState } from "react";

import { Button } from "@anlg/ui/components/ui/button";
import { cn } from "@anlg/utils";

import { useActiveFolderPath, useFolderSelection } from "./selection";

import { createNamedFolder } from "~/session/folder-catalog";
import { resolvedFolderIcon } from "~/session/folder-icon";
import { useFolderIcons, useFolderPaths } from "~/session/queries";
import { CustomSidebarHeader } from "~/sidebar/custom-sidebar-header";
import { FolderNameDialog } from "~/sidebar/folder-name-dialog";
import { TemplateIconGlyph } from "~/templates/template-icon";

export function FoldersSidebar() {
  const { t } = useLingui();
  const folders = useFolderPaths();
  const persistedIcons = useFolderIcons();
  const iconOverrides = useFolderSelection((state) => state.iconOverrides);
  const setSelectedPath = useFolderSelection((state) => state.setSelectedPath);
  const activeFolder = useActiveFolderPath(folders);
  const [creating, setCreating] = useState(false);
  const [search, setSearch] = useState("");

  const filteredFolders = useMemo(() => {
    const query = search.trim().toLowerCase();
    if (!query) {
      return folders;
    }
    return folders.filter((folder) => folder.toLowerCase().includes(query));
  }, [folders, search]);

  const isEmpty = filteredFolders.length === 0;

  return (
    <div className="flex h-full flex-col overflow-hidden">
      <div>
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

        <div className="pb-2">
          <div
            className={cn([
              "border-border bg-accent/50 flex h-8 w-full shrink-0 items-center gap-2 rounded-lg border px-3",
              "focus-within:bg-accent transition-colors",
            ])}
          >
            <MagnifyingGlass className="text-muted-foreground h-4 w-4 shrink-0" />
            <input
              type="text"
              value={search}
              onChange={(event) => setSearch(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === "Escape") {
                  setSearch("");
                }
              }}
              placeholder={t`Search folders...`}
              className="placeholder:text-muted-foreground min-w-0 flex-1 bg-transparent text-sm placeholder:text-sm focus:outline-hidden"
            />
            {search ? (
              <button
                type="button"
                onClick={() => setSearch("")}
                className={cn([
                  "h-4 w-4 shrink-0",
                  "text-muted-foreground hover:text-muted-foreground",
                  "transition-colors",
                ])}
                aria-label={t`Clear search`}
              >
                <X className="h-4 w-4" />
              </button>
            ) : null}
          </div>
        </div>
      </div>

      <div className="scrollbar-hide min-h-0 flex-1 overflow-y-auto pt-1">
        {isEmpty ? (
          <div className="text-muted-foreground px-3 py-8 text-center">
            <FolderSimple
              size={32}
              className="text-muted-foreground/70 mx-auto mb-2"
            />
            <p className="text-sm">
              {search ? (
                <Trans>No folders found</Trans>
              ) : (
                <Trans>No folders yet</Trans>
              )}
            </p>
          </div>
        ) : (
          <ul className="flex flex-col">
            {filteredFolders.map((folder) => {
              const selected = folder === activeFolder;
              return (
                <li key={folder}>
                  <button
                    type="button"
                    aria-label={folder}
                    aria-current={selected ? "page" : undefined}
                    onClick={() => setSelectedPath(folder)}
                    className={cn([
                      "w-full rounded-lg px-3 py-2 text-left text-sm transition-colors select-none",
                      selected
                        ? "bg-accent font-medium"
                        : "text-muted-foreground hover:bg-accent/50 hover:text-foreground",
                    ])}
                  >
                    <span className="flex min-w-0 items-center gap-2">
                      <TemplateIconGlyph
                        icon={resolvedFolderIcon(
                          folder,
                          persistedIcons,
                          iconOverrides,
                        )}
                        className="size-4 text-sm"
                      />
                      <span className="min-w-0 truncate">{folder}</span>
                    </span>
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
