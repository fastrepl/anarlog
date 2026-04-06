import type { TaskStatus } from "@hypr/store";

import type { JSONContent } from "./session";

import { id } from "~/shared/utils";

export interface TaskSource {
  type: string;
  id: string;
}

export type { TaskStatus } from "@hypr/store";

export interface TaskRecord {
  taskId: string;
  sourceId: string;
  sourceType: string;
  sourceOrder: number;
  status: TaskStatus;
  textPreview: string;
  body: JSONContent[];
  dueDate?: string;
}

export function createTaskId(): string {
  return id();
}

export function createTaskItemId(): string {
  return id();
}

export const DEFAULT_TASK_STATUS: TaskStatus = "todo";

export function isTaskStatus(value: unknown): value is TaskStatus {
  return value === "todo" || value === "in_progress" || value === "done";
}

export function normalizeTaskStatus(
  status: unknown,
  checked?: unknown,
  fallback: TaskStatus = DEFAULT_TASK_STATUS,
): TaskStatus {
  if (status === true) {
    return "done";
  }

  if (status === false) {
    return "todo";
  }

  if (isTaskStatus(status)) {
    return status;
  }

  if (checked === true) {
    return "done";
  }

  if (checked === false) {
    return "todo";
  }

  return fallback;
}

export function getOptionalTaskStatus(
  status: unknown,
  checked?: unknown,
): TaskStatus | null {
  if (status === true) {
    return "done";
  }

  if (status === false) {
    return "todo";
  }

  if (isTaskStatus(status)) {
    return status;
  }

  if (checked === true) {
    return "done";
  }

  if (checked === false) {
    return "todo";
  }

  return null;
}

export function createTaskStatusAttrs(status: TaskStatus) {
  return { status, checked: status === "done" };
}

export function createTaskItemAttrs(
  status: TaskStatus | boolean = DEFAULT_TASK_STATUS,
  taskId = createTaskId(),
  taskItemId = createTaskItemId(),
) {
  const normalizedStatus = normalizeTaskStatus(status);
  return { ...createTaskStatusAttrs(normalizedStatus), taskId, taskItemId };
}

export function getNextTaskStatus(status: TaskStatus): TaskStatus {
  switch (status) {
    case "todo":
      return "in_progress";
    case "in_progress":
      return "done";
    case "done":
      return "todo";
  }
}

export function isTaskDone(status: TaskStatus): boolean {
  return status === "done";
}

export function createTaskSourceKey(source: TaskSource): string {
  return `${source.type}:${source.id}`;
}

export function normalizeTaskContent(
  content: JSONContent | undefined,
): JSONContent | undefined {
  if (!content) {
    return content;
  }

  return normalizeNode(content, new Set<string>(), new Set<string>()).node;
}

export function extractTasksFromContent(
  content: JSONContent,
  source: TaskSource,
  previousTasks: ReadonlyMap<string, TaskRecord> = new Map(),
): TaskRecord[] {
  const tasks: TaskRecord[] = [];

  walkContent(content, (node) => {
    if (node.type !== "taskItem") {
      return;
    }

    const taskId =
      typeof node.attrs?.taskId === "string" && node.attrs.taskId.trim()
        ? node.attrs.taskId
        : null;
    if (!taskId) {
      return;
    }

    const previousTask = previousTasks.get(taskId);
    tasks.push({
      taskId,
      sourceId: source.id,
      sourceType: source.type,
      sourceOrder: tasks.length,
      status: normalizeTaskStatus(node.attrs?.status, node.attrs?.checked),
      textPreview: getTaskItemTextContent(node),
      body: cloneContentArray(node.content),
      dueDate: previousTask?.dueDate,
    });
  });

  return tasks;
}

export function createTaskItemNode(task: TaskRecord): JSONContent {
  return {
    type: "taskItem",
    attrs: createTaskItemAttrs(task.status, task.taskId),
    content: cloneContentArray(task.body),
  };
}

