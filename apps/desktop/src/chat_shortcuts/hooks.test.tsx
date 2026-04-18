import { renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const { useValuesMock, useCellMock, useSetRowCallbackMock } = vi.hoisted(
  () => ({
    useValuesMock: vi.fn<() => { user_id: string | undefined }>(() => ({
      user_id: undefined,
    })),
    useCellMock: vi.fn(() => undefined),
    useSetRowCallbackMock: vi.fn(() => vi.fn()),
  }),
);

vi.mock("~/store/tinybase/store/main", () => ({
  STORE_ID: "main",
  QUERIES: {
    visibleChatShortcuts: "visibleChatShortcuts",
  },
  UI: {
    useResultTable: vi.fn(() => ({})),
    useCell: useCellMock,
    useValues: useValuesMock,
    useSetRowCallback: useSetRowCallbackMock,
    useSetPartialRowCallback: vi.fn(() => vi.fn()),
    useDelRowCallback: vi.fn(() => vi.fn()),
  },
}));

import { useChatShortcutCell, useCreateChatShortcut } from "./hooks";

describe("chat shortcuts boundary hooks", () => {
  beforeEach(() => {
    useValuesMock.mockReset();
    useCellMock.mockReset();
    useSetRowCallbackMock.mockReset();
  });

  it("normalizes missing shortcut cells to empty string", () => {
    useCellMock.mockReturnValue(undefined);
    const result = renderHook(() => useChatShortcutCell("shortcut-1", "title"));
    expect(result.result.current).toBe("");
  });

  it("returns null when creating shortcut without user id", () => {
    useValuesMock.mockReturnValue({ user_id: undefined });
    const setRow = vi.fn();
    useSetRowCallbackMock.mockReturnValue(setRow);

    const result = renderHook(() => useCreateChatShortcut());
    expect(result.result.current({ title: "t", content: "c" })).toBeNull();
    expect(setRow).not.toHaveBeenCalled();
  });

  it("creates shortcut with generated id and payload", () => {
    useValuesMock.mockReturnValue({ user_id: "user-1" });
    const setRow = vi.fn();
    useSetRowCallbackMock.mockReturnValue(setRow);

    const result = renderHook(() => useCreateChatShortcut());
    const createdId = result.result.current({
      title: "Title",
      content: "Body",
    });

    expect(typeof createdId).toBe("string");
    expect(createdId).not.toBeNull();
    expect(setRow).toHaveBeenCalledWith(
      expect.objectContaining({
        id: createdId,
        user_id: "user-1",
        title: "Title",
        content: "Body",
      }),
    );
  });
});
