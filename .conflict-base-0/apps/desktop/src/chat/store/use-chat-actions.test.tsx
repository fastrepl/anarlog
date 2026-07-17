import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  createChatGroupWithMessage: vi.fn(),
  ids: [] as string[],
  setChatGroupTitleIfCurrent: vi.fn(),
  upsertChatMessage: vi.fn(),
}));

vi.mock("~/ai/hooks", () => ({
  useLanguageModel: () => undefined,
}));

vi.mock("~/chat/store/queries", () => ({
  createChatGroupWithMessage: mocks.createChatGroupWithMessage,
  setChatGroupTitleIfCurrent: mocks.setChatGroupTitleIfCurrent,
  upsertChatMessage: mocks.upsertChatMessage,
}));

vi.mock("~/shared/utils", () => ({
  id: () => mocks.ids.shift(),
}));

vi.mock("~/shared/owner-user", () => ({
  useOwnerUserId: () => "user-1",
}));

import { useChatActions } from "./use-chat-actions";

describe("useChatActions", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.ids = ["message-1", "group-1"];
    mocks.createChatGroupWithMessage.mockResolvedValue(undefined);
    mocks.setChatGroupTitleIfCurrent.mockResolvedValue(undefined);
    mocks.upsertChatMessage.mockResolvedValue(undefined);
  });

  it("durably commits the first group and message before sending", async () => {
    let finishPersistence: (() => void) | undefined;
    mocks.createChatGroupWithMessage.mockReturnValue(
      new Promise<void>((resolve) => {
        finishPersistence = resolve;
      }),
    );
    const onGroupCreated = vi.fn();
    const sendMessage = vi.fn();
    const { result } = renderHook(() =>
      useChatActions({ groupId: undefined, onGroupCreated }),
    );

    act(() => {
      result.current.handleSendMessage(
        "Hello",
        [{ type: "text", text: "Hello" }],
        sendMessage,
      );
    });

    expect(mocks.createChatGroupWithMessage).toHaveBeenCalledWith(
      expect.objectContaining({
        groupId: "group-1",
        ownerUserId: "user-1",
        title: "Hello",
        message: expect.objectContaining({
          id: "message-1",
          chatGroupId: "group-1",
          content: "Hello",
        }),
      }),
    );
    expect(onGroupCreated).not.toHaveBeenCalled();
    expect(sendMessage).not.toHaveBeenCalled();

    await act(async () => finishPersistence?.());

    expect(onGroupCreated).toHaveBeenCalledWith("group-1");
    expect(sendMessage).toHaveBeenCalledWith(
      expect.objectContaining({ id: "message-1", role: "user" }),
      { chatGroupId: "group-1" },
    );
  });

  it("upserts into an existing group before sending", async () => {
    const sendMessage = vi.fn();
    const { result } = renderHook(() =>
      useChatActions({ groupId: "group-existing", onGroupCreated: vi.fn() }),
    );

    act(() => {
      result.current.handleSendMessage(
        "Follow up",
        [{ type: "text", text: "Follow up" }],
        sendMessage,
      );
    });

    await waitFor(() => expect(sendMessage).toHaveBeenCalledOnce());
    expect(mocks.createChatGroupWithMessage).not.toHaveBeenCalled();
    expect(mocks.upsertChatMessage).toHaveBeenCalledWith(
      expect.objectContaining({
        id: "message-1",
        chatGroupId: "group-existing",
      }),
    );
  });

  it("does not send a request when durable persistence fails", async () => {
    const error = new Error("database unavailable");
    mocks.createChatGroupWithMessage.mockRejectedValue(error);
    const consoleError = vi
      .spyOn(console, "error")
      .mockImplementation(() => {});
    const sendMessage = vi.fn();
    const { result } = renderHook(() =>
      useChatActions({ groupId: undefined, onGroupCreated: vi.fn() }),
    );

    act(() => {
      result.current.handleSendMessage(
        "Hello",
        [{ type: "text", text: "Hello" }],
        sendMessage,
      );
    });

    await waitFor(() =>
      expect(consoleError).toHaveBeenCalledWith(
        "Failed to persist outgoing chat message",
        error,
      ),
    );
    expect(sendMessage).not.toHaveBeenCalled();
    consoleError.mockRestore();
  });
});
