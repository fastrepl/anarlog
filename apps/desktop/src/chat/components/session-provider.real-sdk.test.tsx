import { cleanup, render, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { beginCloudsyncActivity, endCloudsyncActivity } from "@anlg/plugin-db";

import { ChatSession, type ChatSessionRenderProps } from "./session-provider";

import { CHAT_CLOUDSYNC_RELEASE_DELAY_MS } from "~/chat/store/cloudsync-activity";
import type { PersistedChatMessage } from "~/chat/store/persisted-messages";
import { useChatActions } from "~/chat/store/use-chat-actions";
import type { AnlgUIMessage } from "~/chat/types";

const mocks = vi.hoisted(() => ({
  createChatGroupWithMessage: vi.fn().mockResolvedValue(undefined),
  deleteChatMessage: vi.fn().mockResolvedValue(undefined),
  deleteChatMessagesExcept: vi.fn().mockResolvedValue(undefined),
  flushDatabaseWritesByPrefix: vi.fn().mockResolvedValue(undefined),
  getChatMessageGroupId: vi.fn().mockResolvedValue(null),
  ids: [] as string[],
  onGroupCreateFailed: vi.fn(),
  onGroupCreated: vi.fn(),
  persistedMessages: [] as PersistedChatMessage[],
  replaceChatMessage: vi.fn().mockResolvedValue(undefined),
  sendMessages: vi.fn(),
  setChatGroupTitleIfCurrent: vi.fn().mockResolvedValue(undefined),
  transport: {} as {
    sendMessages: ReturnType<typeof vi.fn>;
    reconnectToStream: ReturnType<typeof vi.fn>;
  },
  upsertChatMessage: vi.fn().mockResolvedValue(undefined),
}));

vi.mock("~/ai/hooks", () => ({
  useLanguageModel: () => undefined,
}));

vi.mock("@anlg/ui/components/ui/toast", () => ({
  sonnerToast: { error: vi.fn() },
}));

vi.mock("~/chat/context/use-chat-context-pipeline", () => ({
  useChatContextPipeline: () => ({
    contextEntities: [],
    pendingRefs: [],
  }),
}));

vi.mock("~/chat/store/queries", () => ({
  createChatGroupWithMessage: mocks.createChatGroupWithMessage,
  deleteChatMessage: mocks.deleteChatMessage,
  deleteChatMessagesExcept: mocks.deleteChatMessagesExcept,
  getChatMessageGroupId: mocks.getChatMessageGroupId,
  replaceChatMessage: mocks.replaceChatMessage,
  setChatGroupTitleIfCurrent: mocks.setChatGroupTitleIfCurrent,
  upsertChatMessage: mocks.upsertChatMessage,
  usePersistedChatMessages: () => mocks.persistedMessages,
}));

vi.mock("~/chat/transport/use-transport", () => ({
  useTransport: () => ({
    transport: mocks.transport,
    isSystemPromptReady: true,
  }),
}));

vi.mock("~/db/write-queue", () => ({
  flushDatabaseWritesByPrefix: mocks.flushDatabaseWritesByPrefix,
}));

vi.mock("~/shared/owner-user", () => ({
  useOwnerUserId: () => "owner-1",
}));

vi.mock("~/shared/utils", async (importOriginal) => ({
  ...(await importOriginal<typeof import("~/shared/utils")>()),
  id: () => mocks.ids.shift(),
}));

function deferredVoid() {
  let resolve!: () => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<void>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, reject, resolve };
}

const userMessage: AnlgUIMessage = {
  id: "user-1",
  role: "user",
  parts: [{ type: "text", text: "Question" }],
};

function renderHarness() {
  let current: ChatSessionRenderProps | undefined;
  const view = render(
    <ChatSession chatGroupId="group-1" sessionId="session-1" unstyled>
      {(props) => {
        current = props;
        return null;
      }}
    </ChatSession>,
  );
  return {
    get props() {
      if (!current) {
        throw new Error("ChatSession did not render");
      }
      return current;
    },
    view,
  };
}

