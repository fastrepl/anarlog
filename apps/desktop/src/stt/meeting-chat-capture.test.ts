import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";

import type { MeetingCapturedChatMessage } from "@hypr/plugin-detect";

import { startMeetingChatCapture } from "./meeting-chat-capture";

const {
  captureMeetingChatMessagesMock,
  listMicUsingApplicationsMock,
  persistMeetingChatRecordsMock,
  sonnerToastWarningMock,
  captureSettingState,
} = vi.hoisted(() => ({
  captureMeetingChatMessagesMock: vi.fn(),
  listMicUsingApplicationsMock: vi.fn(),
  persistMeetingChatRecordsMock: vi.fn(),
  sonnerToastWarningMock: vi.fn(),
  captureSettingState: { value: true },
}));

vi.mock("@hypr/plugin-detect", () => ({
  commands: {
    captureMeetingChatMessages: captureMeetingChatMessagesMock,
    listMicUsingApplications: listMicUsingApplicationsMock,
  },
}));

vi.mock("~/stt/meeting-chat-records", () => ({
  persistMeetingChatRecords: persistMeetingChatRecordsMock,
}));

vi.mock("@hypr/ui/components/ui/toast", () => ({
  sonnerToast: { warning: sonnerToastWarningMock },
}));

vi.mock("~/settings/queries", () => ({
  getStoredSettingValues: vi.fn(async () => ({
    values: { capture_meeting_chat: captureSettingState.value },
    hasValues: new Set(["capture_meeting_chat"]),
  })),
}));

const capturedMessage = {
  id: "msg-1",
  platform: "zoom" as const,
  surface: "native" as const,
  sender: "Ada",
  timestamp: "10:42 AM",
  direction: "incoming" as const,
  text: "Here is the doc https://example.com/spec",
  links: ["https://example.com/spec"],
};

