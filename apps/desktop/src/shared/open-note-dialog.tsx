import { Trans, useLingui } from "@lingui/react/macro";
import { Command as CommandPrimitive } from "cmdk";
import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
} from "react";
import { useHotkeys } from "react-hotkeys-hook";

import {
  FileText,
  Gear,
  Lock,
  MagnifyingGlass,
  Users,
  X,
  type Icon,
} from "@anlg/ui/components/icons";
import {
  Dialog,
  DialogContent,
  DialogTitle,
} from "@anlg/ui/components/ui/dialog";
import { cn } from "@anlg/utils";

import { trackAnalyticsEvent } from "~/analytics";
import { useAuth } from "~/auth";
import { useBillingAccess } from "~/auth/billing-context";
import { useSessionSummaries } from "~/session/queries";
import { useDurableSharedNotes } from "~/shared-notes/cache";
import { useMainContentCenterOffset } from "~/shared/main/content-offset";
import { useSettingsNavGroups } from "~/sidebar/settings-nav-groups";
import { type TabInput, useTabs } from "~/store/zustand/tabs";

const MAX_RECENT_DISPLAY = 5;

interface OpenNoteDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  mainContentCenterOffset?: number;
}

type OpenNoteDialogContextValue = {
  open: () => void;
};

type NoteResult = {
  resourceType: "session" | "shared_session";
  id: string;
  title: string;
  createdAt: string;
};

