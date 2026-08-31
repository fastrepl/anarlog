import { useLingui } from "@lingui/react/macro";
import { CaretRight, Check, Plus } from "@phosphor-icons/react";
import { useCallback, useMemo, useState } from "react";

import {
  Command,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
} from "@anlg/ui/components/ui/command";
import {
  AppFloatingPanel,
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@anlg/ui/components/ui/popover";
import { useSquircleRef } from "@anlg/ui/hooks/use-squircle";
import { squircleFocusVisibleClassName } from "@anlg/ui/lib/squircle";
import { cn } from "@anlg/utils";

import { useFolderSelection } from "~/folders/selection";
import { createNamedFolder } from "~/session/folder-catalog";
import { resolvedFolderIcon } from "~/session/folder-icon";
import { normalizeFolderPath } from "~/session/folders";
import {
  useFolderIcons,
  useFolderPaths,
  useSession,
  useUpdateSession,
} from "~/session/queries";
import { useTabs } from "~/store/zustand/tabs";
import { TemplateIconGlyph } from "~/templates/template-icon";

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
  const triggerRef = useSquircleRef<HTMLButtonElement>();
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [highlighted, setHighlighted] = useState("");
  const folderId = useSession(sessionId)?.folder_id ?? "";
  const folderPaths = useFolderPaths();
  const folderIcons = useFolderIcons();
  const updateSession = useUpdateSession(sessionId);
  const openNew = useTabs((state) => state.openNew);
  const setSelectedPath = useFolderSelection((state) => state.setSelectedPath);
  const currentPath = normalizeFolderPath(folderId) ?? "";
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

  const handleOpenChange = useCallback(
    (nextOpen: boolean) => {
      setOpen(nextOpen);
      if (nextOpen) {
        setHighlighted(currentPath);
        return;
      }
      setQuery("");
    },
    [currentPath],
  );

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

      void (async () => {
        try {
          if (normalized && !folderPaths.includes(normalized)) {
            await createNamedFolder(normalized);
            setSelectedPath(normalized);
          }
          await updateSession({ folder_id: normalized });
        } catch (error) {
          console.error("[folder-picker] failed to update folder", error);
        }
      })();
    },
    [folderId, folderPaths, setSelectedPath, updateSession],
  );

  const handleSeeAllFolders = useCallback(() => {
    setOpen(false);
    setQuery("");
    if (currentPath) {
      setSelectedPath(currentPath);
    }
    openNew({ type: "folders" });
  }, [currentPath, openNew, setSelectedPath]);

  return (
    <Popover open={open} onOpenChange={handleOpenChange}>
      <PopoverTrigger asChild>
        <button
          ref={triggerRef}
          type="button"
          data-tauri-drag-region="false"
          role="combobox"
          aria-expanded={open}
          aria-label={
            currentPath ? t`Folder: ${currentPath}` : t`Select folder`
          }
          title={currentPath ? currentPath : t`Select folder`}
          className={cn([
            "flex h-7 items-center rounded-full [&_svg]:size-4",
            currentPath
              ? "max-w-full min-w-0 gap-1 px-1.5"
              : "w-7 justify-center",
            "text-muted-foreground hover:bg-accent hover:text-foreground transition-colors",
            squircleFocusVisibleClassName,
            open && "bg-accent text-foreground",
          ])}
        >
          <TemplateIconGlyph
            icon={resolvedFolderIcon(currentPath, folderIcons)}
            className="size-4"
          />
          {currentPath ? (
            <span className="min-w-0 truncate text-xs text-neutral-600 dark:text-neutral-300">
              {currentPath}
            </span>
          ) : null}
        </button>
      </PopoverTrigger>
      <PopoverContent
        variant="app"
        align={align}
        className="w-56 overflow-hidden pb-0"
      >
        <div className="flex flex-col">
          <AppFloatingPanel className="overflow-hidden">
            <Command
              filter={filterFolders}
              value={highlighted}
              onValueChange={setHighlighted}
              className="rounded-[inherit] border-0 bg-transparent **:[[cmdk-input-wrapper]]:h-8 **:[[cmdk-input-wrapper]]:px-2.5"
            >
              <CommandInput
                placeholder={t`Search or create folder`}
                value={query}
                onValueChange={setQuery}
                className="h-8 py-0"
              />
              <CommandList className="p-1">
                <CommandEmpty className="text-muted-foreground px-2.5 py-2 text-left text-sm">
                  {trimmedQuery
                    ? normalizedQuery === null
                      ? t`Enter a valid folder name.`
                      : t`No folders found.`
                    : t`No folders yet.`}
                </CommandEmpty>
                {folders.length > 0 ? (
                  <CommandGroup>
                    {folders.map((path) => (
                      <CommandItem
                        key={path}
                        value={path}
                        onSelect={() =>
                          handleSelect(path === currentPath ? "" : path)
                        }
                        className="cursor-pointer"
                      >
                        <TemplateIconGlyph
                          icon={resolvedFolderIcon(path, folderIcons)}
                          className="size-4 opacity-70"
                        />
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
          <button
            type="button"
            onClick={handleSeeAllFolders}
            className={cn([
              "flex w-full items-center justify-center gap-1 px-3 py-1.5 text-xs font-medium",
              "text-muted-foreground hover:bg-accent hover:text-foreground transition-colors",
            ])}
          >
            {t`See all folders`}
            <CaretRight className="h-3.5 w-3.5" />
          </button>
        </div>
      </PopoverContent>
    </Popover>
  );
}

function collectWithCurrent(folderPaths: string[], currentPath: string) {
  return [...folderPaths, currentPath].sort((left, right) =>
    left.localeCompare(right),
  );
}
