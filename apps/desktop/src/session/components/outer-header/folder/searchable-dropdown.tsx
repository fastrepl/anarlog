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
  AppFloatingPanel,
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuSubContent,
  DropdownMenuTrigger,
} from "@hypr/ui/components/ui/dropdown-menu";

import { useBillingAccess } from "~/auth/billing";
import { sessionOps } from "~/store/tinybase/persister/session/ops";
import * as main from "~/store/tinybase/store/main";
import { useListener } from "~/stt/contexts";

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
  const [error, setError] = useState<string | null>(null);
  const folders = useFolders();
  const currentFolderId = useSessionFolderId(sessionId);
  const moveDisabledReason = useMoveDisabledReason(sessionId);

  const handleSelectFolder = useMoveSessionToFolder(sessionId);

  return (
    <DropdownMenu open={open} onOpenChange={setOpen}>
      <DropdownMenuTrigger asChild>{trigger}</DropdownMenuTrigger>
      <DropdownMenuContent variant="app" align="start" className="w-50">
        {Object.keys(folders).length ? (
          <SearchableFolderContent
            folders={folders}
            onSelectFolder={handleSelectFolder}
            currentFolderId={currentFolderId}
            disabledReason={moveDisabledReason}
            error={error}
            setError={setError}
            setOpen={setOpen}
          />
        ) : (
          <AppFloatingPanel className="py-6 text-center text-sm text-neutral-500">
            No folders available
          </AppFloatingPanel>
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
  const [error, setError] = useState<string | null>(null);
  const folders = useFolders();
  const currentFolderId = useSessionFolderId(sessionId);
  const moveDisabledReason = useMoveDisabledReason(sessionId);

  const handleSelectFolder = useMoveSessionToFolder(sessionId);

  return (
    <DropdownMenuSubContent variant="app" className="w-50">
      {Object.keys(folders).length ? (
        <SearchableFolderContent
          folders={folders}
          onSelectFolder={handleSelectFolder}
          currentFolderId={currentFolderId}
          disabledReason={moveDisabledReason}
          error={error}
          setError={setError}
          setOpen={setOpen}
        />
      ) : (
        <AppFloatingPanel className="py-6 text-center text-sm text-neutral-500">
          No folders available
        </AppFloatingPanel>
      )}
    </DropdownMenuSubContent>
  );
}

function SearchableFolderContent({
  folders,
  onSelectFolder,
  currentFolderId,
  disabledReason,
  error,
  setError,
  setOpen,
}: {
  folders: Record<string, { name: string }>;
  onSelectFolder: (
    folderId: string,
  ) => Promise<{ status: "ok" } | { status: "error"; error: string }>;
  currentFolderId: string;
  disabledReason: string;
  error: string | null;
  setError: (error: string | null) => void;
  setOpen?: (open: boolean) => void;
}) {
  const handleSelect = async (folderId: string) => {
    if (disabledReason) {
      setError(disabledReason);
      return;
    }

    const result = await onSelectFolder(folderId);
    if (result.status === "ok") {
      setError(null);
      setOpen?.(false);
      return;
    }

    setError(result.error);
  };

  return (
    <AppFloatingPanel className="overflow-hidden">
      <Command className="rounded-[inherit] border-0 bg-transparent">
        <CommandInput
          placeholder="Search folders..."
          autoFocus
          className="h-9"
        />
        <CommandList>
          <CommandEmpty>No folders found.</CommandEmpty>
          <CommandGroup>
            {Object.entries(folders).map(([folderId, folder]) => (
              <CommandItem
                key={folderId}
                value={folder.name}
                disabled={!!disabledReason || folderId === currentFolderId}
                onSelect={() => handleSelect(folderId)}
              >
                <FolderIcon />
                {folder.name}
              </CommandItem>
            ))}
          </CommandGroup>
          {(disabledReason || error) && (
            <div className="px-2 py-2 text-xs text-red-500">
              {error ?? disabledReason}
            </div>
          )}
        </CommandList>
      </Command>
    </AppFloatingPanel>
  );
}

function useMoveSessionToFolder(sessionId: string) {
  return useCallback(
    async (targetFolderId: string) => {
      return sessionOps.moveSessionToFolder(sessionId, targetFolderId);
    },
    [sessionId],
  );
}

function useSessionFolderId(sessionId: string) {
  return (
    (main.UI.useCell("sessions", sessionId, "folder_id", main.STORE_ID) as
      | string
      | undefined) ?? ""
  );
}

function useMoveDisabledReason(sessionId: string) {
  const { isPro } = useBillingAccess();
  const sessionMode = useListener((state) => state.getSessionMode(sessionId));

  if (!isPro) {
    return "Upgrade to Pro to move notes into folders.";
  }

  if (sessionMode === "active" || sessionMode === "finalizing") {
    return "Stop listening before moving this note.";
  }

  return "";
}
