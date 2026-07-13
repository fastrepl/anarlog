import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";

import type { MeetingCapturedChatMessage } from "@hypr/plugin-detect";

import { startMeetingChatCapture } from "./meeting-chat-capture";

const {
  captureMeetingChatMessagesMock,
  listMicUsingApplicationsMock,
  persistMeetingChatRecordsMock,
  showTransientToastMock,
} = vi.hoisted(() => ({
  captureMeetingChatMessagesMock: vi.fn(),
  listMicUsingApplicationsMock: vi.fn(),
  persistMeetingChatRecordsMock: vi.fn(),
  showTransientToastMock: vi.fn(),
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

vi.mock("~/sidebar/toast/transient", () => ({
  showTransientToast: showTransientToastMock,
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

  test("baselines visible history and persists only later messages", async () => {
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
          sourceSignature: "zoom\nnative\nmsg-2",
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

  test("re-baselines after the validated chat surface disappears", async () => {
    const stop = startMeetingChatCapture({
      sessionId: "session-1",
      isEnabled: () => true,
    });
    await vi.advanceTimersByTimeAsync(0);

    captureMeetingChatMessagesMock.mockResolvedValue({
      status: "ok",
      data: {
        app: null,
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
    expect(persistMeetingChatRecordsMock).not.toHaveBeenCalled();

    const laterMessage = { ...capturedMessage, id: "after-rebaseline" };
    captureMeetingChatMessagesMock.mockResolvedValue(
      captureResult([capturedMessage, messageWhileHidden, laterMessage]),
    );
    await vi.advanceTimersByTimeAsync(5_000);
    stop();

    expect(persistMeetingChatRecordsMock).toHaveBeenCalledWith({
      sessionId: "session-1",
      entries: [
        {
          message: laterMessage,
          sourceSignature: "zoom\nnative\nafter-rebaseline",
        },
      ],
    });
  });

  test("excludes the generated disclosure while retaining participant chat", async () => {
    const disclosure = "Anarlog disclosure https://anarlog.so";
    captureMeetingChatMessagesMock.mockResolvedValue(captureResult([]));
    const stop = startMeetingChatCapture({
      sessionId: "session-1",
      isEnabled: () => true,
      excludedTexts: [disclosure],
    });
    await vi.advanceTimersByTimeAsync(0);

    captureMeetingChatMessagesMock.mockResolvedValue(
      captureResult([
        {
          ...capturedMessage,
          id: "disclosure",
          direction: "outgoing",
          text: `  ${disclosure.replace(" ", "\n")}  `,
          links: ["https://anarlog.so"],
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
          sourceSignature: "zoom\nnative\nmsg-1",
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

  test("fails closed when multiple supported meeting apps use the mic", async () => {
    listMicUsingApplicationsMock.mockResolvedValue({
      status: "ok",
      data: [
        { id: "us.zoom.xos", name: "Zoom" },
        { id: "com.tinyspeck.slackmacgap", name: "Slack" },
      ],
    });
    const stop = startMeetingChatCapture({
      sessionId: "session-1",
      isEnabled: () => true,
    });
    await vi.advanceTimersByTimeAsync(0);
    stop();

    expect(captureMeetingChatMessagesMock).not.toHaveBeenCalled();
    expect(persistMeetingChatRecordsMock).not.toHaveBeenCalled();
  });

  test("does not inspect unrelated mic-active apps", async () => {
    listMicUsingApplicationsMock.mockResolvedValue({
      status: "ok",
      data: [{ id: "com.google.Chrome", name: "Google Chrome" }],
    });
    const stop = startMeetingChatCapture({
      sessionId: "session-1",
      isEnabled: () => true,
    });
    await vi.advanceTimersByTimeAsync(0);
    stop();

    expect(captureMeetingChatMessagesMock).not.toHaveBeenCalled();
  });

  test("surfaces missing Accessibility permission once", async () => {
    captureMeetingChatMessagesMock.mockResolvedValue({
      status: "ok",
      data: {
        app: null,
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

    expect(showTransientToastMock).toHaveBeenCalledOnce();
    expect(showTransientToastMock).toHaveBeenCalledWith(
      expect.objectContaining({
        description:
          "Meeting chat capture needs Accessibility permission in Settings",
        variant: "warning",
      }),
      { durationMs: 6_000 },
    );
  });
});

function captureResult(messages: MeetingCapturedChatMessage[]) {
  return {
    status: "ok" as const,
    data: {
      app: { id: "us.zoom.xos", name: "Zoom" },
      platform: "zoom" as const,
      surface: "native" as const,
      messages,
      warnings: [],
    },
  };
}