function NewGroupHarness({
  capture,
}: {
  capture: (props: ChatSessionRenderProps, send: () => void) => void;
}) {
  const { handleSendMessage } = useChatActions({
    chatScope: "general",
    groupId: undefined,
    onGroupCreated: mocks.onGroupCreated,
    onGroupCreateFailed: mocks.onGroupCreateFailed,
  });

  return (
    <ChatSession sessionId="session-new" unstyled>
      {(props) => {
        capture(props, () =>
          handleSendMessage(
            "Question",
            [{ type: "text", text: "Question" }],
            props.sendMessage,
          ),
        );
        return null;
      }}
    </ChatSession>
  );
}

function renderNewGroupHarness() {
  let current: ChatSessionRenderProps | undefined;
  let send: (() => void) | undefined;
  const view = render(
    <NewGroupHarness
      capture={(props, sendMessage) => {
        current = props;
        send = sendMessage;
      }}
    />,
  );
  return {
    get props() {
      if (!current) {
        throw new Error("ChatSession did not render");
      }
      return current;
    },
    send() {
      if (!send) {
        throw new Error("ChatSession did not render");
      }
      send();
    },
    view,
  };
}

function startedNativeKey(callIndex: number) {
  return vi.mocked(beginCloudsyncActivity).mock.calls[callIndex]?.[1] as string;
}

