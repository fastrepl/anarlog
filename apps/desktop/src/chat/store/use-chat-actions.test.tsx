import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  createChatGroupWithMessage: vi.fn(),
  ids: [] as string[],
  setChatGroupTitleIfCurrent: vi.fn(),
  toastError: vi.fn(),
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

vi.mock("@hypr/ui/components/ui/toast", () => ({
  sonnerToast: { error: mocks.toastError },
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

  it("sends immediately and persists the first group in the background", () => {
    mocks.createChatGroupWithMessage.mockReturnValue(
      new Promise<void>(() => {}),
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

    expect(sendMessage).toHaveBeenCalledWith(
      expect.objectContaining({ id: "message-1", role: "user" }),
      { chatGroupId: "group-1" },
    );
    expect(onGroupCreated).toHaveBeenCalledWith("group-1");
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
  });

  it("sends immediately when upserting into an existing group", () => {
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

    expect(sendMessage).toHaveBeenCalledOnce();
    expect(mocks.createChatGroupWithMessage).not.toHaveBeenCalled();
    expect(mocks.upsertChatMessage).toHaveBeenCalledWith(
      expect.objectContaining({
        id: "message-1",
        chatGroupId: "group-existing",
      }),
    );
  });

  it("still sends and surfaces an error toast when persistence fails", async () => {
    const error = new Error("database unavailable");
    mocks.createChatGroupWithMessage.mockRejectedValue(error);
    const consoleError = vi
      .spyOn(console, "error")
      .mockImplementation(() => {});
    const sendMessage = vi.fn();
    const onGroupCreateFailed = vi.fn();
    const { result } = renderHook(() =>
      useChatActions({
        groupId: undefined,
        onGroupCreated: vi.fn(),
        onGroupCreateFailed,
      }),
    );

    act(() => {
      result.current.handleSendMessage(
        "Hello",
        [{ type: "text", text: "Hello" }],
        sendMessage,
      );
    });

    expect(sendMessage).toHaveBeenCalledOnce();
    await waitFor(
      () =>
        expect(consoleError).toHaveBeenCalledWith(
          "Failed to persist outgoing chat message",
          error,
        ),
      { timeout: 3000 },
    );
    expect(mocks.toastError).toHaveBeenCalledWith(
      "Could not save this chat message.",
    );
    expect(mocks.createChatGroupWithMessage).toHaveBeenCalledTimes(3);
    expect(onGroupCreateFailed).toHaveBeenCalledWith("group-1");
    consoleError.mockRestore();
  });

  it("retries a transient persist failure without failing the group", async () => {
    mocks.createChatGroupWithMessage
      .mockRejectedValueOnce(new Error("database is locked"))
      .mockResolvedValueOnce(undefined);
    const consoleError = vi
      .spyOn(console, "error")
      .mockImplementation(() => {});
    const onGroupCreateFailed = vi.fn();
    const { result } = renderHook(() =>
      useChatActions({
        groupId: undefined,
        onGroupCreated: vi.fn(),
        onGroupCreateFailed,
      }),
    );

    act(() => {
      result.current.handleSendMessage(
        "Hello",
        [{ type: "text", text: "Hello" }],
        vi.fn(),
      );
    });

    await waitFor(
      () => expect(mocks.createChatGroupWithMessage).toHaveBeenCalledTimes(2),
      { timeout: 3000 },
    );
    expect(mocks.toastError).not.toHaveBeenCalled();
    expect(onGroupCreateFailed).not.toHaveBeenCalled();
    consoleError.mockRestore();
  });
});
