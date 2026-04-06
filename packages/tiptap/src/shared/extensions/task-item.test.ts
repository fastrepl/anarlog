// @vitest-environment jsdom

import { Editor } from "@tiptap/core";
import TaskList from "@tiptap/extension-task-list";
import StarterKit from "@tiptap/starter-kit";
import { afterEach, describe, expect, test } from "vitest";

import TaskItem from "./task-item";

const editors: Editor[] = [];

function createEditor() {
  const editor = new Editor({
    extensions: [
      StarterKit.configure({ listKeymap: false }),
      TaskList,
      TaskItem.configure({ nested: true }),
    ],
    content: {
      type: "doc",
      content: [
        {
          type: "taskList",
          content: [
            {
              type: "taskItem",
              attrs: { checked: false },
              content: [
                {
                  type: "paragraph",
                  content: [{ type: "text", text: "one" }],
                },
              ],
            },
          ],
        },
      ],
    },
  });

  editors.push(editor);

  return editor;
}

afterEach(() => {
  while (editors.length > 0) {
    editors.pop()?.destroy();
  }
});

describe("task item node view", () => {
  test("does not mark the checkbox wrapper as contenteditable=false", () => {
    const editor = createEditor();
    const taskItemNode = editor.state.doc.firstChild?.firstChild;

    expect(taskItemNode).not.toBeNull();

    const nodeView = editor.extensionManager.nodeViews.taskItem(
      taskItemNode!,
      {} as any,
      () => 1,
      [] as any,
      {} as any,
    );
    const checkboxWrapper = nodeView.dom.querySelector("label");

    expect(checkboxWrapper).not.toBeNull();
    expect(checkboxWrapper?.getAttribute("contenteditable")).toBeNull();
  });
});
