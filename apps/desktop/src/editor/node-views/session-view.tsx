import {
  type NodeViewComponentProps,
  useEditorEventCallback,
} from "@handlewithcare/react-prosemirror";
import { format } from "date-fns";
import { ArrowUpRightIcon } from "lucide-react";
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
  const liveSessionId = useListener((state) => state.live.sessionId);
  const liveStatus = useListener((state) => state.live.status);
  const isRecording =
    liveSessionId === sessionId &&
    (liveStatus === "active" || liveStatus === "finalizing");
  const createdAt = session?.created_at
    ? safeParseDate(session.created_at as string)
    : null;

  const isMeetingOver = useMemo(() => {
    const event = getSessionEvent(session);
    if (!event?.ended_at) return false;
    const endedAt = safeParseDate(event.ended_at);
    return endedAt ? endedAt.getTime() <= Date.now() : false;
  }, [session]);

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

  const handleOpenMouseDown = useCallback((event: React.MouseEvent) => {
    event.preventDefault();
    event.stopPropagation();
  }, []);

  const handleOpenClick = useCallback(
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
        className={cn([
          "group flex items-start rounded-md px-2 py-1 transition-colors",
          "-mx-2 focus-within:bg-neutral-50 hover:bg-neutral-50",
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
        <div
          data-session-title
          className={cn([
            "min-w-0 flex-1 cursor-text text-sm text-neutral-900",
            "[&>p]:m-0 [&>p]:min-w-0 [&>p]:truncate",
            "[&>p]:rounded-sm [&>p]:outline-none",
            "[&>p:focus]:bg-white/80",
            status === "done" && "[&>p]:line-through [&>p]:opacity-60",
          ])}
        >
          {children}
        </div>
        <div
          className="ml-auto flex shrink-0 items-center gap-1.5"
          contentEditable={false}
        >
          {createdAt && (
            <span className="font-mono text-xs text-neutral-400">
              {format(createdAt, "h:mm a")}
            </span>
          )}
          <button
            type="button"
            onMouseDown={handleOpenMouseDown}
            onClick={handleOpenClick}
            className={cn([
              "flex items-center gap-1 rounded-full border border-neutral-200 bg-white/90 px-2 py-1",
              "cursor-pointer text-[11px] font-medium text-neutral-500 transition-all",
              "opacity-40 hover:border-neutral-300 hover:text-neutral-800",
              "group-focus-within:opacity-100 group-hover:opacity-100",
              "focus-visible:opacity-100 focus-visible:ring-2 focus-visible:ring-blue-200 focus-visible:outline-none",
            ])}
            title={
              linkedItemOpenBehavior === "new"
                ? "Open note in new tab"
                : "Open note"
            }
            aria-label={
              linkedItemOpenBehavior === "new"
                ? "Open note in new tab"
                : "Open note"
            }
          >
            <span>Open</span>
            <ArrowUpRightIcon size={12} />
          </button>
        </div>
      </div>
    </div>
  );
});