describe("startMeetingChatCapture", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.clearAllMocks();
    captureSettingState.value = true;
    listMicUsingApplicationsMock.mockResolvedValue({
      status: "ok",
      data: [{ id: "us.zoom.xos", name: "Zoom" }],
    });
    captureMeetingChatMessagesMock.mockResolvedValue(
      captureResult([capturedMessage]),
    );
    persistMeetingChatRecordsMock.mockImplementation(
      async ({ entries }: { entries: Array<{ sourceSignature: string }> }) =>
        entries.map((entry) => entry.sourceSignature),
    );
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  test("persists an appended message while the meeting context stays the same", async () => {
    const stop = startMeetingChatCapture({
      sessionId: "session-1",
      isEnabled: () => true,
    });
    await vi.advanceTimersByTimeAsync(0);

    expect(persistMeetingChatRecordsMock).not.toHaveBeenCalled();

    const laterMessage = {
      ...capturedMessage,
      id: "msg-2",
      text: "Let's discuss this next",
      links: [],
    };
    captureMeetingChatMessagesMock.mockResolvedValue(
      captureResult([capturedMessage, laterMessage]),
    );
    await vi.advanceTimersByTimeAsync(5_000);
    stop();

    expect(persistMeetingChatRecordsMock).toHaveBeenCalledWith({
      sessionId: "session-1",
      entries: [
        {
          message: laterMessage,
          sourceSignature: "zoom:meeting-1\nzoom\nnative\nmsg-2",
        },
      ],
    });
  });

  test("reads the current capture setting after the initiating view unmounts", async () => {
    const stop = startMeetingChatCapture({ sessionId: "session-1" });
    await vi.advanceTimersByTimeAsync(0);

    captureSettingState.value = false;
    await vi.advanceTimersByTimeAsync(5_000);
    expect(captureMeetingChatMessagesMock).toHaveBeenCalledTimes(1);

    const messageWhileDisabled = {
      ...capturedMessage,
      id: "while-disabled",
    };
    captureMeetingChatMessagesMock.mockResolvedValue(
      captureResult([capturedMessage, messageWhileDisabled]),
    );
    captureSettingState.value = true;
    await vi.advanceTimersByTimeAsync(5_000);
    expect(persistMeetingChatRecordsMock).not.toHaveBeenCalled();

    const laterMessage = { ...capturedMessage, id: "after-reenable" };
    captureMeetingChatMessagesMock.mockResolvedValue(
      captureResult([capturedMessage, messageWhileDisabled, laterMessage]),
    );
    await vi.advanceTimersByTimeAsync(5_000);
    stop();

    expect(persistMeetingChatRecordsMock).toHaveBeenCalledWith({
      sessionId: "session-1",
      entries: [
        {
          message: laterMessage,
          sourceSignature: "zoom:meeting-1\nzoom\nnative\nafter-reenable",
        },
      ],
    });
  });

  test("re-baselines history and scopes reused AX ids when the meeting context changes", async () => {
    const stop = startMeetingChatCapture({
      sessionId: "session-1",
      isEnabled: () => true,
    });
    await vi.advanceTimersByTimeAsync(0);

    const nextMeetingHistory = {
      ...capturedMessage,
      id: "next-meeting-history",
      text: "This belongs to another meeting",
      links: [],
    };
    captureMeetingChatMessagesMock.mockResolvedValue(
      captureResult([nextMeetingHistory], "zoom:meeting-2"),
    );
    await vi.advanceTimersByTimeAsync(5_000);

    expect(persistMeetingChatRecordsMock).not.toHaveBeenCalled();

    const nextMeetingMessage = {
      ...capturedMessage,
      id: "msg-1",
      text: "This was sent after switching",
      links: [],
    };
    captureMeetingChatMessagesMock.mockResolvedValue(
      captureResult([nextMeetingHistory, nextMeetingMessage], "zoom:meeting-2"),
    );
    await vi.advanceTimersByTimeAsync(5_000);
    stop();

    expect(persistMeetingChatRecordsMock).toHaveBeenCalledOnce();
    expect(persistMeetingChatRecordsMock).toHaveBeenCalledWith({
      sessionId: "session-1",
      entries: [
        {
          message: nextMeetingMessage,
          sourceSignature: "zoom:meeting-2\nzoom\nnative\nmsg-1",
        },
      ],
    });
  });

  test("keeps the last context across a transient missing-context poll", async () => {
    const stop = startMeetingChatCapture({
      sessionId: "session-1",
      isEnabled: () => true,
    });
    await vi.advanceTimersByTimeAsync(0);

    const messageWithoutContext = {
      ...capturedMessage,
      id: "missing-context",
    };
    captureMeetingChatMessagesMock.mockResolvedValue(
      captureResult([capturedMessage, messageWithoutContext], null),
    );
    await vi.advanceTimersByTimeAsync(5_000);

    captureMeetingChatMessagesMock.mockResolvedValue(
      captureResult([capturedMessage, messageWithoutContext]),
    );
    await vi.advanceTimersByTimeAsync(5_000);
    stop();

    expect(persistMeetingChatRecordsMock).toHaveBeenCalledWith({
      sessionId: "session-1",
      entries: [
        {
          message: messageWithoutContext,
          sourceSignature: "zoom:meeting-1\nzoom\nnative\nmissing-context",
        },
      ],
    });
  });

  test("captures the first message after a visible empty-chat baseline", async () => {
    captureMeetingChatMessagesMock.mockResolvedValue(captureResult([]));
    const stop = startMeetingChatCapture({
      sessionId: "session-1",
      isEnabled: () => true,
    });
    await vi.advanceTimersByTimeAsync(0);

    captureMeetingChatMessagesMock.mockResolvedValue(
      captureResult([capturedMessage]),
    );
    await vi.advanceTimersByTimeAsync(5_000);
    stop();

    expect(persistMeetingChatRecordsMock).toHaveBeenCalledOnce();
  });

  test("keeps the last context while the validated chat surface is hidden", async () => {
    const stop = startMeetingChatCapture({
      sessionId: "session-1",
      isEnabled: () => true,
    });
    await vi.advanceTimersByTimeAsync(0);

    captureMeetingChatMessagesMock.mockResolvedValue({
      status: "ok",
      data: {
        app: null,
        contextId: null,
        platform: "unknown",
        surface: "unknown",
        messages: [],
        warnings: ["no visible supported meeting chat messages found"],
      },
    });
    await vi.advanceTimersByTimeAsync(5_000);

    const messageWhileHidden = {
      ...capturedMessage,
      id: "while-hidden",
    };
    captureMeetingChatMessagesMock.mockResolvedValue(
      captureResult([capturedMessage, messageWhileHidden]),
    );
    await vi.advanceTimersByTimeAsync(5_000);
    expect(persistMeetingChatRecordsMock).toHaveBeenCalledWith({
      sessionId: "session-1",
      entries: [
        {
          message: messageWhileHidden,
          sourceSignature: "zoom:meeting-1\nzoom\nnative\nwhile-hidden",
        },
      ],
    });

    const laterMessage = { ...capturedMessage, id: "after-rebaseline" };
    captureMeetingChatMessagesMock.mockResolvedValue(
      captureResult([capturedMessage, messageWhileHidden, laterMessage]),
    );
    await vi.advanceTimersByTimeAsync(5_000);
    stop();

    expect(persistMeetingChatRecordsMock).toHaveBeenLastCalledWith({
      sessionId: "session-1",
      entries: [
        {
          message: laterMessage,
          sourceSignature: "zoom:meeting-1\nzoom\nnative\nafter-rebaseline",
        },
      ],
    });
  });

  test("excludes configured message text while retaining other chat", async () => {
    const excludedText = "Automated meeting message";
    captureMeetingChatMessagesMock.mockResolvedValue(captureResult([]));
    const stop = startMeetingChatCapture({
      sessionId: "session-1",
      isEnabled: () => true,
      excludedTexts: [excludedText],
    });
    await vi.advanceTimersByTimeAsync(0);

    captureMeetingChatMessagesMock.mockResolvedValue(
      captureResult([
        {
          ...capturedMessage,
          id: "excluded-message",
          direction: "outgoing",
          text: `  ${excludedText.replace(" ", "\n")}  `,
          links: [],
        },
        capturedMessage,
      ]),
    );
    await vi.advanceTimersByTimeAsync(5_000);
    stop();

    expect(persistMeetingChatRecordsMock).toHaveBeenCalledWith({
      sessionId: "session-1",
      entries: [
        {
          message: capturedMessage,
          sourceSignature: "zoom:meeting-1\nzoom\nnative\nmsg-1",
        },
      ],
    });
  });

  test("retries a message after a storage failure", async () => {
    captureMeetingChatMessagesMock.mockResolvedValue(captureResult([]));
    const stop = startMeetingChatCapture({
      sessionId: "session-1",
      isEnabled: () => true,
    });
    await vi.advanceTimersByTimeAsync(0);

    captureMeetingChatMessagesMock.mockResolvedValue(
      captureResult([capturedMessage]),
    );
    persistMeetingChatRecordsMock.mockRejectedValueOnce(new Error("locked"));
    await vi.advanceTimersByTimeAsync(5_000);
    await vi.advanceTimersByTimeAsync(5_000);
    stop();

    expect(persistMeetingChatRecordsMock).toHaveBeenCalledTimes(2);
  });

  test("does not persist a poll that resolves after capture stops", async () => {
    let resolveCapture: ((value: unknown) => void) | undefined;
    captureMeetingChatMessagesMock.mockReturnValue(
      new Promise((resolve) => {
        resolveCapture = resolve;
      }),
    );
    const stop = startMeetingChatCapture({
      sessionId: "session-1",
      isEnabled: () => true,
    });
    await vi.advanceTimersByTimeAsync(0);

    stop();
    resolveCapture?.(captureResult([capturedMessage]));
    await Promise.resolve();
    await Promise.resolve();

    expect(persistMeetingChatRecordsMock).not.toHaveBeenCalled();
  });

  test("does not inspect apps while disabled and re-baselines when enabled", async () => {
    let enabled = false;
    const stop = startMeetingChatCapture({
      sessionId: "session-1",
      isEnabled: () => enabled,
    });
    await vi.advanceTimersByTimeAsync(0);

    expect(listMicUsingApplicationsMock).not.toHaveBeenCalled();

    enabled = true;
    await vi.advanceTimersByTimeAsync(5_000);
    expect(captureMeetingChatMessagesMock).toHaveBeenCalledWith([
      "us.zoom.xos",
    ]);
    expect(persistMeetingChatRecordsMock).not.toHaveBeenCalled();

    enabled = false;
    await vi.advanceTimersByTimeAsync(5_000);
    enabled = true;
    captureMeetingChatMessagesMock.mockResolvedValue(
      captureResult([
        capturedMessage,
        { ...capturedMessage, id: "during-disabled" },
      ]),
    );
    await vi.advanceTimersByTimeAsync(5_000);
    stop();

    expect(persistMeetingChatRecordsMock).not.toHaveBeenCalled();
  });

  test("lets Rust fail closed when multiple recognized meeting apps use the mic", async () => {
    listMicUsingApplicationsMock.mockResolvedValue({
      status: "ok",
      data: [
        { id: "us.zoom.xos", name: "Zoom" },
        { id: "com.tinyspeck.slackmacgap", name: "Slack" },
      ],
    });
    captureMeetingChatMessagesMock.mockResolvedValue({
      status: "ok",
      data: {
        app: null,
        contextId: null,
        platform: "unknown",
        surface: "unknown",
        messages: [],
        warnings: ["expected exactly one active supported meeting app"],
      },
    });
    const stop = startMeetingChatCapture({
      sessionId: "session-1",
      isEnabled: () => true,
    });
    await vi.advanceTimersByTimeAsync(0);
    stop();

    expect(captureMeetingChatMessagesMock).toHaveBeenCalledWith([
      "us.zoom.xos",
      "com.tinyspeck.slackmacgap",
    ]);
    expect(persistMeetingChatRecordsMock).not.toHaveBeenCalled();
  });

  test("recognizes the alternate Slack bundle id", async () => {
    listMicUsingApplicationsMock.mockResolvedValue({
      status: "ok",
      data: [{ id: "com.slack.Slack", name: "Slack" }],
    });
    captureMeetingChatMessagesMock.mockResolvedValue({
      status: "ok",
      data: {
        app: { id: "com.slack.Slack", name: "Slack" },
        contextId: "slack:test",
        platform: "slack",
        surface: "native",
        messages: [],
        warnings: [],
      },
    });
    const stop = startMeetingChatCapture({
      sessionId: "session-1",
      isEnabled: () => true,
    });
    await vi.advanceTimersByTimeAsync(0);
    stop();

    expect(captureMeetingChatMessagesMock).toHaveBeenCalledWith([
      "com.slack.Slack",
    ]);
  });

  test("passes a mic-active browser to Rust for provider resolution", async () => {
    listMicUsingApplicationsMock.mockResolvedValue({
      status: "ok",
      data: [{ id: "com.google.Chrome", name: "Google Chrome" }],
    });
    captureMeetingChatMessagesMock.mockResolvedValue({
      status: "ok",
      data: {
        app: { id: "com.google.Chrome", name: "Google Chrome" },
        contextId: "google-meet:meeting-1",
        platform: "googleMeet",
        surface: "web",
        messages: [],
        warnings: [],
      },
    });
    const stop = startMeetingChatCapture({
      sessionId: "session-1",
      isEnabled: () => true,
    });
    await vi.advanceTimersByTimeAsync(0);
    stop();

    expect(captureMeetingChatMessagesMock).toHaveBeenCalledWith([
      "com.google.Chrome",
    ]);
    expect(persistMeetingChatRecordsMock).not.toHaveBeenCalled();
  });

  test("rejects capture results for an app outside the caller-observed mic scope", async () => {
    captureMeetingChatMessagesMock.mockResolvedValue({
      status: "ok",
      data: {
        app: { id: "com.tinyspeck.slackmacgap", name: "Slack" },
        contextId: "slack:test",
        platform: "slack",
        surface: "native",
        messages: [{ ...capturedMessage, platform: "slack" }],
        warnings: [],
      },
    });
    const stop = startMeetingChatCapture({
      sessionId: "session-1",
      isEnabled: () => true,
    });
    await vi.advanceTimersByTimeAsync(0);

    captureMeetingChatMessagesMock.mockResolvedValue({
      status: "ok",
      data: {
        app: { id: "com.tinyspeck.slackmacgap", name: "Slack" },
        contextId: "slack:test",
        platform: "slack",
        surface: "native",
        messages: [
          { ...capturedMessage, platform: "slack" },
          { ...capturedMessage, id: "msg-2", platform: "slack" },
        ],
        warnings: [],
      },
    });
    await vi.advanceTimersByTimeAsync(5_000);
    stop();

    expect(captureMeetingChatMessagesMock).toHaveBeenCalledWith([
      "us.zoom.xos",
    ]);
    expect(persistMeetingChatRecordsMock).not.toHaveBeenCalled();
  });

  test("surfaces missing Accessibility permission once", async () => {
    captureMeetingChatMessagesMock.mockResolvedValue({
      status: "ok",
      data: {
        app: null,
        contextId: null,
        platform: "unknown",
        surface: "unknown",
        messages: [],
        warnings: ["macOS accessibility permission is not trusted"],
      },
    });
    const stop = startMeetingChatCapture({
      sessionId: "session-1",
      isEnabled: () => true,
    });
    await vi.advanceTimersByTimeAsync(5_000);
    stop();

    expect(sonnerToastWarningMock).toHaveBeenCalledOnce();
    expect(sonnerToastWarningMock).toHaveBeenCalledWith(
      "Meeting chat capture needs Accessibility permission in Settings",
      {
        id: "meeting-chat-capture-warning",
        duration: 6_000,
      },
    );
  });
});

function captureResult(
  messages: MeetingCapturedChatMessage[],
  contextId: string | null = "zoom:meeting-1",
) {
  return {
    status: "ok" as const,
    data: {
      app: { id: "us.zoom.xos", name: "Zoom" },
      contextId,
      platform: "zoom" as const,
      surface: "native" as const,
      messages,
      warnings: [],
    },
  };
}
