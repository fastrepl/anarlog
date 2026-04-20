import { Plugin } from "prosemirror-state";

import { createTaskId, createTaskItemId } from "../tasks";

export function taskIdentityPlugin() {
  return new Plugin({
    appendTransaction(transactions, _oldState, newState) {
      if (!transactions.some((transaction) => transaction.docChanged)) {
        return null;
      }

      const seenTaskIds = new Set<string>();
      const seenTaskItemIds = new Set<string>();
      const updates: {
        pos: number;
        taskId: string;
        taskItemId: string;
      }[] = [];

      newState.doc.descendants((node, pos) => {
        if (node.type.name !== "taskItem") {
          return;
        }

        let taskId =
          typeof node.attrs.taskId === "string" && node.attrs.taskId.trim()
            ? node.attrs.taskId
            : "";

        while (!taskId || seenTaskIds.has(taskId)) {
          taskId = createTaskId();
        }

        let taskItemId =
          typeof node.attrs.taskItemId === "string" &&
          node.attrs.taskItemId.trim()
            ? node.attrs.taskItemId
            : "";

        while (!taskItemId || seenTaskItemIds.has(taskItemId)) {
          taskItemId = createTaskItemId();
        }

        seenTaskIds.add(taskId);
        seenTaskItemIds.add(taskItemId);

        if (
          node.attrs.taskId !== taskId ||
          node.attrs.taskItemId !== taskItemId
        ) {
          updates.push({ pos, taskId, taskItemId });
        }
      });

      if (updates.length === 0) {
        return null;
      }

      let tr = newState.tr;
      updates.forEach(({ pos, taskId, taskItemId }) => {
        const node = tr.doc.nodeAt(pos);
        if (!node) {
          return;
        }

        tr = tr.setNodeMarkup(
          pos,
          undefined,
          { ...node.attrs, taskId, taskItemId },
          node.marks,
        );
      });

      return tr;
    },
  });
}
