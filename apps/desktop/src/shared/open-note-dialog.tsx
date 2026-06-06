import { Command as CommandPrimitive } from "cmdk";
import { FileTextIcon, SearchIcon } from "lucide-react";
import {
  createContext,
  useCallback,
  useContext,
  useMemo,
  useState,
} from "react";
import { createPortal } from "react-dom";
import { useHotkeys } from "react-hotkeys-hook";

import { Kbd } from "@hypr/ui/components/ui/kbd";
import { cn } from "@hypr/utils";

import { useMainContentCenterOffset } from "~/shared/main/content-offset";
import * as main from "~/store/tinybase/store/main";
import { useTabs } from "~/store/zustand/tabs";

const MAX_RECENT_DISPLAY = 5;

interface OpenNoteDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  mainContentCenterOffset?: number;
}

type OpenNoteDialogContextValue = {
  open: () => void;
};

type Session = {
  id: string;
  title: string;
  createdAt: string;
};

const OpenNoteDialogContext = createContext<OpenNoteDialogContextValue | null>(
  null,
);

export function OpenNoteDialogProvider({
  children,
}: {
  children: React.ReactNode;
}) {
  const [open, setOpen] = useState(false);
  const mainContentCenterOffset = useMainContentCenterOffset();

  const openDialog = useCallback(() => {
    setOpen(true);
  }, []);

  useHotkeys("mod+k", openDialog, {
    preventDefault: true,
    enableOnFormTags: true,
    enableOnContentEditable: true,
  });

  const value = useMemo(() => ({ open: openDialog }), [openDialog]);

  return (
    <OpenNoteDialogContext.Provider value={value}>
      {children}
      <OpenNoteDialog
        open={open}
        onOpenChange={setOpen}
        mainContentCenterOffset={mainContentCenterOffset}
      />
    </OpenNoteDialogContext.Provider>
  );
}

export function useOpenNoteDialog() {
  const context = useContext(OpenNoteDialogContext);
  if (!context) {
    throw new Error(
      "useOpenNoteDialog must be used within OpenNoteDialogProvider",
    );
  }
  return context;
}

