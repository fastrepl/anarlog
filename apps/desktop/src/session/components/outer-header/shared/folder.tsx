import { FolderIcon } from "lucide-react";
import { type ReactNode, useCallback, useMemo, useState } from "react";

import {
  Command,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
} from "@hypr/ui/components/ui/command";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuSubContent,
  DropdownMenuTrigger,
} from "@hypr/ui/components/ui/dropdown-menu";

import { sessionOps } from "~/store/tinybase/persister/session/ops";
import * as main from "~/store/tinybase/store/main";

function useFolders() {
  const sessionIds = main.UI.useRowIds("sessions", main.STORE_ID);
  const store = main.UI.useStore(main.STORE_ID);

  return useMemo(() => {
    if (!store || !sessionIds) return {};

    const folders: Record<string, { name: string }> = {};
    for (const id of sessionIds) {
      const folderId = store.getCell("sessions", id, "folder_id") as string;
      if (folderId && !folders[folderId]) {
        const parts = folderId.split("/");
        folders[folderId] = { name: parts[parts.length - 1] };
      }
    }
    return folders;
  }, [sessionIds, store]);
}

export function SearchableFolderDropdown({
  sessionId,
  trigger,
}: {
  sessionId: string;
  trigger: ReactNode;
}) {
  const [open, setOpen] = useState(false);
  const folders = useFolders();

  const handleSelectFolder = useMoveSessionToFolder(sessionId);

  return (
    <DropdownMenu open={open} onOpenChange={setOpen}>
      <DropdownMenuTrigger asChild>{trigger}</DropdownMenuTrigger>
      <DropdownMenuContent align="start" className="w-50 p-0">
        {Object.keys(folders).length ? (
          <SearchableFolderContent
            folders={folders}
            onSelectFolder={handleSelectFolder}
            setOpen={setOpen}
          />
        ) : (
          <div className="text-muted-foreground py-6 text-center text-sm">
            No folders available
          </div>
        )}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

export function SearchableFolderSubmenuContent({
  sessionId,
  setOpen,
}: {
  sessionId: string;
  setOpen?: (open: boolean) => void;
}) {
  const folders = useFolders();

  const handleSelectFolder = useMoveSessionToFolder(sessionId);

  return (
    <DropdownMenuSubContent className="w-50 p-0">
      {Object.keys(folders).length ? (
        <SearchableFolderContent
          folders={folders}
          onSelectFolder={handleSelectFolder}
          setOpen={setOpen}
        />
      ) : (
        <div className="text-muted-foreground py-6 text-center text-sm">
          No folders available
        </div>
      )}
    </DropdownMenuSubContent>
  );
}

function SearchableFolderContent({
  folders,
  onSelectFolder,
  setOpen,
}: {
  folders: Record<string, { name: string }>;
  onSelectFolder: (folderId: string) => Promise<void>;
  setOpen?: (open: boolean) => void;
}) {
  const handleSelect = async (folderId: string) => {
    await onSelectFolder(folderId);
    setOpen?.(false);
  };

  return (
    <Command>
      <CommandInput placeholder="Search folders..." autoFocus className="h-9" />
      <CommandList>
        <CommandEmpty>No folders found.</CommandEmpty>
        <CommandGroup>
          {Object.entries(folders).map(([folderId, folder]) => (
            <CommandItem
              key={folderId}
              value={folder.name}
              onSelect={() => handleSelect(folderId)}
            >
              <FolderIcon />
              {folder.name}
            </CommandItem>
          ))}
        </CommandGroup>
      </CommandList>
    </Command>
  );
}

function useMoveSessionToFolder(sessionId: string) {
  return useCallback(
    async (targetFolderId: string) => {
      const result = await sessionOps.moveSessionToFolder(
        sessionId,
        targetFolderId,
      );
      if (result.status === "error") {
        console.error("[MoveSession] Failed:", result.error);
      }
    },
    [sessionId],
  );
}
