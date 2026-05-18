import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

const hoisted = vi.hoisted(() => {
  const transaction = {
    setNodeMarkup: vi.fn(),
  };
  const view = {
    state: { tr: transaction },
    dispatch: vi.fn(),
  };

  return { transaction, view };
});

vi.mock("@handlewithcare/react-prosemirror", () => ({
  useEditorEventCallback:
    (callback: (view: typeof hoisted.view) => void) => () =>
      callback(hoisted.view),
  useEditorState: () => ({
    selection: { from: 0, to: 0 },
  }),
}));

import { TaskItemView } from "./task-item-view";

describe("TaskItemView", () => {
  it("toggles the task status when the checkbox is clicked", () => {
    hoisted.transaction.setNodeMarkup.mockImplementation(
      (_pos, _type, attrs) => ({ attrs }),
    );
    hoisted.view.dispatch.mockClear();

    render(
      <TaskItemView
        nodeProps={
          {
            node: {
              attrs: {
                status: "todo",
                checked: false,
                taskId: null,
                taskItemId: null,
              },
              nodeSize: 2,
            },
            getPos: () => 4,
          } as any
        }
      >
        <p>All hands</p>
      </TaskItemView>,
    );

    fireEvent.click(screen.getByRole("checkbox"));

    expect(hoisted.transaction.setNodeMarkup).toHaveBeenCalledWith(
      4,
      undefined,
      {
        status: "done",
        checked: true,
        taskId: null,
        taskItemId: null,
      },
    );
    expect(hoisted.view.dispatch).toHaveBeenCalledWith({
      attrs: {
        status: "done",
        checked: true,
        taskId: null,
        taskItemId: null,
      },
    });
  });
});
