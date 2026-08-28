import { Trans, useLingui } from "@lingui/react/macro";
import { FileText, MagnifyingGlass, Users, X } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
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

import { colors, media, radii } from "@anlg/design-system/tokens.stylex";
import {
  Dialog,
  DialogContent,
  DialogTitle,
} from "@anlg/ui/components/ui/dialog";

import { trackAnalyticsEvent } from "~/analytics";
import { useAuth } from "~/auth";
import { useSessionSummaries } from "~/session/queries";
import { useDurableSharedNotes } from "~/shared-notes/cache";
import { useMainContentCenterOffset } from "~/shared/main/content-offset";
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

type NoteResult = {
  resourceType: "session" | "shared_session";
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
  const { t } = useLingui();
  const [query, setQuery] = useState("");
  const openCurrent = useTabs((state) => state.openCurrent);
  const recentlyOpenedSessionIds = useTabs(
    (state) => state.recentlyOpenedSessionIds,
  );
  const { session } = useAuth();

  const sessions = useSessionSummaries();
  const sharedNotes = useDurableSharedNotes(session?.user.id);

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
    filteredRecentSessions.length > 0 || filteredOtherNotes.length > 0;

  useEffect(() => {
    if (!open || !query.trim()) return;
    const timeout = setTimeout(() => {
      trackAnalyticsEvent("search_performed", {
        entry_point: "open_note_dialog",
        result_count: filteredRecentSessions.length + filteredOtherNotes.length,
        entity_types: [
          ...new Set(
            [...filteredRecentSessions, ...filteredOtherNotes].map(
              (note) => note.resourceType,
            ),
          ),
        ].sort(),
      });
    }, 300);
    return () => clearTimeout(timeout);
  }, [filteredOtherNotes.length, filteredRecentSessions.length, open, query]);

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

  if (!open) return null;

  return (
    <Dialog open={open} onOpenChange={handleOpenChange}>
      <DialogContent
        aria-describedby={undefined}
        overlaySx={styles.overlay}
        overlayChildren={
          <div
            data-open-note-dialog-drag-region
            data-tauri-drag-region
            {...stylex.props(styles.dragRegion)}
            onClick={(event) => event.stopPropagation()}
          />
        }
        sx={styles.dialog}
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
        <DialogTitle sx={styles.visuallyHidden}>
          <Trans>Find a note...</Trans>
        </DialogTitle>
        <div {...stylex.props(styles.surface)}>
          <CommandPrimitive
            shouldFilter={false}
            {...stylex.props(styles.command)}
          >
            <div {...stylex.props(styles.searchRow)}>
              <MagnifyingGlass {...stylex.props(styles.searchIcon)} />
              <CommandPrimitive.Input
                ref={focusInput}
                value={query}
                onValueChange={setQuery}
                placeholder={t`Find a note...`}
                {...stylex.props(styles.input)}
              />
              <button
                aria-label={t`Close`}
                onClick={() => handleOpenChange(false)}
                {...stylex.props(styles.close)}
              >
                <X {...stylex.props(styles.closeIcon)} />
              </button>
            </div>

            <CommandPrimitive.List {...stylex.props(styles.list)}>
              {!hasAnyResults ? (
                <CommandPrimitive.Empty {...stylex.props(styles.empty)}>
                  <Trans>No notes found.</Trans>
                </CommandPrimitive.Empty>
              ) : (
                <>
                  {filteredRecentSessions.length > 0 && (
                    <CommandPrimitive.Group
                      {...stylex.props(
                        filteredOtherNotes.length > 0 && styles.recentGroup,
                      )}
                      heading={
                        <div {...stylex.props(styles.groupHeading)}>
                          <Trans>Recent</Trans>
                        </div>
                      }
                    >
                      {filteredRecentSessions.map((session) => (
                        <CommandPrimitive.Item
                          key={`recent-${session.id}`}
                          value={`recent-${session.id}`}
                          onSelect={() => handleSelect(session)}
                          {...stylex.props(styles.item)}
                        >
                          <FileText {...stylex.props(styles.itemIcon)} />
                          <span {...stylex.props(styles.itemTitle)}>
                            {session.title}
                          </span>
                        </CommandPrimitive.Item>
                      ))}
                    </CommandPrimitive.Group>
                  )}

                  {filteredOtherNotes.length > 0 && (
                    <CommandPrimitive.Group
                      heading={
                        <div {...stylex.props(styles.allHeading)}>
                          {filteredRecentSessions.length > 0 && (
                            <div {...stylex.props(styles.separator)} />
                          )}
                          <div {...stylex.props(styles.groupHeading)}>
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
                          {...stylex.props(styles.item)}
                        >
                          {note.resourceType === "shared_session" ? (
                            <Users
                              {...stylex.props(styles.itemIcon)}
                              data-testid="shared-note-icon"
                            />
                          ) : (
                            <FileText {...stylex.props(styles.itemIcon)} />
                          )}
                          <span {...stylex.props(styles.itemTitle)}>
                            {note.title}
                          </span>
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

const styles = stylex.create({
  allHeading: {
    display: "flex",
    flexDirection: "column",
    gap: "0.75rem",
  },
  close: {
    alignItems: "center",
    backgroundColor: {
      default: `color-mix(in oklab, ${colors.accent} 80%, transparent)`,
      ":hover": `color-mix(in oklab, ${colors.accent} 80%, transparent)`,
    },
    borderRadius: radii.full,
    color: colors.mutedForeground,
    display: "flex",
    fontSize: "0.75rem",
    height: "1.25rem",
    justifyContent: "center",
    transitionDuration: "150ms",
    transitionProperty:
      "color, background-color, border-color, text-decoration-color, fill, stroke",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
    width: "1.25rem",
  },
  closeIcon: {
    height: "0.75rem",
    width: "0.75rem",
  },
  command: {
    display: "flex",
    flexDirection: "column",
  },
  dialog: {
    animationName: {
      default: null,
      ':is([data-state="closed"])': "none",
      ':is([data-state="open"])': "none",
    },
    backgroundColor: "transparent",
    borderRadius: {
      default: 0,
      [media.sm]: 0,
    },
    borderWidth: 0,
    boxShadow: "none",
    display: {
      default: "grid",
      ":is(*) > button:last-child": "none",
    },
    gap: 0,
    maxWidth: "32rem",
    paddingBlock: 0,
    paddingInline: "1rem",
    top: "15%",
    transform: "translate(-50%, 0)",
    width: "100%",
  },
  dragRegion: {
    height: "15%",
    left: 0,
    position: "absolute",
    right: 0,
    top: 0,
  },
  empty: {
    color: colors.mutedForeground,
    fontSize: "0.875rem",
    paddingBlock: "1.5rem",
    textAlign: "center",
  },
  groupHeading: {
    color: colors.mutedForeground,
    fontSize: "0.75rem",
    fontWeight: 500,
    letterSpacing: "0.05em",
    paddingBlock: "0.375rem",
    paddingInline: "0.5rem",
    textTransform: "uppercase",
  },
  input: {
    backgroundColor: "transparent",
    color: {
      default: null,
      "::placeholder": colors.mutedForeground,
    },
    flex: "1",
    fontSize: "0.875rem",
    outlineWidth: "2px",
    outlineStyle: "solid",
    outlineColor: "transparent",
    outlineOffset: "2px",
  },
  item: {
    alignItems: "center",
    backgroundColor: {
      default: null,
      ':is([data-selected="true"])': `color-mix(in oklab, ${colors.accent} 60%, transparent)`,
    },
    borderRadius: radii.lg,
    color: colors.mutedForeground,
    cursor: "pointer",
    display: "flex",
    fontSize: "0.875rem",
    gap: "0.75rem",
    paddingBlock: "0.625rem",
    paddingInline: "0.75rem",
    transitionDuration: "150ms",
    transitionProperty:
      "color, background-color, border-color, text-decoration-color, fill, stroke",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
  },
  itemIcon: {
    color: colors.mutedForeground,
    flexShrink: 0,
    height: "1rem",
    width: "1rem",
  },
  itemTitle: {
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
  },
  list: {
    maxHeight: "20rem",
    overflowY: "auto",
    padding: "0.5rem",
  },
  overlay: {
    backdropFilter: "blur(4px)",
    backgroundColor: "rgb(0 0 0 / 0.2)",
  },
  recentGroup: {
    paddingBottom: "0.375rem",
  },
  searchIcon: {
    color: colors.mutedForeground,
    flexShrink: 0,
    height: "1rem",
    width: "1rem",
  },
  searchRow: {
    alignItems: "center",
    borderBottomColor: `color-mix(in oklab, ${colors.border} 60%, transparent)`,
    borderBottomStyle: "solid",
    borderBottomWidth: "1px",
    display: "flex",
    gap: "0.75rem",
    paddingBlock: "0.75rem",
    paddingInline: "1rem",
  },
  separator: {
    backgroundColor: colors.accent,
    height: "1px",
    marginInline: "0.5rem",
  },
  surface: {
    backgroundColor: colors.background,
    borderColor: `color-mix(in oklab, ${colors.border} 80%, transparent)`,
    borderRadius: "1rem",
    borderStyle: "solid",
    borderWidth: "1px",
    boxShadow: "0 25px 50px -12px rgba(0, 0, 0, 0.25)",
    overflow: "hidden",
  },
  visuallyHidden: {
    borderWidth: 0,
    clipPath: "inset(50%)",
    height: "1px",
    margin: "-1px",
    overflow: "hidden",
    padding: 0,
    position: "absolute",
    whiteSpace: "nowrap",
    width: "1px",
  },
});
