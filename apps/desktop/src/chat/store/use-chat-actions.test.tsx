import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  createChatGroupWithMessage: vi.fn(),
  generateChatTitle: vi.fn(),
  ids: [] as string[],
  setChatGroupTitleIfCurrent: vi.fn(),
  titleModel: undefined as unknown,
  toastError: vi.fn(),
  upsertChatMessage: vi.fn(),
}));

vi.mock("~/ai/hooks", () => ({
  useLanguageModel: () => mocks.titleModel,
}));

vi.mock("./chat-title", async (importOriginal) => ({
  ...(await importOriginal<typeof import("./chat-title")>()),
  generateChatTitle: mocks.generateChatTitle,
}));

vi.mock("~/chat/store/queries", () => ({
  createChatGroupWithMessage: mocks.createChatGroupWithMessage,
  setChatGroupTitleIfCurrent: mocks.setChatGroupTitleIfCurrent,
  upsertChatMessage: mocks.upsertChatMessage,
}));

vi.mock("@anlg/ui/components/ui/toast", () => ({
  sonnerToast: { error: mocks.toastError },
}));

vi.mock("~/shared/utils", () => ({
  id: () => mocks.ids.shift(),
}));

vi.mock("~/shared/owner-user", () => ({
  useOwnerUserId: () => "user-1",
}));

import {
  clearFailedChatGroupCreate,
  isFailedChatGroupCreate,
} from "./pending-persists";
import { useChatActions } from "./use-chat-actions";

import type { ChatSendOptions } from "~/chat/types";

describe("useChatActions", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.ids = ["message-1", "group-1"];
    mocks.titleModel = undefined;
    mocks.createChatGroupWithMessage.mockResolvedValue(undefined);
    mocks.generateChatTitle.mockResolvedValue("Generated title");
    mocks.setChatGroupTitleIfCurrent.mockResolvedValue(undefined);
    mocks.upsertChatMessage.mockResolvedValue(undefined);
    clearFailedChatGroupCreate("group-1");
  });

  it("queues first-group persistence until the transport preflight", async () => {
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
      {
        chatGroupId: "group-1",
        beforeSend: expect.any(Function),
      },
    );
    expect(mocks.createChatGroupWithMessage).not.toHaveBeenCalled();
    expect(onGroupCreated).not.toHaveBeenCalled();

    const options = sendMessage.mock.calls[0]?.[1] as ChatSendOptions;
    await options.beforeSend?.(vi.fn());

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
    expect(onGroupCreated).toHaveBeenCalledWith("group-1");
  });

  it("queues existing-group persistence until the transport preflight", async () => {
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
    expect(mocks.upsertChatMessage).not.toHaveBeenCalled();

    const options = sendMessage.mock.calls[0]?.[1] as ChatSendOptions;
    await options.beforeSend?.(vi.fn());

    expect(mocks.upsertChatMessage).toHaveBeenCalledWith(
      expect.objectContaining({
        id: "message-1",
        chatGroupId: "group-existing",
      }),
    );
  });

  it("rejects the preflight and rolls back when persistence fails", async () => {
    const error = new Error("database unavailable");
    mocks.createChatGroupWithMessage.mockRejectedValue(error);
    const consoleError = vi
      .spyOn(console, "error")
      .mockImplementation(() => {});
    const sendMessage = vi.fn();
    const onGroupCreateFailed = vi.fn();
    const onGroupCreated = vi.fn(() => {
      expect(isFailedChatGroupCreate("group-1")).toBe(false);
    });
    const { result } = renderHook(() =>
      useChatActions({
        groupId: undefined,
        onGroupCreated,
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
    const options = sendMessage.mock.calls[0]?.[1] as ChatSendOptions;
    await expect(options.beforeSend?.(vi.fn())).rejects.toBe(error);
    expect(consoleError).toHaveBeenCalledWith(
      "Failed to persist outgoing chat message",
      error,
    );
    expect(mocks.toastError).toHaveBeenCalledWith(
      "Could not save this chat message.",
    );
    expect(mocks.createChatGroupWithMessage).toHaveBeenCalledTimes(3);
    expect(onGroupCreateFailed).toHaveBeenCalledWith("group-1");
    expect(isFailedChatGroupCreate("group-1")).toBe(true);

    mocks.createChatGroupWithMessage.mockResolvedValue(undefined);
    await options.beforeSend?.(vi.fn());

    expect(mocks.createChatGroupWithMessage).toHaveBeenCalledTimes(4);
    expect(isFailedChatGroupCreate("group-1")).toBe(false);
    expect(onGroupCreated).toHaveBeenCalledWith("group-1");
    expect(onGroupCreateFailed).toHaveBeenCalledOnce();
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
    const sendMessage = vi.fn();
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

    const options = sendMessage.mock.calls[0]?.[1] as ChatSendOptions;
    await options.beforeSend?.(vi.fn());

    expect(mocks.createChatGroupWithMessage).toHaveBeenCalledTimes(2);
    expect(mocks.toastError).not.toHaveBeenCalled();
    expect(onGroupCreateFailed).not.toHaveBeenCalled();
    consoleError.mockRestore();
  });

  it("tracks generated title persistence as part of the chat attempt", async () => {
    let finishTitle: ((title: string) => void) | undefined;
    mocks.titleModel = {};
    mocks.generateChatTitle.mockReturnValueOnce(
      new Promise<string>((resolve) => {
        finishTitle = resolve;
      }),
    );
    const sendMessage = vi.fn();
    const trackCompletion = vi.fn();
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

    const options = sendMessage.mock.calls[0]?.[1] as ChatSendOptions;
    await options.beforeSend?.(trackCompletion);
    expect(trackCompletion).toHaveBeenCalledOnce();
    expect(mocks.setChatGroupTitleIfCurrent).not.toHaveBeenCalled();

    finishTitle?.("Generated title");
    await trackCompletion.mock.calls[0]?.[0];
    expect(mocks.setChatGroupTitleIfCurrent).toHaveBeenCalledWith({
      groupId: "group-1",
      expectedTitle: "Hello",
      title: "Generated title",
    });
  });
});