type PageResult = {
  id: string;
  label: string;
  hint: string | null;
  groupLabel: string;
  icon: Icon;
  requiresPro: boolean;
  destination: TabInput;
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
  const { t } = useLingui();
  const [query, setQuery] = useState("");
  const openCurrent = useTabs((state) => state.openCurrent);
  const openNew = useTabs((state) => state.openNew);
  const recentlyOpenedSessionIds = useTabs(
    (state) => state.recentlyOpenedSessionIds,
  );
  const { isPro } = useBillingAccess();
  const { session } = useAuth();
  const settingsNavGroups = useSettingsNavGroups();

  const sessions = useSessionSummaries();
  const sharedNotes = useDurableSharedNotes(session?.user.id);

  const pageResults = useMemo<PageResult[]>(
    () => [
      ...settingsNavGroups.flatMap((group) =>
        group.items.map((item): PageResult => {
          const hasDestination = "destination" in item;

          return {
            id: item.id,
            label: item.label,
            hint: hasDestination ? null : t`Settings`,
            groupLabel: group.label,
            icon: item.icon,
            requiresPro: Boolean(item.requiresPro),
            destination: hasDestination
              ? item.destination
              : { type: "settings", state: { tab: item.id } },
          };
        }),
      ),
      {
        id: "settings",
        label: t`Settings`,
        hint: null,
        groupLabel: t`Go to`,
        icon: Gear,
        requiresPro: false,
        destination: { type: "settings", state: { tab: "app" } },
      },
    ],
    [settingsNavGroups, t],
  );

  const topLevelPageIds = new Set([
    "settings",
    ...settingsNavGroups.flatMap((group) =>
      group.items.flatMap((item) => ("destination" in item ? [item.id] : [])),
    ),
  ]);
  const filteredPages = query.trim()
    ? pageResults.filter((page) => {
        const normalizedQuery = query.trim().toLowerCase();
        return (
          page.label.toLowerCase().includes(normalizedQuery) ||
          page.groupLabel.toLowerCase().includes(normalizedQuery) ||
          page.hint?.toLowerCase().includes(normalizedQuery)
        );
      })
    : pageResults.filter((page) => topLevelPageIds.has(page.id));

  const sessionsMap = useMemo(() => {
    return new Map<string, NoteResult>(
      sessions.map((session) => [
        session.id,
        {
          resourceType: "session",
          id: session.id,
          title: session.title || t`Untitled`,
          createdAt: session.created_at,
        },
      ]),
    );
  }, [sessions, t]);

  const allNotesSortedByDate = useMemo(() => {
    return [
      ...sessionsMap.values(),
      ...sharedNotes
        .filter(
          (note) => !(note.manageAccess && sessionsMap.has(note.sessionId)),
        )
        .map(
          (note): NoteResult => ({
            resourceType: "shared_session",
            id: note.shareId,
            title: note.title || t`Untitled`,
            createdAt: note.publishedAt,
          }),
        ),
    ].sort((a, b) => {
      if (!a.createdAt || !b.createdAt) return 0;
      return new Date(b.createdAt).getTime() - new Date(a.createdAt).getTime();
    });
  }, [sessionsMap, sharedNotes, t]);

  const recentSessions = useMemo(() => {
    return recentlyOpenedSessionIds
      .slice(0, MAX_RECENT_DISPLAY)
      .map((id) => sessionsMap.get(id))
      .filter((s): s is NoteResult => s !== undefined);
  }, [recentlyOpenedSessionIds, sessionsMap]);

  const recentSessionIdSet = useMemo(() => {
    return new Set(recentSessions.map((s) => s.id));
  }, [recentSessions]);

  const otherNotes = useMemo(() => {
    return allNotesSortedByDate.filter(
      (note) =>
        note.resourceType === "shared_session" ||
        !recentSessionIdSet.has(note.id),
    );
  }, [allNotesSortedByDate, recentSessionIdSet]);

  const filteredRecentSessions = useMemo(() => {
    if (!query.trim()) return recentSessions;
    const lowerQuery = query.toLowerCase();
    return recentSessions.filter((s) =>
      s.title.toLowerCase().includes(lowerQuery),
    );
  }, [recentSessions, query]);

  const filteredOtherNotes = useMemo(() => {
    if (!query.trim()) return otherNotes;
    const lowerQuery = query.toLowerCase();
    return otherNotes.filter((note) =>
      note.title.toLowerCase().includes(lowerQuery),
    );
  }, [otherNotes, query]);

  const hasAnyResults =
    filteredPages.length > 0 ||
    filteredRecentSessions.length > 0 ||
    filteredOtherNotes.length > 0;

  useEffect(() => {
    if (!open || !query.trim()) return;
    const timeout = setTimeout(() => {
      trackAnalyticsEvent("search_performed", {
        entry_point: "open_note_dialog",
        result_count:
          filteredPages.length +
          filteredRecentSessions.length +
          filteredOtherNotes.length,
        entity_types: [
          ...new Set([
            ...[...filteredRecentSessions, ...filteredOtherNotes].map(
              (note) => note.resourceType,
            ),
            ...(filteredPages.length > 0 ? ["page"] : []),
          ]),
        ].sort(),
      });
    }, 300);
    return () => clearTimeout(timeout);
  }, [
    filteredOtherNotes.length,
    filteredPages.length,
    filteredRecentSessions.length,
    open,
    query,
  ]);

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
    (note: NoteResult) => {
      trackAnalyticsEvent("search_result_opened", {
        entry_point: "open_note_dialog",
        result_type: note.resourceType,
        had_query: Boolean(query.trim()),
      });
      handleOpenChange(false);
      openCurrent(
        note.resourceType === "shared_session"
          ? { type: "shared_sessions", id: note.id }
          : { type: "sessions", id: note.id },
      );
    },
    [handleOpenChange, openCurrent, query],
  );

  const handleSelectPage = useCallback(
    (page: PageResult) => {
      trackAnalyticsEvent("search_result_opened", {
        entry_point: "open_note_dialog",
        result_type: "page",
        page_id: page.id,
        had_query: Boolean(query.trim()),
      });
      handleOpenChange(false);
      openNew(page.destination);
    },
    [handleOpenChange, openNew, query],
  );

  const isQueryEmpty = !query.trim();
  const pageGroup = filteredPages.length > 0 && (
    <CommandPrimitive.Group
      className={
        isQueryEmpty
          ? filteredOtherNotes.length > 0
            ? "pb-1.5"
            : ""
          : filteredRecentSessions.length > 0 || filteredOtherNotes.length > 0
            ? "pb-1.5"
            : ""
      }
      heading={
        <div className="flex flex-col gap-3">
          {isQueryEmpty && filteredRecentSessions.length > 0 && (
            <div className="bg-accent mx-2 h-px" />
          )}
          <div className="text-muted-foreground px-2 py-1.5 text-xs font-medium tracking-wider uppercase">
            <Trans>Go to</Trans>
          </div>
        </div>
      }
    >
      {filteredPages.map((page) => (
        <CommandPrimitive.Item
          key={`page-${page.id}`}
          value={`page-${page.id}`}
          onSelect={() => handleSelectPage(page)}
          className={cn([
            "flex cursor-pointer items-center gap-3 rounded-lg px-3 py-2.5",
            "text-muted-foreground text-sm",
            "data-[selected=true]:bg-accent/60",
            "transition-colors",
          ])}
        >
          <page.icon className="text-muted-foreground h-4 w-4 shrink-0" />
          <span className="truncate">{page.label}</span>
          {page.hint ? (
            <span className="text-muted-foreground ml-auto shrink-0 text-xs">
              {page.hint}
            </span>
          ) : null}
          {page.requiresPro && !isPro ? (
            <Lock
              aria-label={t`Requires Anarlog Pro`}
              className="h-3.5 w-3.5 shrink-0"
            />
          ) : null}
        </CommandPrimitive.Item>
      ))}
    </CommandPrimitive.Group>
  );

  if (!open) return null;

  return (
    <Dialog open={open} onOpenChange={handleOpenChange}>
      <DialogContent
        aria-describedby={undefined}
        overlayClassName="bg-black/20 backdrop-blur-xs"
        overlayChildren={
          <div
            data-open-note-dialog-drag-region
            data-tauri-drag-region
            className="absolute top-0 right-0 left-0 h-[15%]"
            onClick={(event) => event.stopPropagation()}
          />
        }
        className={cn([
          "top-[15%] w-full max-w-lg -translate-y-0 gap-0 border-0 bg-transparent px-4 py-0 shadow-none sm:rounded-none",
          "data-[state=closed]:animate-none data-[state=open]:animate-none",
          "[&>button:last-child]:hidden",
        ])}
        style={{ marginLeft: mainContentCenterOffset }}
        onPointerDownOutside={(event) => {
          const target = event.detail.originalEvent.target;
          if (
            target instanceof Element &&
            target.closest("[data-open-note-dialog-drag-region]")
          ) {
            event.preventDefault();
          }
        }}
      >
        <DialogTitle className="sr-only">
          <Trans>Search notes and pages...</Trans>
        </DialogTitle>
        <div
          className={cn([
            "border-border/80 bg-background rounded-2xl border",
            "shadow-[0_25px_50px_-12px_rgba(0,0,0,0.25)]",
            "overflow-hidden",
          ])}
        >
          <CommandPrimitive shouldFilter={false} className="flex flex-col">
            <div className="border-border/60 flex items-center gap-3 border-b px-4 py-3">
              <MagnifyingGlass className="text-muted-foreground h-4 w-4 shrink-0" />
              <CommandPrimitive.Input
                ref={focusInput}
                value={query}
                onValueChange={setQuery}
                placeholder={t`Search notes and pages...`}
                className={cn([
                  "flex-1 bg-transparent text-sm",
                  "placeholder:text-muted-foreground outline-hidden",
                ])}
              />
              <button
                aria-label={t`Close`}
                onClick={() => handleOpenChange(false)}
                className={cn([
                  "h-5 w-5 rounded-full",
                  "flex items-center justify-center",
                  "bg-accent/80 hover:bg-accent/80",
                  "text-muted-foreground text-xs",
                  "transition-colors",
                ])}
              >
                <X className="h-3 w-3" />
              </button>
            </div>

            <CommandPrimitive.List className="max-h-80 overflow-y-auto p-2">
              {!hasAnyResults ? (
                <CommandPrimitive.Empty className="text-muted-foreground py-6 text-center text-sm">
                  <Trans>No results found.</Trans>
                </CommandPrimitive.Empty>
              ) : (
                <>
                  {isQueryEmpty ? null : pageGroup}

                  {filteredRecentSessions.length > 0 && (
                    <CommandPrimitive.Group
                      className={
                        filteredOtherNotes.length > 0 ||
                        (isQueryEmpty && filteredPages.length > 0)
                          ? "pb-1.5"
                          : ""
                      }
                      heading={
                        <div className="flex flex-col gap-3">
                          {!isQueryEmpty && filteredPages.length > 0 && (
                            <div className="bg-accent mx-2 h-px" />
                          )}
                          <div className="text-muted-foreground px-2 py-1.5 text-xs font-medium tracking-wider uppercase">
                            <Trans>Recent</Trans>
                          </div>
                        </div>
                      }
                    >
                      {filteredRecentSessions.map((session) => (
                        <CommandPrimitive.Item
                          key={`recent-${session.id}`}
                          value={`recent-${session.id}`}
                          onSelect={() => handleSelect(session)}
                          className={cn([
                            "flex cursor-pointer items-center gap-3 rounded-lg px-3 py-2.5",
                            "text-muted-foreground text-sm",
                            "data-[selected=true]:bg-accent/60",
                            "transition-colors",
                          ])}
                        >
                          <FileText className="text-muted-foreground h-4 w-4 shrink-0" />
                          <span className="truncate">{session.title}</span>
                        </CommandPrimitive.Item>
                      ))}
                    </CommandPrimitive.Group>
                  )}

                  {isQueryEmpty ? pageGroup : null}

                  {filteredOtherNotes.length > 0 && (
                    <CommandPrimitive.Group
                      heading={
                        <div className="flex flex-col gap-3">
                          {(filteredPages.length > 0 ||
                            filteredRecentSessions.length > 0) && (
                            <div className="bg-accent mx-2 h-px" />
                          )}
                          <div className="text-muted-foreground px-2 py-1.5 text-xs font-medium tracking-wider uppercase">
                            <Trans>All Notes</Trans>
                          </div>
                        </div>
                      }
                    >
                      {filteredOtherNotes.map((note) => (
                        <CommandPrimitive.Item
                          key={`${note.resourceType}-${note.id}`}
                          value={`${note.resourceType}-${note.id}`}
                          onSelect={() => handleSelect(note)}
                          className={cn([
                            "flex cursor-pointer items-center gap-3 rounded-lg px-3 py-2.5",
                            "text-muted-foreground text-sm",
                            "data-[selected=true]:bg-accent/60",
                            "transition-colors",
                          ])}
                        >
                          {note.resourceType === "shared_session" ? (
                            <Users
                              className="text-muted-foreground h-4 w-4 shrink-0"
                              data-testid="shared-note-icon"
                            />
                          ) : (
                            <FileText className="text-muted-foreground h-4 w-4 shrink-0" />
                          )}
                          <span className="truncate">{note.title}</span>
                        </CommandPrimitive.Item>
                      ))}
                    </CommandPrimitive.Group>
                  )}
                </>
              )}
            </CommandPrimitive.List>
          </CommandPrimitive>
        </div>
      </DialogContent>
    </Dialog>
  );
}