export function OpenNoteDialog({
  open,
  onOpenChange,
  mainContentCenterOffset = 0,
}: OpenNoteDialogProps) {
  const [query, setQuery] = useState("");
  const openCurrent = useTabs((state) => state.openCurrent);
  const recentlyOpenedSessionIds = useTabs(
    (state) => state.recentlyOpenedSessionIds,
  );

  const sessionIds = main.UI.useRowIds("sessions", main.STORE_ID);
  const store = main.UI.useStore(main.STORE_ID);

  const sessionsMap = useMemo(() => {
    if (!store || !sessionIds) return new Map<string, Session>();

    const map = new Map<string, Session>();
    for (const id of sessionIds) {
      map.set(id, {
        id,
        title: (store.getCell("sessions", id, "title") as string) || "Untitled",
        createdAt: store.getCell("sessions", id, "created_at") as string,
      });
    }
    return map;
  }, [sessionIds, store]);

  const allSessionsSortedByDate = useMemo(() => {
    return Array.from(sessionsMap.values()).sort((a, b) => {
      if (!a.createdAt || !b.createdAt) return 0;
      return new Date(b.createdAt).getTime() - new Date(a.createdAt).getTime();
    });
  }, [sessionsMap]);

  const recentSessions = useMemo(() => {
    return recentlyOpenedSessionIds
      .slice(0, MAX_RECENT_DISPLAY)
      .map((id) => sessionsMap.get(id))
      .filter((s): s is Session => s !== undefined);
  }, [recentlyOpenedSessionIds, sessionsMap]);

  const recentSessionIdSet = useMemo(() => {
    return new Set(recentSessions.map((s) => s.id));
  }, [recentSessions]);

  const otherSessions = useMemo(() => {
    return allSessionsSortedByDate.filter((s) => !recentSessionIdSet.has(s.id));
  }, [allSessionsSortedByDate, recentSessionIdSet]);

  const filteredRecentSessions = useMemo(() => {
    if (!query.trim()) return recentSessions;
    const lowerQuery = query.toLowerCase();
    return recentSessions.filter((s) =>
      s.title.toLowerCase().includes(lowerQuery),
    );
  }, [recentSessions, query]);

  const filteredOtherSessions = useMemo(() => {
    if (!query.trim()) return otherSessions;
    const lowerQuery = query.toLowerCase();
    return otherSessions.filter((s) =>
      s.title.toLowerCase().includes(lowerQuery),
    );
  }, [otherSessions, query]);

  const hasAnyResults =
    filteredRecentSessions.length > 0 || filteredOtherSessions.length > 0;

  const handleOpenChange = useCallback(
    (nextOpen: boolean) => {
      if (!nextOpen) {
        setQuery("");
      }
      onOpenChange(nextOpen);
    },
    [onOpenChange],
  );

  const focusInput = useCallback((node: HTMLInputElement | null) => {
    node?.focus();
  }, []);

  const handleSelect = useCallback(
    (sessionId: string) => {
      handleOpenChange(false);
      openCurrent({ type: "sessions", id: sessionId });
    },
    [handleOpenChange, openCurrent],
  );

  if (!open) return null;

  return createPortal(
    <div
      className="fixed inset-0 z-50 bg-black/20 backdrop-blur-xs"
      onClick={() => handleOpenChange(false)}
    >
      <div
        data-tauri-drag-region
        className="absolute top-0 right-0 left-0 h-[15%]"
        onClick={(e) => e.stopPropagation()}
      />
      <div
        className="absolute top-[15%] left-1/2 w-full max-w-lg -translate-x-1/2 px-4"
        style={{ marginLeft: mainContentCenterOffset }}
      >
        <div
          className={cn([
            "border-border/80 bg-background rounded-xl border",
            "shadow-[0_25px_50px_-12px_rgba(0,0,0,0.25)]",
            "overflow-hidden",
          ])}
          onClick={(e) => e.stopPropagation()}
        >
          <CommandPrimitive
            shouldFilter={false}
            className="flex flex-col"
            onKeyDown={(e) => {
              if (e.key === "Escape") {
                handleOpenChange(false);
              }
            }}
          >
            <div className="border-border/60 flex items-center gap-3 border-b px-4 py-3">
              <SearchIcon className="text-muted-foreground h-4 w-4 shrink-0" />
              <CommandPrimitive.Input
                ref={focusInput}
                value={query}
                onValueChange={setQuery}
                placeholder="Find a note..."
                className={cn([
                  "flex-1 bg-transparent text-sm",
                  "placeholder:text-muted-foreground outline-hidden",
                ])}
              />
              <button
                onClick={() => handleOpenChange(false)}
                className={cn([
                  "h-5 w-5 rounded-full",
                  "flex items-center justify-center",
                  "bg-accent/80 hover:bg-accent/80",
                  "text-muted-foreground text-xs",
                  "transition-colors",
                ])}
              >
                ×
              </button>
            </div>

            <CommandPrimitive.List className="max-h-80 overflow-y-auto p-2">
              {!hasAnyResults ? (
                <CommandPrimitive.Empty className="text-muted-foreground py-6 text-center text-sm">
                  No notes found.
                </CommandPrimitive.Empty>
              ) : (
                <>
                  {filteredRecentSessions.length > 0 && (
                    <CommandPrimitive.Group
                      className={
                        filteredOtherSessions.length > 0 ? "pb-1.5" : ""
                      }
                      heading={
                        <div className="text-muted-foreground px-2 py-1.5 text-xs font-medium tracking-wider uppercase">
                          Recent
                        </div>
                      }
                    >
                      {filteredRecentSessions.map((session) => (
                        <CommandPrimitive.Item
                          key={`recent-${session.id}`}
                          value={`recent-${session.id}`}
                          onSelect={() => handleSelect(session.id)}
                          className={cn([
                            "flex cursor-pointer items-center gap-3 rounded-lg px-3 py-2.5",
                            "text-muted-foreground text-sm",
                            "data-[selected=true]:bg-accent/60",
                            "transition-colors",
                          ])}
                        >
                          <FileTextIcon className="text-muted-foreground h-4 w-4 shrink-0" />
                          <span className="truncate">{session.title}</span>
                        </CommandPrimitive.Item>
                      ))}
                    </CommandPrimitive.Group>
                  )}

                  {filteredOtherSessions.length > 0 && (
                    <CommandPrimitive.Group
                      heading={
                        <div className="flex flex-col gap-3">
                          {filteredRecentSessions.length > 0 && (
                            <div className="bg-accent mx-2 h-px" />
                          )}
                          <div className="text-muted-foreground px-2 py-1.5 text-xs font-medium tracking-wider uppercase">
                            All Notes
                          </div>
                        </div>
                      }
                    >
                      {filteredOtherSessions.map((session) => (
                        <CommandPrimitive.Item
                          key={session.id}
                          value={session.id}
                          onSelect={() => handleSelect(session.id)}
                          className={cn([
                            "flex cursor-pointer items-center gap-3 rounded-lg px-3 py-2.5",
                            "text-muted-foreground text-sm",
                            "data-[selected=true]:bg-accent/60",
                            "transition-colors",
                          ])}
                        >
                          <FileTextIcon className="text-muted-foreground h-4 w-4 shrink-0" />
                          <span className="truncate">{session.title}</span>
                        </CommandPrimitive.Item>
                      ))}
                    </CommandPrimitive.Group>
                  )}
                </>
              )}
            </CommandPrimitive.List>

            <div
              className={cn([
                "flex items-center justify-center gap-4 px-4 py-2.5",
                "border-border/60 border-t",
                "text-muted-foreground text-xs",
              ])}
            >
              <span className="flex items-center gap-1.5">
                <Kbd className="text-[10px]">↑↓</Kbd>
                <span>to navigate</span>
              </span>
              <span className="flex items-center gap-1.5">
                <Kbd className="text-[10px]">↵</Kbd>
                <span>to open</span>
              </span>
              <span className="flex items-center gap-1.5">
                <Kbd className="text-[10px]">esc</Kbd>
                <span>to dismiss</span>
              </span>
            </div>
          </CommandPrimitive>
        </div>
      </div>
    </div>,
    document.body,
  );
}
