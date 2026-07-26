import { EditorState } from "prosemirror-state";
import { describe, expect, it } from "vitest";

import { schema } from "../note/schema";
import { taskIdentityPlugin } from "./task-identity";

describe("taskIdentityPlugin", () => {
  it("repairs duplicate task identities after unrelated document edits", () => {
    const duplicateAttrs = {
      status: "todo",
      checked: false,
      taskId: "task-1",
      taskItemId: "task-item-1",
    };
    let state = EditorState.create({
      schema,
      doc: schema.node("doc", null, [
        schema.node("paragraph", null, [schema.text("intro")]),
        schema.node("taskList", null, [
          schema.node("taskItem", duplicateAttrs, [schema.node("paragraph")]),
          schema.node("taskItem", duplicateAttrs, [schema.node("paragraph")]),
        ]),
      ]),
      plugins: [taskIdentityPlugin()],
    });

    state = state.applyTransaction(state.tr.insertText("!", 1)).state;

    const taskIds: string[] = [];
    const taskItemIds: string[] = [];
    state.doc.descendants((node) => {
      if (node.type.name !== "taskItem") {
        return true;
      }

      taskIds.push(node.attrs.taskId);
      taskItemIds.push(node.attrs.taskItemId);
      return false;
    });

    expect(new Set(taskIds).size).toBe(2);
    expect(new Set(taskItemIds).size).toBe(2);
  });
});
