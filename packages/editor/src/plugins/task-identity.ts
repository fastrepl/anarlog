import type { Node as PMNode } from "prosemirror-model";
import { Plugin, PluginKey } from "prosemirror-state";

import { createTaskId, createTaskItemId } from "../tasks";
import { hasChangedNodeOfType } from "./changed-ranges";

const taskIdentityPluginKey = new PluginKey<boolean>("taskIdentity");

export function taskIdentityPlugin() {
  return new Plugin<boolean>({
    key: taskIdentityPluginKey,
    // Documents arrive from storage, paste and import, so the whole doc has to
    // be swept once for missing/duplicate ids. Editing can only introduce them
    // inside a changed range, so the sweep runs on the first edit after a state
    // is created and never again -- otherwise every keystroke walks every node.
    state: {
      init: () => false,
      apply: (transaction, swept) => swept || transaction.docChanged,
    },
    appendTransaction(transactions, oldState, newState) {
      if (!transactions.some((transaction) => transaction.docChanged)) {
        return null;
      }
      if (
        !hasChangedNodeOfType(newState.doc, transactions, "taskItem") &&
        (taskIdentityPluginKey.getState(oldState) === true ||
          !hasInvalidTaskIdentity(newState.doc))
      ) {
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

function hasInvalidTaskIdentity(doc: PMNode) {
  const seenTaskIds = new Set<string>();
  const seenTaskItemIds = new Set<string>();
  let invalid = false;

  doc.descendants((node) => {
    if (invalid || node.type.name !== "taskItem") {
      return !invalid;
    }

    const taskId =
      typeof node.attrs.taskId === "string" && node.attrs.taskId.trim()
        ? node.attrs.taskId
        : "";
    const taskItemId =
      typeof node.attrs.taskItemId === "string" && node.attrs.taskItemId.trim()
        ? node.attrs.taskItemId
        : "";

    invalid =
      !taskId ||
      !taskItemId ||
      seenTaskIds.has(taskId) ||
      seenTaskItemIds.has(taskItemId);
    seenTaskIds.add(taskId);
    seenTaskItemIds.add(taskItemId);

    return !invalid;
  });

  return invalid;
}
