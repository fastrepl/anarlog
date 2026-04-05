import { Plugin } from "prosemirror-state";

import { createTaskId } from "../tasks";

export function taskIdentityPlugin() {
  return new Plugin({
    appendTransaction(transactions, _oldState, newState) {
      if (!transactions.some((transaction) => transaction.docChanged)) {
        return null;
      }

      const seenTaskIds = new Set<string>();
      const updates: { pos: number; taskId: string }[] = [];

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

        seenTaskIds.add(taskId);

        if (node.attrs.taskId !== taskId) {
          updates.push({ pos, taskId });
        }
      });

      if (updates.length === 0) {
        return null;
      }

      let tr = newState.tr;
      updates.forEach(({ pos, taskId }) => {
        const node = tr.doc.nodeAt(pos);
        if (!node) {
          return;
        }

        tr = tr.setNodeMarkup(
          pos,
          undefined,
          { ...node.attrs, taskId },
          node.marks,
        );
      });

      return tr;
    },
  });
}
