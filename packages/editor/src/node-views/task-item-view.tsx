import {
  type NodeViewComponentProps,
  useEditorEventCallback,
  useEditorState,
} from "@handlewithcare/react-prosemirror";
import type { NodeSpec } from "prosemirror-model";
import { forwardRef, type ReactNode } from "react";

import {
  createTaskStatusAttrs,
  getNextTaskStatus,
  normalizeTaskStatus,
} from "../tasks";
import { getSafeNodePos } from "./error-boundary";
import { TaskCheckbox } from "./task-checkbox";

export const taskListNodeSpec: NodeSpec = {
  content: "taskItem+",
  group: "block",
  parseDOM: [{ tag: 'ul[data-type="taskList"]' }],
  toDOM() {
    return ["ul", { "data-type": "taskList", class: "task-list" }, 0];
  },
};

export const taskItemNodeSpec: NodeSpec = {
  content: "paragraph block*",
  defining: true,
  attrs: {
    status: { default: "todo" },
    checked: { default: false },
    taskId: { default: null },
    taskItemId: { default: null },
  },
  parseDOM: [
    {
      tag: 'li[data-type="taskItem"]',
      getAttrs(dom) {
        const element = dom as HTMLElement;
        const status = normalizeTaskStatus(
          element.getAttribute("data-status"),
          element.getAttribute("data-checked") === "true",
        );
        return {
          ...createTaskStatusAttrs(status),
          taskId: element.getAttribute("data-task-id"),
          taskItemId: element.getAttribute("data-task-item-id"),
        };
      },
    },
  ],
  toDOM(node) {
    const status = normalizeTaskStatus(node.attrs.status, node.attrs.checked);
    return [
      "li",
      {
        "data-type": "taskItem",
        "data-status": status,
        "data-checked": status === "done" ? "true" : "false",
        "data-task-id": node.attrs.taskId,
        "data-task-item-id": node.attrs.taskItemId,
      },
      0,
    ];
  },
};

export const TaskItemView = forwardRef<
  HTMLLIElement,
  NodeViewComponentProps & { children?: ReactNode }
>(function TaskItemView({ nodeProps, children, ...htmlAttrs }, ref) {
  const { node, getPos } = nodeProps;
  const status = normalizeTaskStatus(node.attrs.status, node.attrs.checked);
  const taskId = node.attrs.taskId as string | null;

  const pos = getSafeNodePos(getPos);
  const { selection } = useEditorState();
  const isSelected =
    pos !== null &&
    pos >= selection.from &&
    pos + node.nodeSize <= selection.to - 1;

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
    <li
      ref={ref}
      {...htmlAttrs}
      data-type="taskItem"
      data-status={status}
      data-checked={status === "done" ? "true" : "false"}
      data-task-id={taskId ?? undefined}
      data-task-item-id={
        (node.attrs.taskItemId as string | null | undefined) ?? undefined
      }
    >
      <TaskCheckbox
        status={status}
        isInteractive
        isSelected={isSelected}
        onToggle={handleToggle}
      />
      <div ref={nodeProps.contentDOMRef}>{children}</div>
    </li>
  );
});
