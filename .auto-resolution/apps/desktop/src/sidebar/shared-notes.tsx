import { Trans, useLingui } from "@lingui/react/macro";
import { UsersRoundIcon } from "lucide-react";
import { useMemo } from "react";

import { cn } from "@hypr/utils";

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

  if (notes.length === 0) return null;

  return (
    <section className="border-border/60 shrink-0 border-b px-2 pt-1 pb-2">
      <div className="text-muted-foreground flex items-center gap-1.5 px-1.5 py-1 text-xs font-medium">
        <UsersRoundIcon className="size-3.5" />
        <span>
          <Trans>Shared with me</Trans>
        </span>
      </div>
      <div className="max-h-40 overflow-y-auto">
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
              <UsersRoundIcon className="size-3.5 shrink-0" />
              <span className="truncate">{note.title || t`Untitled`}</span>
            </button>
          );
        })}
      </div>
    </section>
  );
}
