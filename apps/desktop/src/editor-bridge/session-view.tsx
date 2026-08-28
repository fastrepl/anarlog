import {
  type NodeViewComponentProps,
  useEditorEventCallback,
} from "@handlewithcare/react-prosemirror";
import * as stylex from "@stylexjs/stylex";
import { format } from "date-fns";
import { forwardRef, type ReactNode, useCallback, useMemo } from "react";

import { colors, fonts, radii } from "@anlg/design-system/tokens.stylex";
import { getSafeNodePos, TaskCheckbox } from "@anlg/editor/node-views";
import { useLinkedItemOpenBehavior } from "@anlg/editor/note";
import {
  createTaskStatusAttrs,
  getNextTaskStatus,
  getOptionalTaskStatus,
  normalizeTaskStatus,
} from "@anlg/editor/tasks";
import { safeParseDate } from "@anlg/utils";

import { toTz, useTimezone } from "~/calendar/hooks";
import { useSession } from "~/session/queries";
import { getSessionEvent } from "~/session/utils";
import { useTabs } from "~/store/zustand/tabs";
import { useListener } from "~/stt/contexts";

export const SessionNodeView = forwardRef<
  HTMLDivElement,
  NodeViewComponentProps & { children?: ReactNode }
>(function SessionNodeView({ nodeProps, children, ...htmlAttrs }, ref) {
  const { node, getPos } = nodeProps;
  const sessionId = node.attrs.sessionId as string;

  const session = useSession(sessionId);
  const tz = useTimezone();
  const liveSessionId = useListener((state) => state.live.sessionId);
  const liveStatus = useListener((state) => state.live.status);
  const isRecording =
    liveSessionId === sessionId &&
    (liveStatus === "active" || liveStatus === "finalizing");
  const event = useMemo(() => getSessionEvent(session ?? {}), [session]);
  const displayTime = useMemo(() => {
    if (event?.is_all_day) {
      return null;
    }

    const rawDate = event?.started_at ?? session?.created_at;
    const parsed = rawDate ? safeParseDate(rawDate) : null;

    return parsed ? format(toTz(parsed, tz), "h:mm a") : null;
  }, [event?.is_all_day, event?.started_at, session?.created_at, tz]);

  const isMeetingOver = useMemo(() => {
    if (!event?.ended_at) return false;
    const endedAt = safeParseDate(event.ended_at);
    return endedAt ? endedAt.getTime() <= Date.now() : false;
  }, [event]);

  const linkedItemOpenBehavior = useLinkedItemOpenBehavior();
  const openCurrent = useTabs((state) => state.openCurrent);
  const openNew = useTabs((state) => state.openNew);

  const openSession = useCallback(() => {
    const tab = { id: sessionId, type: "sessions" as const };
    if (linkedItemOpenBehavior === "new") {
      openNew(tab);
      return;
    }

    openCurrent(tab);
  }, [linkedItemOpenBehavior, openCurrent, openNew, sessionId]);

  const handleRowMouseDown = useCallback((event: React.MouseEvent) => {
    event.preventDefault();
    event.stopPropagation();
  }, []);

  const handleRowClick = useCallback(
    (event: React.MouseEvent) => {
      event.preventDefault();
      event.stopPropagation();
      openSession();
    },
    [openSession],
  );

  const derivedChecked = !isRecording && isMeetingOver;
  const explicitStatus = getOptionalTaskStatus(
    node.attrs.status,
    node.attrs.checked,
  );
  const status =
    explicitStatus ?? normalizeTaskStatus(undefined, derivedChecked);

  const handleToggle = useEditorEventCallback((view) => {
    if (!view) return;
    const pos = getSafeNodePos(getPos);
    if (pos === null) return;

    const nextStatus = getNextTaskStatus(status);
    const tr = view.state.tr.setNodeMarkup(pos, undefined, {
      ...node.attrs,
      ...createTaskStatusAttrs(nextStatus),
    });
    view.dispatch(tr);
  });

  return (
    <div
      ref={ref}
      {...htmlAttrs}
      data-status={explicitStatus ?? undefined}
      data-checked={
        explicitStatus ? String(explicitStatus === "done") : undefined
      }
    >
      <div
        data-session-row
        onMouseDown={handleRowMouseDown}
        onClick={handleRowClick}
        {...stylex.props(styles.row)}
      >
        {isRecording ? (
          <div
            {...stylex.props(styles.recordingContainer)}
            contentEditable={false}
          >
            <div {...stylex.props(styles.recordingIndicator)} />
          </div>
        ) : (
          <TaskCheckbox status={status} isInteractive onToggle={handleToggle} />
        )}
        <div {...stylex.props(styles.content)}>
          <div
            ref={nodeProps.contentDOMRef}
            data-session-title
            {...stylex.props(
              styles.title,
              status === "done" && styles.completedTitle,
            )}
          >
            {children}
          </div>
          {displayTime && (
            <span {...stylex.props(styles.time)} contentEditable={false}>
              {displayTime}
            </span>
          )}
        </div>
      </div>
    </div>
  );
});

const recordingPulse = stylex.keyframes({
  "0%, 100%": {
    opacity: 1,
  },
  "50%": {
    opacity: 0.5,
  },
});

const styles = stylex.create({
  completedTitle: {
    opacity: {
      default: null,
      ":is(*) > p": 0.6,
    },
    textDecorationLine: {
      default: null,
      ":is(*) > p": "line-through",
    },
  },
  content: {
    alignItems: "baseline",
    display: "flex",
    flex: "1",
    gap: "0.5rem",
    minWidth: {
      default: 0,
      ":is(*) > p": 0,
    },
  },
  recordingContainer: {
    alignItems: "center",
    display: "flex",
    flexShrink: 0,
    height: "1.125rem",
    justifyContent: "center",
    width: "1.125rem",
  },
  recordingIndicator: {
    animationDuration: "2s",
    animationIterationCount: "infinite",
    animationName: recordingPulse,
    animationTimingFunction: "cubic-bezier(0.4, 0, 0.6, 1)",
    backgroundColor: "#ef4444",
    borderRadius: radii.full,
    height: "0.625rem",
    width: "0.625rem",
  },
  row: {
    alignItems: "flex-start",
    backgroundColor: {
      default: "transparent",
      ":focus-within": colors.muted,
      ":hover": colors.accent,
    },
    borderRadius: radii.md,
    cursor: "pointer",
    display: "flex",
    marginInline: "-0.5rem",
    paddingBlock: "0.25rem",
    paddingInline: "0.5rem",
    transitionDuration: "150ms",
    transitionProperty: "color, background-color",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
  },
  time: {
    color: colors.mutedForeground,
    flexShrink: 0,
    fontFamily: fonts.mono,
    fontSize: "0.75rem",
  },
  title: {
    color: colors.foreground,
    fontSize: "0.875rem",
    margin: {
      default: null,
      ":is(*) > p": 0,
    },
    minWidth: 0,
    overflow: {
      default: null,
      ":is(*) > p": "hidden",
    },
    textOverflow: {
      default: null,
      ":is(*) > p": "ellipsis",
    },
    whiteSpace: {
      default: null,
      ":is(*) > p": "nowrap",
    },
  },
});
