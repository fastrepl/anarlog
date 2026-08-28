import { useLingui } from "@lingui/react/macro";
import * as stylex from "@stylexjs/stylex";
import { platform } from "@tauri-apps/plugin-os";
import { format } from "date-fns";
import { useCallback, useMemo } from "react";

import { colors, fonts, radii } from "@anlg/design-system/tokens.stylex";
import { commands as fsSyncCommands } from "@anlg/plugin-fs-sync";
import { commands as openerCommands } from "@anlg/plugin-opener2";
import { Button } from "@anlg/ui/components/ui/button";
import {
  AppFloatingPanel,
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@anlg/ui/components/ui/popover";

import { toTz, useTimezone } from "~/calendar/hooks";
import { useDeleteSession } from "~/session/hooks/useDeleteSession";
import { getSessionEvent } from "~/session/utils";
import {
  type MenuItemDef,
  useNativeContextMenu,
} from "~/shared/hooks/useNativeContextMenu";
import type { TimelineSessionRow } from "~/sidebar/timeline/utils";
import { useTabs } from "~/store/zustand/tabs";

export function SessionChip({
  sessionId,
  session,
}: {
  sessionId: string;
  session: TimelineSessionRow | undefined;
}) {
  const { t } = useLingui();
  const tz = useTimezone();
  const deleteSession = useDeleteSession();
  const title = session?.title ?? undefined;
  const eventJson = session?.event_json;
  const createdAt = session?.created_at
    ? format(toTz(session.created_at, tz), "h:mm a")
    : null;

  const handleShowInFolder = useCallback(async () => {
    const result = await fsSyncCommands.sessionDir(sessionId);
    if (result.status === "ok") {
      await openerCommands.openPath(result.data, null);
    }
  }, [sessionId]);

  const handleDelete = useCallback(() => {
    const sessionEvent = getSessionEvent({ event_json: eventJson });
    deleteSession(sessionId, {
      trackingId: sessionEvent?.tracking_id,
      title,
    });
  }, [deleteSession, sessionId, eventJson, title]);

  const contextMenu = useMemo<MenuItemDef[]>(
    () => [
      {
        id: "show",
        text: platform() === "macos" ? t`Show in Finder` : t`Show in folder`,
        action: handleShowInFolder,
      },
      { separator: true },
      {
        id: "delete",
        text: "Delete Note",
        action: handleDelete,
      },
    ],
    [t, handleShowInFolder, handleDelete],
  );
  const showContextMenu = useNativeContextMenu(contextMenu);

  if (!session || !title) {
    return null;
  }

  return (
    <Popover>
      <PopoverTrigger asChild>
        <button {...stylex.props(styles.chip)} onContextMenu={showContextMenu}>
          <div {...stylex.props(styles.marker)} />
          <span {...stylex.props(styles.truncate)}>{title}</span>
          {createdAt && <span {...stylex.props(styles.time)}>{createdAt}</span>}
        </button>
      </PopoverTrigger>
      <PopoverContent
        variant="app"
        align="start"
        sx={styles.popover}
        onClick={(e) => e.stopPropagation()}
      >
        <AppFloatingPanel>
          <SessionPopoverContent sessionId={sessionId} session={session} />
        </AppFloatingPanel>
      </PopoverContent>
    </Popover>
  );
}

function SessionPopoverContent({
  sessionId,
  session,
}: {
  sessionId: string;
  session: TimelineSessionRow;
}) {
  const openCurrent = useTabs((state) => state.openCurrent);
  const tz = useTimezone();

  const handleOpen = useCallback(() => {
    openCurrent({ type: "sessions", id: sessionId });
  }, [openCurrent, sessionId]);

  const createdAt = session.created_at
    ? format(toTz(session.created_at, tz), "MMM d, yyyy h:mm a")
    : null;

  return (
    <div {...stylex.props(styles.popoverContent)}>
      <div {...stylex.props(styles.title)}>{session.title}</div>
      <div {...stylex.props(styles.separator)} />
      {createdAt && <div {...stylex.props(styles.createdAt)}>{createdAt}</div>}
      <Button size="sm" sx={styles.openButton} onClick={handleOpen}>
        Open note
      </Button>
    </div>
  );
}

const styles = stylex.create({
  chip: {
    alignItems: "center",
    cursor: "pointer",
    display: "flex",
    fontSize: "0.75rem",
    gap: "0.25rem",
    lineHeight: 1.25,
    opacity: {
      default: 1,
      ":hover": 0.8,
    },
    paddingLeft: "0.125rem",
    borderRadius: "0.25rem",
    textAlign: "left",
    userSelect: "none",
    width: "100%",
  },
  createdAt: {
    color: colors.mutedForeground,
    fontSize: "0.875rem",
    lineHeight: "1.25rem",
  },
  marker: {
    alignSelf: "stretch",
    backgroundColor: "transparent",
    borderColor: colors.border,
    borderRadius: radii.full,
    borderStyle: "solid",
    borderWidth: "1px",
    flexShrink: 0,
    width: "4px",
  },
  openButton: {
    minHeight: "2rem",
    width: "100%",
  },
  popover: {
    width: "280px",
  },
  popoverContent: {
    display: "flex",
    flexDirection: "column",
    gap: "0.75rem",
    padding: "1rem",
  },
  separator: {
    backgroundColor: colors.accent,
    height: "1px",
  },
  time: {
    color: colors.mutedForeground,
    flexShrink: 0,
    fontFamily: fonts.mono,
    marginLeft: "auto",
  },
  title: {
    color: colors.foreground,
    fontSize: "1rem",
    fontWeight: 500,
    lineHeight: "1.5rem",
  },
  truncate: {
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
  },
});
