import { useLingui } from "@lingui/react/macro";
import { Check, Folder, Plus } from "@phosphor-icons/react";
import { useCallback, useMemo, useState } from "react";

import {
  Command,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
  CommandSeparator,
} from "@anlg/ui/components/ui/command";
import {
  AppFloatingPanel,
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@anlg/ui/components/ui/popover";
import { cn } from "@anlg/utils";

import { folderDisplayName, normalizeFolderPath } from "~/session/folders";
import {
  useFolderPaths,
  useSession,
  useUpdateSession,
} from "~/session/queries";

const filterFolders = (value: string, search: string) => {
  const haystack = value.toLocaleLowerCase();
  const needle = search.toLocaleLowerCase();
  return haystack.includes(needle) ? 1 : 0;
};

export function FolderPicker({
  sessionId,
  align = "start",
}: {
  sessionId: string;
  align?: "start" | "end";
}) {
  const { t } = useLingui();
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const folderId = useSession(sessionId)?.folder_id ?? "";
  const folderPaths = useFolderPaths();
  const updateSession = useUpdateSession(sessionId);
  const currentPath = folderDisplayName(folderId);
  const folders = useMemo(() => {
    if (currentPath && !folderPaths.includes(currentPath)) {
      return collectWithCurrent(folderPaths, currentPath);
    }
    return folderPaths;
  }, [currentPath, folderPaths]);
  const trimmedQuery = query.trim();
  const normalizedQuery = normalizeFolderPath(trimmedQuery);
  const canCreateFolder =
    Boolean(normalizedQuery) && !folders.includes(normalizedQuery ?? "");
  const folderName = normalizedQuery ?? "";

  const handleOpenChange = useCallback((nextOpen: boolean) => {
    setOpen(nextOpen);
    if (!nextOpen) {
      setQuery("");
    }
  }, []);

  const handleSelect = useCallback(
    (nextFolderId: string) => {
      const normalized = normalizeFolderPath(nextFolderId);
      if (normalized === null) {
        return;
      }

      setOpen(false);
      setQuery("");
      if (normalized === folderId) {
        return;
      }

      void updateSession({ folder_id: normalized }).catch((error) => {
        console.error("[folder-picker] failed to update folder", error);
      });
    },
    [folderId, updateSession],
  );

  return (
    <Popover open={open} onOpenChange={handleOpenChange}>
      <PopoverTrigger asChild>
        <button
          type="button"
          data-tauri-drag-region="false"
          role="combobox"
          aria-expanded={open}
          aria-label={
            currentPath ? t`Folder: ${currentPath}` : t`Select folder`
          }
          title={currentPath ? currentPath : t`Select folder`}
          className={cn([
            "flex h-7 items-center rounded-full",
            currentPath
              ? "max-w-full min-w-0 gap-1 px-1.5"
              : "w-7 justify-center",
            "text-muted-foreground hover:bg-accent hover:text-foreground transition-colors",
            "focus-visible:ring-ring focus-visible:ring-2 focus-visible:outline-hidden",
            open && "bg-accent text-foreground",
          ])}
        >
          <Folder className="size-4 shrink-0" aria-hidden="true" />
          {currentPath ? (
            <span className="min-w-0 truncate text-xs text-neutral-600 dark:text-neutral-300">
              {currentPath}
            </span>
          ) : null}
        </button>
      </PopoverTrigger>
      <PopoverContent variant="app" align={align} className="w-64 p-0">
        <AppFloatingPanel className="overflow-hidden">
          <Command
            filter={filterFolders}
            className="rounded-[inherit] border-0 bg-transparent"
          >
            <CommandInput
              placeholder={t`Search or create folder`}
              value={query}
              onValueChange={setQuery}
            />
            <CommandList>
              <CommandEmpty>
                <div className="text-muted-foreground px-2 py-1.5 text-sm">
                  {trimmedQuery
                    ? normalizedQuery === null
                      ? t`Enter a valid folder name.`
                      : t`No folders found.`
                    : t`No folders yet.`}
                </div>
              </CommandEmpty>
              {currentPath ? (
                <CommandGroup>
                  <CommandItem
                    value={`no-folder ${t`No folder`}`}
                    onSelect={() => handleSelect("")}
                    className="cursor-pointer"
                  >
                    <span className="flex-1 truncate">{t`No folder`}</span>
                  </CommandItem>
                </CommandGroup>
              ) : null}
              {currentPath && (folders.length > 0 || canCreateFolder) ? (
                <CommandSeparator />
              ) : null}
              {folders.length > 0 ? (
                <CommandGroup>
                  {folders.map((path) => (
                    <CommandItem
                      key={path}
                      value={path}
                      onSelect={() => handleSelect(path)}
                      className="cursor-pointer"
                    >
                      <Folder className="size-4 shrink-0 opacity-70" />
                      <span className="min-w-0 flex-1 truncate">{path}</span>
                      {path === currentPath ? (
                        <Check className="size-4 shrink-0" />
                      ) : null}
                    </CommandItem>
                  ))}
                </CommandGroup>
              ) : null}
              {canCreateFolder && normalizedQuery ? (
                <CommandGroup>
                  <CommandItem
                    value={`create-folder ${normalizedQuery}`}
                    onSelect={() => handleSelect(normalizedQuery)}
                    className="cursor-pointer"
                  >
                    <Plus className="size-4 shrink-0" />
                    <span className="min-w-0 flex-1 truncate">
                      {t`Create "${folderName}"`}
                    </span>
                  </CommandItem>
                </CommandGroup>
              ) : null}
            </CommandList>
          </Command>
        </AppFloatingPanel>
      </PopoverContent>
    </Popover>
  );
}

function collectWithCurrent(folderPaths: string[], currentPath: string) {
  return [...folderPaths, currentPath].sort((left, right) =>
    left.localeCompare(right),
  );
}