export function hydrateTaskContent(args: {
  content: JSONContent;
  sourceTasks: TaskRecord[];
  getTask: (taskId: string) => TaskRecord | null;
}): JSONContent {
  const sourceTasksById = new Map(
    args.sourceTasks.map((task) => [task.taskId, task]),
  );
  const usedTaskIds = new Set<string>();
  const hydratedContent = hydrateNodeContent(
    args.content,
    sourceTasksById,
    usedTaskIds,
    args.getTask,
  );
  const missingTasks = args.sourceTasks.filter(
    (task) => !usedTaskIds.has(task.taskId),
  );

  if (missingTasks.length === 0) {
    return hydratedContent;
  }

  return appendTaskItems(
    hydratedContent,
    missingTasks.map((task) => createTaskItemNode(task)),
  );
}

export function moveOpenTasksBetweenContents(args: {
  previousContent: JSONContent;
  currentContent: JSONContent;
  previousTasks: TaskRecord[];
  currentTasks: TaskRecord[];
  currentSource: TaskSource;
}): {
  previousContent: JSONContent;
  currentContent: JSONContent;
  previousTasks: TaskRecord[];
  currentTasks: TaskRecord[];
  movedTasks: TaskRecord[];
} | null {
  const currentTaskIds = new Set(args.currentTasks.map((task) => task.taskId));
  const tasksToMove = args.previousTasks.filter(
    (task) => !isTaskDone(task.status) && !currentTaskIds.has(task.taskId),
  );

  if (tasksToMove.length === 0) {
    return null;
  }

  const movedTaskIds = new Set(tasksToMove.map((task) => task.taskId));
  const movedTasks = tasksToMove.map((task, index) => ({
    ...task,
    sourceId: args.currentSource.id,
    sourceType: args.currentSource.type,
    sourceOrder: args.currentTasks.length + index,
  }));

  return {
    previousContent: removeTaskItems(args.previousContent, movedTaskIds),
    currentContent: appendTaskItems(
      args.currentContent,
      movedTasks.map((task) => createTaskItemNode(task)),
    ),
    previousTasks: args.previousTasks.filter(
      (task) => !movedTaskIds.has(task.taskId),
    ),
    currentTasks: [...args.currentTasks, ...movedTasks],
    movedTasks,
  };
}

function hydrateNodeContent(
  node: JSONContent,
  sourceTasksById: ReadonlyMap<string, TaskRecord>,
  usedTaskIds: Set<string>,
  getTask: (taskId: string) => TaskRecord | null,
): JSONContent {
  const nextNode = hydrateNode(node, sourceTasksById, usedTaskIds, getTask);

  if (nextNode) {
    return nextNode;
  }

  return { type: "doc", content: [{ type: "paragraph" }] };
}

function hydrateNode(
  node: JSONContent,
  sourceTasksById: ReadonlyMap<string, TaskRecord>,
  usedTaskIds: Set<string>,
  getTask: (taskId: string) => TaskRecord | null,
): JSONContent | null {
  if (node.type === "taskItem") {
    const taskId =
      typeof node.attrs?.taskId === "string" && node.attrs.taskId.trim()
        ? node.attrs.taskId
        : null;

    if (!taskId) {
      return node;
    }

    const sourceTask = sourceTasksById.get(taskId);
    if (sourceTask) {
      usedTaskIds.add(taskId);
      return createTaskItemNode(sourceTask);
    }

    if (getTask(taskId)) {
      return null;
    }

    return node;
  }

  if (!node.content?.length) {
    return node;
  }

  const nextContent = node.content
    .map((child) => hydrateNode(child, sourceTasksById, usedTaskIds, getTask))
    .filter((child): child is JSONContent => child !== null);
  const changed = nextContent.some(
    (child, index) => child !== node.content?.[index],
  );

  if (node.type === "taskList" && nextContent.length === 0) {
    return null;
  }

  if (!changed && nextContent.length === node.content.length) {
    return node;
  }

  return {
    ...node,
    content: nextContent,
  };
}

function removeTaskItems(content: JSONContent, taskIds: ReadonlySet<string>) {
  const nextContent = removeTaskNodes(content, taskIds);
  if (nextContent) {
    return nextContent;
  }

  return { type: "doc", content: [{ type: "paragraph" }] };
}

