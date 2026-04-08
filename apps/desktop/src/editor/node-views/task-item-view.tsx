import {
  type NodeViewComponentProps,
  useEditorEventCallback,
  useEditorState,
} from "@handlewithcare/react-prosemirror";
import type { NodeSpec } from "prosemirror-model";
import { forwardRef, type ReactNode, useMemo, useState } from "react";

import { cn, format, parseISO } from "@hypr/utils";

import { useTaskSourceOptional } from "../task-source";
import { useTaskRecord, useTaskStorageOptional } from "../task-storage";
import {
  createTaskStatusAttrs,
  getNextTaskStatus,
  normalizeTaskStatus,
} from "../tasks";
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
  const taskSource = useTaskSourceOptional();
  const taskStorage = useTaskStorageOptional();
  const taskRecord = useTaskRecord(taskSource, taskId);
  const dueDate = taskRecord?.dueDate ?? "";
  const [isEditingDueDate, setIsEditingDueDate] = useState(false);

  const pos = getPos();
  const { selection } = useEditorState();
  const isSelected =
    pos >= selection.from && pos + node.nodeSize <= selection.to - 1;
  const showDueDateInput = isEditingDueDate || isSelected;
  const formattedDueDate = useMemo(() => {
    if (!dueDate) {
      return "";
    }

    try {
      return format(parseISO(`${dueDate}T00:00:00`), "MMM d");
    } catch {
      return dueDate;
    }
  }, [dueDate]);

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

  const handleDueDateChange = (value: string) => {
    if (!taskSource || !taskId || !taskStorage) {
      return;
    }

    const sourceTasks = taskStorage.getTasksForSource(taskSource);
    taskStorage.upsertTasksForSource(
      taskSource,
      sourceTasks.map((task) =>
        task.taskId === taskId
          ? { ...task, dueDate: value || undefined }
          : task,
      ),
    );
  };

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
      <div className="flex min-w-0 flex-1 flex-wrap items-start gap-2">
        <div className="min-w-0 flex-1">{children}</div>
        {taskSource && taskId ? (
          <div contentEditable={false} suppressContentEditableWarning>
            {showDueDateInput ? (
              <input
                type="date"
                value={dueDate}
                onChange={(event) => handleDueDateChange(event.target.value)}
                onBlur={() => setIsEditingDueDate(false)}
                onMouseDown={(event) => event.stopPropagation()}
                className={cn([
                  "rounded border border-neutral-200 bg-transparent px-2 py-1 text-xs text-neutral-600 transition outline-none",
                  "focus:border-neutral-400",
                ])}
              />
            ) : (
              <button
                type="button"
                onClick={() => setIsEditingDueDate(true)}
                onMouseDown={(event) => event.stopPropagation()}
                className={cn([
                  "rounded-full border border-neutral-200 px-2 py-1 text-xs text-neutral-600 transition hover:border-neutral-300 hover:text-neutral-800",
                ])}
              >
                {formattedDueDate || "Due"}
              </button>
            )}
          </div>
        ) : null}
      </div>
    </li>
  );
});