describe("ChatSession real SDK CloudSync activity", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(beginCloudsyncActivity).mockReset().mockResolvedValue(undefined);
    vi.mocked(endCloudsyncActivity).mockReset().mockResolvedValue(undefined);
    mocks.createChatGroupWithMessage.mockReset().mockResolvedValue(undefined);
    mocks.deleteChatMessage.mockReset().mockResolvedValue(undefined);
    mocks.deleteChatMessagesExcept.mockReset().mockResolvedValue(undefined);
    mocks.flushDatabaseWritesByPrefix.mockReset().mockResolvedValue(undefined);
    mocks.getChatMessageGroupId.mockReset().mockResolvedValue(null);
    mocks.ids = ["user-new", "group-new"];
    mocks.onGroupCreateFailed.mockReset();
    mocks.onGroupCreated.mockReset();
    mocks.replaceChatMessage.mockReset().mockResolvedValue(undefined);
    mocks.setChatGroupTitleIfCurrent.mockReset().mockResolvedValue(undefined);
    mocks.upsertChatMessage.mockReset().mockResolvedValue(undefined);
    mocks.sendMessages = vi.fn().mockResolvedValue(
      new ReadableStream({
        start: (controller) => controller.close(),
      }),
    );
    mocks.transport = {
      sendMessages: mocks.sendMessages,
      reconnectToStream: vi.fn().mockResolvedValue(null),
    };
    vi.spyOn(console, "error").mockImplementation(() => {});
  });

  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
  });

  it("persists after unmount when deferred acquisition later succeeds", async () => {
    const acquisition = deferredVoid();
    vi.mocked(beginCloudsyncActivity).mockReturnValueOnce(acquisition.promise);
    const persist = vi.fn().mockResolvedValue(undefined);
    const harness = renderHarness();

    harness.props.sendMessage(userMessage, {
      chatGroupId: "group-1",
      beforeSend: persist,
    });
    await waitFor(() => expect(beginCloudsyncActivity).toHaveBeenCalledOnce());

    harness.view.unmount();
    acquisition.resolve();

    await waitFor(() => expect(persist).toHaveBeenCalledOnce());
    await waitFor(() => expect(endCloudsyncActivity).toHaveBeenCalledOnce());
    expect(mocks.sendMessages).not.toHaveBeenCalled();
  });

  it("repairs the user after unmount when initial acquisition fails", async () => {
    const initialAcquisition = deferredVoid();
    const repairAcquisition = deferredVoid();
    vi.mocked(beginCloudsyncActivity)
      .mockReturnValueOnce(initialAcquisition.promise)
      .mockReturnValueOnce(repairAcquisition.promise);
    const persist = vi.fn().mockResolvedValue(undefined);
    const harness = renderHarness();

    harness.props.sendMessage(userMessage, {
      chatGroupId: "group-1",
      beforeSend: persist,
    });
    await waitFor(() => expect(beginCloudsyncActivity).toHaveBeenCalledOnce());

    harness.view.unmount();
    initialAcquisition.reject(new Error("activity drain timed out"));

    await waitFor(() =>
      expect(beginCloudsyncActivity).toHaveBeenCalledTimes(2),
    );
    expect(persist).not.toHaveBeenCalled();
    expect(mocks.upsertChatMessage).not.toHaveBeenCalled();

    repairAcquisition.resolve();

    await waitFor(() => expect(mocks.upsertChatMessage).toHaveBeenCalledOnce());
    await waitFor(() =>
      expect(endCloudsyncActivity).toHaveBeenCalledWith(
        "chat",
        startedNativeKey(1),
      ),
    );
    expect(mocks.upsertChatMessage).toHaveBeenCalledWith(
      expect.objectContaining({ id: userMessage.id }),
    );
    expect(mocks.replaceChatMessage).not.toHaveBeenCalled();
    expect(mocks.deleteChatMessage).not.toHaveBeenCalled();
  });

  it("replays new-group creation under the repair lease before finishing", async () => {
    const repairAcquisition = deferredVoid();
    vi.mocked(beginCloudsyncActivity)
      .mockRejectedValueOnce(new Error("activity drain timed out"))
      .mockReturnValueOnce(repairAcquisition.promise);
    let persistedGroupId: string | null = null;
    mocks.createChatGroupWithMessage.mockImplementationOnce(
      async ({ groupId }: { groupId: string }) => {
        persistedGroupId = groupId;
      },
    );
    mocks.getChatMessageGroupId.mockImplementation(
      async () => persistedGroupId,
    );
    const harness = renderNewGroupHarness();

    harness.send();

    await waitFor(() =>
      expect(beginCloudsyncActivity).toHaveBeenCalledTimes(2),
    );
    expect(mocks.createChatGroupWithMessage).not.toHaveBeenCalled();
    expect(mocks.upsertChatMessage).not.toHaveBeenCalled();
    expect(mocks.deleteChatMessage).not.toHaveBeenCalled();
    expect(mocks.sendMessages).not.toHaveBeenCalled();

    repairAcquisition.resolve();

    await waitFor(() =>
      expect(mocks.createChatGroupWithMessage).toHaveBeenCalledOnce(),
    );
    await waitFor(() => expect(mocks.deleteChatMessage).toHaveBeenCalledOnce());
    await waitFor(() =>
      expect(endCloudsyncActivity).toHaveBeenCalledWith(
        "chat",
        startedNativeKey(1),
      ),
    );
    expect(mocks.createChatGroupWithMessage).toHaveBeenCalledWith(
      expect.objectContaining({
        groupId: "group-new",
        ownerUserId: "owner-1",
        message: expect.objectContaining({
          id: "user-new",
          chatGroupId: "group-new",
        }),
      }),
    );
    expect(mocks.onGroupCreated).toHaveBeenCalledWith("group-new");
    expect(mocks.upsertChatMessage).not.toHaveBeenCalled();
    expect(
      vi.mocked(beginCloudsyncActivity).mock.invocationCallOrder[1],
    ).toBeLessThan(
      mocks.createChatGroupWithMessage.mock.invocationCallOrder[0],
    );
    expect(
      mocks.createChatGroupWithMessage.mock.invocationCallOrder[0],
    ).toBeLessThan(
      vi.mocked(endCloudsyncActivity).mock.invocationCallOrder[
        vi.mocked(endCloudsyncActivity).mock.invocationCallOrder.length - 1
      ] ?? 0,
    );
  });

  it("continues repair when unmounted during fresh acquisition", async () => {
    const repairAcquisition = deferredVoid();
    vi.mocked(beginCloudsyncActivity)
      .mockRejectedValueOnce(new Error("activity drain timed out"))
      .mockReturnValueOnce(repairAcquisition.promise);
    const persist = vi.fn().mockResolvedValue(undefined);
    const harness = renderHarness();

    harness.props.sendMessage(userMessage, {
      chatGroupId: "group-1",
      beforeSend: persist,
    });
    await waitFor(() =>
      expect(beginCloudsyncActivity).toHaveBeenCalledTimes(2),
    );

    harness.view.unmount();
    repairAcquisition.resolve();

    await waitFor(() => expect(mocks.upsertChatMessage).toHaveBeenCalledOnce());
    await waitFor(() =>
      expect(endCloudsyncActivity).toHaveBeenCalledWith(
        "chat",
        startedNativeKey(1),
      ),
    );
    expect(mocks.upsertChatMessage).toHaveBeenCalledWith(
      expect.objectContaining({ id: userMessage.id }),
    );
    expect(mocks.replaceChatMessage).not.toHaveBeenCalled();
  });

  it("reacquires before repairing after activity acquisition fails", async () => {
    const freshAcquisition = deferredVoid();
    vi.mocked(beginCloudsyncActivity)
      .mockRejectedValueOnce(new Error("activity drain timed out"))
      .mockReturnValueOnce(freshAcquisition.promise);
    const persist = vi.fn().mockResolvedValue(undefined);
    const repair = deferredVoid();
    mocks.upsertChatMessage.mockReturnValueOnce(repair.promise);
    const harness = renderHarness();

    harness.props.sendMessage(userMessage, {
      chatGroupId: "group-1",
      beforeSend: persist,
    });

    await waitFor(() =>
      expect(beginCloudsyncActivity).toHaveBeenCalledTimes(2),
    );
    expect(persist).not.toHaveBeenCalled();
    expect(mocks.upsertChatMessage).not.toHaveBeenCalled();

    freshAcquisition.resolve();
    await waitFor(() => expect(mocks.upsertChatMessage).toHaveBeenCalledOnce());
    expect(endCloudsyncActivity).not.toHaveBeenCalledWith(
      "chat",
      startedNativeKey(1),
    );

    repair.resolve();
    await waitFor(() => expect(mocks.deleteChatMessage).toHaveBeenCalledOnce());
    await waitFor(
      () =>
        expect(endCloudsyncActivity).toHaveBeenCalledWith(
          "chat",
          startedNativeKey(1),
        ),
      { timeout: 1_500 },
    );
    expect(mocks.sendMessages).not.toHaveBeenCalled();
  });

  it("fails closed when repair activity acquisition also fails", async () => {
    vi.mocked(beginCloudsyncActivity)
      .mockRejectedValueOnce(new Error("initial activity drain timed out"))
      .mockRejectedValueOnce(new Error("guard repair activity drain timed out"))
      .mockRejectedValueOnce(
        new Error("finished repair activity drain timed out"),
      );
    const persist = vi.fn().mockResolvedValue(undefined);
    const harness = renderHarness();

    harness.props.sendMessage(userMessage, {
      chatGroupId: "group-1",
      beforeSend: persist,
    });

    await waitFor(() =>
      expect(beginCloudsyncActivity).toHaveBeenCalledTimes(3),
    );
    await waitFor(() => expect(endCloudsyncActivity).toHaveBeenCalledTimes(3));
    expect(persist).not.toHaveBeenCalled();
    expect(mocks.upsertChatMessage).not.toHaveBeenCalled();
    expect(mocks.deleteChatMessage).not.toHaveBeenCalled();
  });

  it("holds the failed preflight lease through deferred repair", async () => {
    const persist = vi
      .fn()
      .mockRejectedValueOnce(new Error("persist failed"))
      .mockResolvedValueOnce(undefined);
    const repair = deferredVoid();
    mocks.upsertChatMessage.mockReturnValueOnce(repair.promise);
    const harness = renderHarness();

    harness.props.sendMessage(userMessage, {
      chatGroupId: "group-1",
      beforeSend: persist,
    });

    await waitFor(() => expect(mocks.upsertChatMessage).toHaveBeenCalledOnce());
    expect(persist).toHaveBeenCalledTimes(2);
    await new Promise((resolve) =>
      setTimeout(resolve, CHAT_CLOUDSYNC_RELEASE_DELAY_MS + 50),
    );
    expect(endCloudsyncActivity).not.toHaveBeenCalled();

    repair.resolve();
    await waitFor(() => expect(mocks.deleteChatMessage).toHaveBeenCalledOnce());
    await waitFor(() => expect(endCloudsyncActivity).toHaveBeenCalledTimes(2), {
      timeout: 1_500,
    });
    expect(mocks.deleteChatMessage.mock.invocationCallOrder[0]).toBeLessThan(
      vi.mocked(endCloudsyncActivity).mock.invocationCallOrder[0],
    );
  });
});
