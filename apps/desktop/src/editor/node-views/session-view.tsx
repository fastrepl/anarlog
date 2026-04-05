import {
  type NodeViewComponentProps,
  useEditorEventCallback,
} from "@handlewithcare/react-prosemirror";
import { format } from "date-fns";
import { ArrowUpRightIcon } from "lucide-react";
import type { NodeSpec } from "prosemirror-model";
import { forwardRef, type ReactNode, useCallback, useMemo } from "react";

import { cn, safeParseDate } from "@hypr/utils";

import { TaskCheckbox } from "./task-checkbox";

import { useLinkedItemOpenBehavior } from "~/editor/session/linked-item-open-behavior";
import { getSessionEvent } from "~/session/utils";
import * as main from "~/store/tinybase/store/main";
import { useTabs } from "~/store/zustand/tabs";
import { useListener } from "~/stt/contexts";

export const sessionNodeSpec: NodeSpec = {
  group: "block",
  content: "text*",
  marks: "",
  selectable: false,
  attrs: {
    sessionId: { default: null },
    checked: { default: null },
  },
  parseDOM: [
    {
      tag: 'div[data-type="session"]',
      getAttrs(dom) {
        const el = dom as HTMLElement;
        const checked = el.getAttribute("data-checked");

        return {
          sessionId: el.getAttribute("data-session-id"),
          checked:
            checked === "true" ? true : checked === "false" ? false : null,
        };
      },
    },
  ],
  toDOM(node) {
    return [
      "div",
      {
        "data-type": "session",
        "data-session-id": node.attrs.sessionId,
        "data-checked":
          typeof node.attrs.checked === "boolean"
            ? String(node.attrs.checked)
            : undefined,
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
  const checked =
    typeof node.attrs.checked === "boolean"
      ? node.attrs.checked
      : derivedChecked;

  const handleToggle = useEditorEventCallback((view) => {
    if (!view) return;
    const pos = getPos();
    const tr = view.state.tr.setNodeMarkup(pos, undefined, {
      ...node.attrs,
      checked: !checked,
    });
    view.dispatch(tr);
  });

  return (
    <div
      ref={ref}
      {...htmlAttrs}
      data-checked={
        typeof node.attrs.checked === "boolean"
          ? String(node.attrs.checked)
          : undefined
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
          <TaskCheckbox
            checked={checked}
            isInteractive
            onToggle={handleToggle}
          />
        )}
        <span
          data-session-title
          className={cn([
            "min-w-0 flex-1 cursor-text truncate text-sm text-neutral-900",
            "rounded-sm outline-none focus:bg-white/80",
            checked && "line-through opacity-60",
          ])}
        >
          {children}
        </span>
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