function removeTaskNodes(
  node: JSONContent,
  taskIds: ReadonlySet<string>,
): JSONContent | null {
  if (node.type === "taskItem") {
    const taskId = node.attrs?.taskId;
    if (typeof taskId === "string" && taskIds.has(taskId)) {
      return null;
    }

    return node;
  }

  if (!node.content?.length) {
    return node;
  }

  const nextContent = node.content
    .map((child) => removeTaskNodes(child, taskIds))
    .filter((child): child is JSONContent => child !== null);
  const changed = nextContent.some(
    (child, index) => child !== node.content?.[index],
  );

  if (node.type === "taskList" && nextContent.length === 0) {
    return null;
  }

  if (!changed && nextContent.length === node.content.length) {
    return node;
  }

  return {
    ...node,
    content: nextContent,
  };
}

function normalizeNode(
  node: JSONContent,
  seenTaskIds: Set<string>,
  seenTaskItemIds: Set<string>,
): {
  node: JSONContent;
  changed: boolean;
} {
  let changed = false;
  let nextAttrs = node.attrs;
  let nextContent = node.content;

  if (node.type === "taskItem") {
    let nextTaskId =
      typeof node.attrs?.taskId === "string" && node.attrs.taskId.trim()
        ? node.attrs.taskId
        : "";
    let nextTaskItemId =
      typeof node.attrs?.taskItemId === "string" && node.attrs.taskItemId.trim()
        ? node.attrs.taskItemId
        : "";

    while (!nextTaskId || seenTaskIds.has(nextTaskId)) {
      nextTaskId = createTaskId();
    }

    while (!nextTaskItemId || seenTaskItemIds.has(nextTaskItemId)) {
      nextTaskItemId = createTaskItemId();
    }

    seenTaskIds.add(nextTaskId);
    seenTaskItemIds.add(nextTaskItemId);

    if (
      node.attrs?.taskId !== nextTaskId ||
      node.attrs?.taskItemId !== nextTaskItemId
    ) {
      nextAttrs = {
        ...(node.attrs ?? {}),
        taskId: nextTaskId,
        taskItemId: nextTaskItemId,
      };
      changed = true;
    }
  }

  if (node.content?.length) {
    const normalizedChildren = node.content.map((child) =>
      normalizeNode(child, seenTaskIds, seenTaskItemIds),
    );
    if (normalizedChildren.some((child) => child.changed)) {
      nextContent = normalizedChildren.map((child) => child.node);
      changed = true;
    }
  }

  if (!changed) {
    return { node, changed: false };
  }

  return {
    node: {
      ...node,
      ...(nextAttrs ? { attrs: nextAttrs } : {}),
      ...(nextContent ? { content: nextContent } : {}),
    },
    changed: true,
  };
}

function walkContent(
  node: JSONContent | undefined,
  visit: (node: JSONContent) => void,
) {
  if (!node) {
    return;
  }

  visit(node);

  for (const child of node.content ?? []) {
    walkContent(child, visit);
  }
}

function getNodeTextContent(node: JSONContent | undefined): string {
  if (!node) {
    return "";
  }

  if (typeof node.text === "string") {
    return node.text;
  }

  return (node.content ?? [])
    .map((child) => getNodeTextContent(child))
    .join(" ")
    .replace(/\s+/g, " ")
    .trim();
}

function getTaskItemTextContent(node: JSONContent): string {
  const paragraph = node.content?.find((child) => child.type === "paragraph");
  return getNodeTextContent(paragraph);
}

function appendTaskItems(
  content: JSONContent,
  taskItems: JSONContent[],
): JSONContent {
  const nextContent = [...(content.content ?? [])];
  const lastIndex = nextContent.length - 1;
  const lastNode = nextContent[lastIndex];

  if (lastNode?.type === "taskList") {
    nextContent[lastIndex] = {
      ...lastNode,
      content: [...(lastNode.content ?? []), ...taskItems],
    };
  } else {
    nextContent.push({
      type: "taskList",
      content: taskItems,
    });
  }

  return {
    ...content,
    content: nextContent,
  };
}

function cloneContentArray(content: JSONContent[] | undefined): JSONContent[] {
  return (
    content?.map((node) => structuredClone(node)) ?? [{ type: "paragraph" }]
  );
}
