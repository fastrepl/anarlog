import {
  type NodeViewComponentProps,
  useEditorEventCallback,
} from "@handlewithcare/react-prosemirror";
import { format } from "date-fns";
import type { NodeSpec } from "prosemirror-model";
import { forwardRef, type ReactNode, useCallback, useMemo } from "react";

import { cn, safeParseDate } from "@hypr/utils";

import {
  createTaskStatusAttrs,
  getNextTaskStatus,
  getOptionalTaskStatus,
  normalizeTaskStatus,
} from "../tasks";
import { TaskCheckbox } from "./task-checkbox";

import { toTz, useTimezone } from "~/calendar/hooks";
import { useLinkedItemOpenBehavior } from "~/editor/session/linked-item-open-behavior";
import { getSessionEvent } from "~/session/utils";
import * as main from "~/store/tinybase/store/main";
import { useTabs } from "~/store/zustand/tabs";
import { useListener } from "~/stt/contexts";

export const sessionNodeSpec: NodeSpec = {
  group: "block",
  content: "paragraph",
  marks: "",
  defining: true,
  isolating: true,
  selectable: false,
  attrs: {
    sessionId: { default: null },
    status: { default: null },
    checked: { default: null },
  },
  parseDOM: [
    {
      tag: 'div[data-type="session"]',
      getAttrs(dom) {
        const el = dom as HTMLElement;
        const status = getOptionalTaskStatus(
          el.getAttribute("data-status"),
          el.getAttribute("data-checked") === "true"
            ? true
            : el.getAttribute("data-checked") === "false"
              ? false
              : undefined,
        );

        return {
          sessionId: el.getAttribute("data-session-id"),
          status,
          checked: status === null ? null : status === "done",
        };
      },
    },
  ],
  toDOM(node) {
    const status = getOptionalTaskStatus(node.attrs.status, node.attrs.checked);
    return [
      "div",
      {
        "data-type": "session",
        "data-session-id": node.attrs.sessionId,
        "data-status": status ?? undefined,
        "data-checked": status ? String(status === "done") : undefined,
      },
      0,
    ];
  },
};

export const SessionNodeView = forwardRef<
  HTMLDivElement,
  NodeViewComponentProps & { children?: ReactNode }
>(function SessionNodeView({ nodeProps, children, ...htmlAttrs }, ref) {
  const { node, getPos } = nodeProps;
  const sessionId = node.attrs.sessionId as string;

  const session = main.UI.useRow("sessions", sessionId, main.STORE_ID);
  const tz = useTimezone();
  const liveSessionId = useListener((state) => state.live.sessionId);
  const liveStatus = useListener((state) => state.live.status);
  const isRecording =
    liveSessionId === sessionId &&
    (liveStatus === "active" || liveStatus === "finalizing");
  const event = useMemo(() => getSessionEvent(session), [session]);
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
    const pos = getPos();
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
        className={cn([
          "group flex items-start rounded-md px-2 py-1 transition-colors",
          "-mx-2 focus-within:bg-neutral-50 hover:bg-neutral-50",
          "cursor-pointer",
        ])}
      >
        {isRecording ? (
          <div
            className="flex size-[18px] shrink-0 items-center justify-center"
            contentEditable={false}
          >
            <div className="size-2.5 animate-pulse rounded-full bg-red-500" />
          </div>
        ) : (
          <TaskCheckbox status={status} isInteractive onToggle={handleToggle} />
        )}
        <div className="flex min-w-0 flex-1 items-baseline gap-2">
          <div
            data-session-title
            className={cn([
              "min-w-0 text-sm text-neutral-900",
              "[&>p]:m-0 [&>p]:min-w-0 [&>p]:truncate",
              status === "done" && "[&>p]:line-through [&>p]:opacity-60",
            ])}
          >
            {children}
          </div>
          {displayTime && (
            <span
              className="shrink-0 font-mono text-xs text-neutral-400"
              contentEditable={false}
            >
              {displayTime}
            </span>
          )}
        </div>
      </div>
    </div>
  );
});
