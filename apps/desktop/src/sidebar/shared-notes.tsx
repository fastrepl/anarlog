import { Trans, useLingui } from "@lingui/react/macro";
import { Users } from "@phosphor-icons/react";
import { useMemo } from "react";

import { cn } from "@anlg/utils";

import { useAuth } from "~/auth";
import { useSessionSummaries } from "~/session/queries";
import { useDurableSharedNotes } from "~/shared-notes/cache";
import { useTabs } from "~/store/zustand/tabs";

export function SharedNotesNav() {
  const { t } = useLingui();
  const { session } = useAuth();
  const sessions = useSessionSummaries();
  const localSessionIds = useMemo(
    () => new Set(sessions.map((session) => session.id)),
    [sessions],
  );
  const notes = useDurableSharedNotes(session?.user.id).filter(
    (note) => !(note.manageAccess && localSessionIds.has(note.sessionId)),
  );
  const currentTab = useTabs((state) => state.currentTab);
  const openCurrent = useTabs((state) => state.openCurrent);

  return (
    <div className="h-full overflow-y-auto pt-2">
      <div>
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
                "flex w-full min-w-0 items-center gap-2 rounded-lg px-3 py-2 text-left text-sm",
                selected
                  ? "bg-accent text-accent-foreground"
                  : "hover:bg-accent/50",
              ])}
            >
              <span className="min-w-0 flex-1 truncate">
                {note.title || t`Untitled`}
              </span>
              <Users
                aria-label={t`Shared note`}
                className="text-muted-foreground size-3.5 shrink-0"
              />
            </button>
          );
        })}
      </div>
    </div>
  );
}
