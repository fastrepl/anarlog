import { Trans, useLingui } from "@lingui/react/macro";
import { Users } from "@phosphor-icons/react";
import { useMemo } from "react";

import { cn } from "@anlg/utils";

import {
  getSidebarWorkspaceFilterId,
  type SidebarNoteFilter,
} from "./note-filter";

import { useAuth } from "~/auth";
import { useSessionSummaries } from "~/session/queries";
import { useDurableSharedNotes } from "~/shared-notes/cache";
import { useTabs } from "~/store/zustand/tabs";

export function SharedNotesNav({
  filter = "all",
}: {
  filter?: SidebarNoteFilter;
}) {
  const { t } = useLingui();
  const { session } = useAuth();
  const sessions = useSessionSummaries();
  const localSessionIds = useMemo(
    () => new Set(sessions.map((session) => session.id)),
    [sessions],
  );
  const workspaceId = getSidebarWorkspaceFilterId(filter);
  const notes = useDurableSharedNotes(session?.user.id)
    .filter(
      (note) => !(note.manageAccess && localSessionIds.has(note.sessionId)),
    )
    .filter((note) => !workspaceId || note.workspaceId === workspaceId);
  const currentTab = useTabs((state) => state.currentTab);
  const openCurrent = useTabs((state) => state.openCurrent);

  const showReceivedNotes =
    filter === "all" || filter === "shared-with-me" || workspaceId !== null;

  if (!showReceivedNotes) return null;

  const focused = filter !== "all";

  if (notes.length === 0 && !focused) return null;

  return (
    <section
      className={cn([
        "px-2 pt-1 pb-2",
        focused
          ? "flex min-h-0 flex-1 flex-col overflow-hidden"
          : "border-border/60 shrink-0 border-b",
      ])}
    >
      <div className="text-muted-foreground flex items-center gap-1.5 px-1.5 py-1 text-xs font-medium">
        <Users className="size-3.5" />
        <span>
          <Trans>Shared with me</Trans>
        </span>
      </div>
      <div
        className={cn([
          "overflow-y-auto",
          focused ? "min-h-0 flex-1" : "max-h-40",
        ])}
      >
        {notes.length === 0 ? (
          <div className="text-muted-foreground px-2 py-6 text-center text-sm">
            <Trans>No shared notes</Trans>
          </div>
        ) : null}
        {notes.map((note) => {
          const selected =
            currentTab?.type === "shared_sessions" &&
            currentTab.id === note.shareId;
          return (
            <button
              key={note.shareId}
              type="button"
              aria-current={selected ? "page" : undefined}
              onClick={() =>
                openCurrent({
                  type: "shared_sessions",
                  id: note.shareId,
                })
              }
              className={cn([
                "flex w-full min-w-0 items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm",
                selected
                  ? "bg-accent text-accent-foreground"
                  : "text-muted-foreground hover:bg-accent/60 hover:text-foreground",
              ])}
            >
              <Users className="size-3.5 shrink-0" />
              <span className="truncate">{note.title || t`Untitled`}</span>
            </button>
          );
        })}
      </div>
    </section>
  );
}
