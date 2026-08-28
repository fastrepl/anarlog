import { Trans, useLingui } from "@lingui/react/macro";
import { Users } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { useMemo } from "react";

import { colors, radii, spacing } from "@anlg/design-system/tokens.stylex";

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
    <div {...stylex.props(styles.root)}>
      <div>
        {notes.length === 0 ? (
          <div {...stylex.props(styles.empty)}>
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
              {...stylex.props(
                styles.note,
                selected ? styles.noteSelected : styles.noteIdle,
              )}
            >
              <span {...stylex.props(styles.title)}>
                {note.title || t`Untitled`}
              </span>
              <Users
                aria-label={t`Shared note`}
                {...stylex.props(styles.icon)}
              />
            </button>
          );
        })}
      </div>
    </div>
  );
}

const styles = stylex.create({
  empty: {
    color: colors.mutedForeground,
    fontSize: "0.875rem",
    paddingBlock: spacing.xl,
    paddingInline: spacing.sm,
    textAlign: "center",
  },
  icon: {
    color: colors.mutedForeground,
    flexShrink: 0,
    height: "0.875rem",
    width: "0.875rem",
  },
  note: {
    alignItems: "center",
    borderRadius: radii.lg,
    display: "flex",
    fontSize: "0.875rem",
    gap: spacing.sm,
    minWidth: 0,
    paddingBlock: spacing.sm,
    paddingInline: spacing.md,
    textAlign: "left",
    width: "100%",
  },
  noteIdle: {
    backgroundColor: {
      default: null,
      ":hover": `color-mix(in oklab, ${colors.accent} 50%, transparent)`,
    },
  },
  noteSelected: {
    backgroundColor: colors.accent,
    color: colors.accentForeground,
  },
  root: {
    height: "100%",
    overflowY: "auto",
    paddingTop: spacing.sm,
  },
  title: {
    flex: "1",
    minWidth: 0,
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
  },
});
