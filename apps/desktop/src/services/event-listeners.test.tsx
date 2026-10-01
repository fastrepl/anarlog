import { render } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";

import { EventListeners } from "./event-listeners";

import {
  cancelAutoStopEndedNotification,
  createAutoStopEndedNotificationKey,
} from "~/stt/auto-stop-notification";
import { createBatchCompletedNotificationKey } from "~/stt/batch-completed-notification";

const speakerContextJson = JSON.stringify({
  intervals: [
    {
      start_ms: 0,
      end_ms: 60_000,
      active_call: false,
      calendar_call: false,
      mic_isolated: null,
      shared_microphone: false,
      title: "",
      self_names: [],
      participants: [],
    },
  ],
});

const {
  notificationListenMock,
  updaterListenMock,
  maybeEmitUpdatedMock,
  getCurrentWebviewWindowLabelMock,
  liveQuerySubscribeMock,
  listenerSubscribeMock,
  useConfigValueMock,
  useConfigValuesMock,
  setSettingValueMock,
  openNewMock,
  createSessionMock,
  getOrCreateSessionForEventIdMock,
  getCalendarEventStartedAtMock,
  getCalendarEventMeetingLinkMock,
  openUrlMock,
  setTriggerAppIdsMock,
  stopMock,
  updateCaptureConfigMock,
  getListenerStateMock,
} = vi.hoisted(() => ({
  notificationListenMock: vi.fn(),
  updaterListenMock: vi.fn(),
  maybeEmitUpdatedMock: vi.fn(),
  getCurrentWebviewWindowLabelMock: vi.fn(() => "main"),
  liveQuerySubscribeMock: vi.fn(),
  listenerSubscribeMock: vi.fn(),
  useConfigValueMock: vi.fn((): string[] => []),
  useConfigValuesMock: vi.fn(),
  setSettingValueMock: vi.fn(async () => {}),
  openNewMock: vi.fn(),
  createSessionMock: vi.fn(async () => "session-new"),
  getOrCreateSessionForEventIdMock: vi.fn(async () => "session-event"),
  getCalendarEventStartedAtMock: vi.fn(),
  getCalendarEventMeetingLinkMock: vi.fn(),
  openUrlMock: vi.fn(
    async (): Promise<
      { status: "ok"; data: null } | { status: "error"; error: string }
    > => ({ status: "ok", data: null }),
  ),
  setTriggerAppIdsMock: vi.fn(),
  stopMock: vi.fn(),
  updateCaptureConfigMock: vi.fn(),
  getListenerStateMock: vi.fn(),
}));

vi.mock("@anlg/plugin-notification", () => ({
  events: {
    notificationEvent: {
      listen: notificationListenMock,
    },
  },
}));

vi.mock("@anlg/plugin-opener2", () => ({
  commands: { openUrl: openUrlMock },
}));

vi.mock("@anlg/plugin-updater2", () => ({
  commands: {
    maybeEmitUpdated: maybeEmitUpdatedMock,
  },
  events: {
    updatedEvent: {
      listen: updaterListenMock,
    },
  },
}));

vi.mock("@anlg/plugin-windows", () => ({
  getCurrentWebviewWindowLabel: getCurrentWebviewWindowLabelMock,
}));

vi.mock("~/db", () => ({
  liveQueryClient: {
    subscribe: liveQuerySubscribeMock,
  },
}));

vi.mock("~/shared/config", () => ({
  useConfigValue: useConfigValueMock,
  useConfigValues: useConfigValuesMock,
}));

vi.mock("~/settings/queries", () => ({
  setSettingValue: setSettingValueMock,
}));

vi.mock("~/session/queries", () => ({
  createSession: createSessionMock,
  getOrCreateSessionForEventId: getOrCreateSessionForEventIdMock,
}));

vi.mock("~/calendar/queries", () => ({
  getCalendarEventStartedAt: getCalendarEventStartedAtMock,
  getCalendarEventMeetingLink: getCalendarEventMeetingLinkMock,
}));

vi.mock("~/store/zustand/tabs", () => ({
  useTabs: (selector: (state: { openNew: typeof openNewMock }) => unknown) =>
    selector({ openNew: openNewMock }),
}));

vi.mock("~/store/zustand/listener/instance", () => ({
  listenerStore: {
    getState: getListenerStateMock,
    subscribe: listenerSubscribeMock,
  },
}));

function findLiveQueryHandlers(sqlFragment: string) {
  const call = liveQuerySubscribeMock.mock.calls.find(([sql]) =>
    String(sql).includes(sqlFragment),
  );
  expect(call).toBeDefined();
  return call![2] as {
    onData: (rows: unknown[]) => void;
    onError: (error: unknown) => void;
  };
}

async function renderNotificationHandler() {
  render(<EventListeners />);
  await vi.waitFor(() =>
    expect(notificationListenMock).toHaveBeenCalledTimes(1),
  );
  return notificationListenMock.mock.calls[0]![0] as (event: {
    payload: Record<string, unknown>;
  }) => unknown;
}

function useLiveLanguage(language: string) {
  useConfigValuesMock.mockReturnValue({
    ai_language: language,
    spoken_languages: [language],
    current_stt_provider: "soniox",
    current_stt_model: "stt-v4",
  });
}

function participantRows(...humanIds: string[]) {
  return humanIds.map((human_id) => ({
    session_id: "session-1",
    owner_user_id: "human-self",
    human_id,
  }));
}

async function renderLiveCaptureSync() {
  render(<EventListeners />);
  await vi.waitFor(() =>
    expect(liveQuerySubscribeMock).toHaveBeenCalledTimes(2),
  );
}

function emptyTranscriptRow(
  words: unknown[] = [],
  hints: unknown[] = [],
): Record<string, unknown> {
  return {
    id: "transcript-1",
    started_at_ms: 1_000,
    speaker_context: null,
    words_json: JSON.stringify(words),
    speaker_hints_json: JSON.stringify(hints),
  };
}

const remoteSpeakerHints = [
  {
    id: "w1:provider_speaker_index",
    word_id: "w1",
    type: "provider_speaker_index",
    value: JSON.stringify({ channel: 1, speaker_index: 0 }),
  },
  {
    id: "w1:user_speaker_assignment",
    word_id: "w1",
    type: "user_speaker_assignment",
    value: JSON.stringify({
      human_id: "human-artem",
      scope: "speaker",
      channel: 1,
      speaker_index: 0,
    }),
  },
];

const artemRemoteAssignment = {
  human_id: "human-artem",
  scope: {
    kind: "channel_speaker",
    channel: "RemoteParty",
    speaker_index: 0,
  },
};
describe("EventListeners notification events", () => {
  beforeEach(() => {
    cancelAutoStopEndedNotification("session-1");
    cancelAutoStopEndedNotification("session-old");
    notificationListenMock.mockReset();
    updaterListenMock.mockReset();
    maybeEmitUpdatedMock.mockReset();
    getCurrentWebviewWindowLabelMock.mockReset();
    liveQuerySubscribeMock.mockReset();
    listenerSubscribeMock.mockReset();
    useConfigValueMock.mockReset();
    useConfigValuesMock.mockReset();
    setSettingValueMock.mockReset();
    openNewMock.mockReset();
    createSessionMock.mockReset();
    getOrCreateSessionForEventIdMock.mockReset();
    getCalendarEventStartedAtMock.mockReset();
    getCalendarEventMeetingLinkMock.mockReset();
    openUrlMock.mockReset();
    setTriggerAppIdsMock.mockReset();
    stopMock.mockReset();
    updateCaptureConfigMock.mockReset();
    getListenerStateMock.mockReset();

    getCurrentWebviewWindowLabelMock.mockReturnValue("main");
    notificationListenMock.mockResolvedValue(() => {});
    updaterListenMock.mockResolvedValue(() => {});
    createSessionMock.mockResolvedValue("session-new");
    getOrCreateSessionForEventIdMock.mockResolvedValue("session-event");
    getCalendarEventStartedAtMock.mockResolvedValue(null);
    getCalendarEventMeetingLinkMock.mockResolvedValue(null);
    openUrlMock.mockResolvedValue({ status: "ok", data: null });
    liveQuerySubscribeMock.mockImplementation(
      async (_sql, _params, handlers) => {
        handlers.onData([]);
        return async () => {};
      },
    );
    listenerSubscribeMock.mockReturnValue(() => {});
    useConfigValueMock.mockReturnValue([]);
    useConfigValuesMock.mockReturnValue({
      ai_language: "en",
      spoken_languages: [],
      current_stt_provider: undefined,
      current_stt_model: undefined,
    });
    setSettingValueMock.mockResolvedValue(undefined);
    getListenerStateMock.mockReturnValue({
      setTriggerAppIds: setTriggerAppIdsMock,
      stop: stopMock,
      updateCaptureConfig: updateCaptureConfigMock,
      live: {
        status: "active",
        sessionId: "session-1",
        captureGenerationBySession: { "session-1": 1 },
      },
    });
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.restoreAllMocks();
  });

  test("stores mic-detected footer actions as ignored platforms", async () => {
    useConfigValueMock.mockReturnValue(["com.existing.app"]);
    const handler = await renderNotificationHandler();

    handler({
      payload: {
        type: "notification_footer_action",
        key: "mic-1",
        source: {
          type: "mic_detected",
          app_names: ["Zoom"],
          app_ids: ["us.zoom.xos", "com.existing.app"],
          event_ids: [],
        },
      },
    });

    expect(setSettingValueMock).toHaveBeenCalledWith(
      "ignored_platforms",
      JSON.stringify(["com.existing.app", "us.zoom.xos"]),
    );
    expect(openNewMock).not.toHaveBeenCalled();
  });

  test.each(["notification_accept", "notification_timeout"])(
    "%s with auto-stop prompt stops the active session",
    async (type) => {
      const handler = await renderNotificationHandler();

      handler({
        payload: {
          type,
          key: createAutoStopEndedNotificationKey("session-1"),
          source: null,
        },
      });

      expect(stopMock).toHaveBeenCalledTimes(1);
      expect(createSessionMock).not.toHaveBeenCalled();
      expect(openNewMock).not.toHaveBeenCalled();
    },
  );

  test.each([
    ["notification_timeout", "session-old"],
    ["notification_confirm", "session-1"],
  ])(
    "%s for auto-stop prompt of %s does not stop recording",
    async (type, sessionId) => {
      const handler = await renderNotificationHandler();

      handler({
        payload: {
          type,
          key: createAutoStopEndedNotificationKey(sessionId),
          source: null,
        },
      });

      expect(stopMock).not.toHaveBeenCalled();
      expect(createSessionMock).not.toHaveBeenCalled();
      expect(openNewMock).not.toHaveBeenCalled();
    },
  );

  test("notification_action open_meeting opens the current event link only", async () => {
    getCalendarEventMeetingLinkMock.mockResolvedValue(
      "https://meet.example.com/current",
    );
    const handler = await renderNotificationHandler();

    handler({
      payload: {
        type: "notification_action",
        action: "open_meeting",
        source: { type: "calendar_event", event_id: "evt-1" },
      },
    });

    await vi.waitFor(() =>
      expect(openUrlMock).toHaveBeenCalledWith(
        "https://meet.example.com/current",
        null,
      ),
    );
    expect(openNewMock).not.toHaveBeenCalled();
    expect(getOrCreateSessionForEventIdMock).not.toHaveBeenCalled();
  });

  test("notification_action join_and_record starts immediately and opens the meeting", async () => {
    getCalendarEventMeetingLinkMock.mockResolvedValue(
      "https://meet.example.com/current",
    );
    const handler = await renderNotificationHandler();

    handler({
      payload: {
        type: "notification_action",
        action: "join_and_record",
        source: { type: "calendar_event", event_id: "evt-1" },
      },
    });

    await vi.waitFor(() =>
      expect(openUrlMock).toHaveBeenCalledWith(
        "https://meet.example.com/current",
        null,
      ),
    );
    expect(openNewMock).toHaveBeenCalledWith({
      type: "sessions",
      id: "session-event",
      state: {
        view: null,
        autoStart: true,
      },
    });
  });

  test("notification_action join_and_record still starts when the meeting link is unavailable", async () => {
    const handler = await renderNotificationHandler();

    handler({
      payload: {
        type: "notification_action",
        action: "join_and_record",
        source: { type: "calendar_event", event_id: "evt-1" },
      },
    });

    await vi.waitFor(() =>
      expect(openNewMock).toHaveBeenCalledWith({
        type: "sessions",
        id: "session-event",
        state: {
          view: null,
          autoStart: true,
        },
      }),
    );
    expect(openUrlMock).not.toHaveBeenCalled();
  });

  test("notification_action join_and_record still opens the meeting when recording setup fails", async () => {
    const consoleError = vi
      .spyOn(console, "error")
      .mockImplementation(() => undefined);
    getOrCreateSessionForEventIdMock.mockRejectedValue(
      new Error("database unavailable"),
    );
    getCalendarEventMeetingLinkMock.mockResolvedValue(
      "https://meet.example.com/current",
    );
    const handler = await renderNotificationHandler();

    handler({
      payload: {
        type: "notification_action",
        action: "join_and_record",
        source: { type: "calendar_event", event_id: "evt-1" },
      },
    });

    await vi.waitFor(() =>
      expect(openUrlMock).toHaveBeenCalledWith(
        "https://meet.example.com/current",
        null,
      ),
    );
    expect(openNewMock).not.toHaveBeenCalled();
    consoleError.mockRestore();
  });

  test("notification_action reports meeting opener result errors", async () => {
    const consoleError = vi
      .spyOn(console, "error")
      .mockImplementation(() => undefined);
    openUrlMock.mockResolvedValue({
      status: "error",
      error: "No application",
    });
    getCalendarEventMeetingLinkMock.mockResolvedValue(
      "https://meet.example.com/current",
    );
    const handler = await renderNotificationHandler();

    handler({
      payload: {
        type: "notification_action",
        action: "open_meeting",
        source: { type: "calendar_event", event_id: "evt-1" },
      },
    });

    await vi.waitFor(() =>
      expect(consoleError).toHaveBeenCalledWith(
        "[notification] failed to open calendar meeting",
        expect.objectContaining({ message: "No application" }),
      ),
    );
    consoleError.mockRestore();
  });

  test("live capture config sync pushes remotes before the transcript snapshot", async () => {
    vi.useFakeTimers();
    useLiveLanguage("ko");
    liveQuerySubscribeMock.mockImplementation(
      async (sql, _params, handlers) => {
        if (!String(sql).includes("FROM transcripts")) {
          handlers.onData([]);
        }
        return async () => {};
      },
    );

    await renderLiveCaptureSync();
    findLiveQueryHandlers("session_participants").onData(
      participantRows("human-remote"),
    );
    await vi.runOnlyPendingTimersAsync();

    expect(updateCaptureConfigMock).toHaveBeenCalledTimes(1);
    expect(updateCaptureConfigMock).toHaveBeenCalledWith({
      session_id: "session-1",
      languages: ["ko"],
      participant_human_ids: ["human-remote"],
      self_human_id: "human-self",
      speaker_assignments: [],
    });

    findLiveQueryHandlers("FROM transcripts").onData([]);
    await vi.runOnlyPendingTimersAsync();

    expect(updateCaptureConfigMock).toHaveBeenCalledTimes(1);
  });

  test.each([
    [
      "the transcript read fails",
      (handlers: { onError: (error: unknown) => void }) =>
        handlers.onError("no such table: transcripts"),
    ],
    [
      "the transcript subscription rejects",
      () => {
        throw new Error("subscribe failed");
      },
    ],
  ])(
    "live capture config sync runs without names when %s",
    async (_name, failTranscripts) => {
      vi.useFakeTimers();
      vi.spyOn(console, "error").mockImplementation(() => {});
      useLiveLanguage("ko");
      liveQuerySubscribeMock.mockImplementation(
        async (sql, _params, handlers) => {
          if (String(sql).includes("FROM transcripts")) {
            failTranscripts(handlers);
          } else {
            handlers.onData([]);
          }
          return async () => {};
        },
      );

      await renderLiveCaptureSync();
      findLiveQueryHandlers("session_participants").onData(
        participantRows("human-remote"),
      );
      await vi.runOnlyPendingTimersAsync();

      expect(updateCaptureConfigMock).toHaveBeenCalledTimes(1);
      expect(updateCaptureConfigMock).toHaveBeenCalledWith({
        session_id: "session-1",
        languages: ["ko"],
        participant_human_ids: ["human-remote"],
        self_human_id: "human-self",
        speaker_assignments: [],
      });
    },
  );

  test("live capture config sync pushes the active transcript's speaker assignments", async () => {
    vi.useFakeTimers();
    useLiveLanguage("en");

    await renderLiveCaptureSync();
    const transcriptCall = liveQuerySubscribeMock.mock.calls.find(([sql]) =>
      String(sql).includes("FROM transcripts"),
    );
    expect(transcriptCall?.[1]).toEqual(["session-1"]);

    findLiveQueryHandlers("session_participants").onData(
      participantRows("human-artem", "human-guest"),
    );
    findLiveQueryHandlers("FROM transcripts").onData([
      {
        ...emptyTranscriptRow(
          [
            { id: "w1", text: " hello", start_ms: 0, end_ms: 100, channel: 1 },
            {
              id: "w2",
              text: " there",
              start_ms: 100,
              end_ms: 200,
              channel: 1,
            },
          ],
          [
            ...remoteSpeakerHints,
            {
              id: "w2:user_speaker_assignment:segment",
              word_id: "w2",
              type: "user_speaker_assignment",
              value: JSON.stringify({
                human_id: "human-guest",
                scope: "segment",
                word_ids: ["w2"],
              }),
            },
          ],
        ),
        speaker_context: speakerContextJson,
      },
    ]);
    await vi.runOnlyPendingTimersAsync();

    expect(updateCaptureConfigMock).toHaveBeenCalledTimes(1);
    expect(updateCaptureConfigMock).toHaveBeenCalledWith({
      session_id: "session-1",
      languages: ["en"],
      participant_human_ids: ["human-artem", "human-guest"],
      self_human_id: "human-self",
      speaker_assignments: [
        artemRemoteAssignment,
        {
          human_id: "human-guest",
          scope: { kind: "words", word_ids: ["w2"] },
        },
      ],
    });
  });

  test.each([
    {
      invitees: ["human-remote"],
      expected: [
        {
          human_id: "human-self",
          scope: { kind: "channel", channel: "DirectMic" },
        },
        {
          human_id: "human-remote",
          scope: { kind: "channel", channel: "RemoteParty" },
        },
      ],
    },
    {
      invitees: ["human-a", "human-b"],
      expected: [
        {
          human_id: "human-self",
          scope: { kind: "channel", channel: "DirectMic" },
        },
      ],
    },
  ])(
    "live capture config sync adds channel defaults without speaker context for invitees $invitees",
    async ({ invitees, expected }) => {
      vi.useFakeTimers();
      useLiveLanguage("en");

      await renderLiveCaptureSync();
      findLiveQueryHandlers("session_participants").onData(
        participantRows(...invitees),
      );
      findLiveQueryHandlers("FROM transcripts").onData([emptyTranscriptRow()]);
      await vi.runOnlyPendingTimersAsync();

      expect(updateCaptureConfigMock).toHaveBeenCalledWith({
        session_id: "session-1",
        languages: ["en"],
        participant_human_ids: invitees,
        self_human_id: "human-self",
        speaker_assignments: expected,
      });
    },
  );

  test("live capture config sync keeps explicit channel assignments over defaults", async () => {
    vi.useFakeTimers();
    useLiveLanguage("en");

    await renderLiveCaptureSync();
    findLiveQueryHandlers("session_participants").onData(
      participantRows("human-remote"),
    );
    findLiveQueryHandlers("FROM transcripts").onData([
      emptyTranscriptRow(
        [{ id: "w1", text: " hi", start_ms: 0, end_ms: 100, channel: 1 }],
        [
          {
            id: "w1:user_speaker_assignment",
            word_id: "w1",
            type: "user_speaker_assignment",
            value: JSON.stringify({
              human_id: "human-pinned",
              scope: "speaker",
              channel: 1,
              speaker_index: null,
            }),
          },
        ],
      ),
    ]);
    await vi.runOnlyPendingTimersAsync();

    expect(updateCaptureConfigMock).toHaveBeenCalledWith({
      session_id: "session-1",
      languages: ["en"],
      participant_human_ids: ["human-remote"],
      self_human_id: "human-self",
      speaker_assignments: [
        {
          human_id: "human-pinned",
          scope: { kind: "channel", channel: "RemoteParty" },
        },
        {
          human_id: "human-self",
          scope: { kind: "channel", channel: "DirectMic" },
        },
      ],
    });
  });

  test("live capture config sync pushes again after a restart on the same session", async () => {
    vi.useFakeTimers();
    useLiveLanguage("en");
    liveQuerySubscribeMock.mockImplementation(
      async (sql, _params, handlers) => {
        if (!String(sql).includes("FROM transcripts")) {
          handlers.onData([]);
        }
        return async () => {};
      },
    );
    const setLive = (live: Record<string, unknown>) =>
      getListenerStateMock.mockReturnValue({
        setTriggerAppIds: setTriggerAppIdsMock,
        stop: stopMock,
        updateCaptureConfig: updateCaptureConfigMock,
        live,
      });
    const latestTranscriptHandlers = () => {
      const calls = liveQuerySubscribeMock.mock.calls.filter(([sql]) =>
        String(sql).includes("FROM transcripts"),
      );
      const call = calls[calls.length - 1];
      expect(call).toBeDefined();
      return call![2] as { onData: (rows: unknown[]) => void };
    };
    const transcriptRows = [
      {
        ...emptyTranscriptRow(
          [{ id: "w1", text: " hello", start_ms: 0, end_ms: 100, channel: 1 }],
          remoteSpeakerHints,
        ),
        speaker_context: speakerContextJson,
      },
    ];

    await renderLiveCaptureSync();
    const storeListener = listenerSubscribeMock.mock.calls[0]?.[0];
    expect(storeListener).toBeTypeOf("function");

    findLiveQueryHandlers("session_participants").onData(
      participantRows("human-artem"),
    );
    latestTranscriptHandlers().onData(transcriptRows);
    await vi.runOnlyPendingTimersAsync();
    expect(updateCaptureConfigMock).toHaveBeenCalledTimes(1);

    setLive({
      status: "inactive",
      sessionId: null,
      captureGenerationBySession: {},
    });
    storeListener();
    await vi.runOnlyPendingTimersAsync();

    setLive({
      status: "active",
      sessionId: "session-1",
      captureGenerationBySession: { "session-1": 2 },
    });
    storeListener();
    await vi.waitFor(() =>
      expect(liveQuerySubscribeMock).toHaveBeenCalledTimes(3),
    );
    latestTranscriptHandlers().onData(transcriptRows);
    await vi.runOnlyPendingTimersAsync();

    expect(updateCaptureConfigMock).toHaveBeenCalledTimes(2);
    expect(updateCaptureConfigMock.mock.calls[1]?.[0]).toMatchObject({
      speaker_assignments: [artemRemoteAssignment],
    });
  });

  test.each(["notification_confirm", "notification_accept"])(
    "%s opens Team settings for an invitation without creating a session",
    async (type) => {
      const handler = await renderNotificationHandler();

      handler({
        payload: { type, key: "team-invitation:inv-1", source: null },
      });

      expect(createSessionMock).not.toHaveBeenCalled();
      expect(openNewMock).toHaveBeenCalledWith({
        type: "settings",
        state: { tab: "team" },
      });
    },
  );

  test.each([
    ["notification_confirm", "cloudsync-initial-sync-complete-user-1"],
    ["notification_accept", "unknown-informational-notification"],
  ])(
    "%s for %s leaves the current note and recording alone",
    async (type, key) => {
      const handler = await renderNotificationHandler();

      await handler({ payload: { type, key, source: null } });

      expect(stopMock).not.toHaveBeenCalled();
      expect(createSessionMock).not.toHaveBeenCalled();
      expect(openNewMock).not.toHaveBeenCalled();
      expect(setTriggerAppIdsMock).not.toHaveBeenCalled();
    },
  );

  test.each([
    {
      name: "session source",
      key: "batch-completed-session-1",
      source: { type: "session", session_id: "session-1" },
    },
    {
      name: "batch key",
      key: createBatchCompletedNotificationKey("session-1"),
      source: null,
    },
  ])("notification_confirm with $name opens that session", async (payload) => {
    const handler = await renderNotificationHandler();

    handler({
      payload: {
        type: "notification_confirm",
        key: payload.key,
        source: payload.source,
      },
    });

    expect(createSessionMock).not.toHaveBeenCalled();
    expect(openNewMock).toHaveBeenCalledWith({
      type: "sessions",
      id: "session-1",
      state: { view: null, autoStart: null },
    });
  });

  test("notification_confirm with mic_detected source opens detected event and sets triggerAppIds", async () => {
    const handler = await renderNotificationHandler();

    handler({
      payload: {
        type: "notification_confirm",
        source: {
          type: "mic_detected",
          app_names: ["Zoom"],
          app_ids: ["us.zoom.xos"],
          event_ids: ["event-1"],
        },
      },
    });

    await vi.waitFor(() => expect(openNewMock).toHaveBeenCalledTimes(1));

    expect(getOrCreateSessionForEventIdMock).toHaveBeenCalledWith("event-1");
    expect(createSessionMock).not.toHaveBeenCalled();
    expect(setTriggerAppIdsMock).toHaveBeenCalledWith(["us.zoom.xos"]);
    expect(openNewMock).toHaveBeenCalledWith({
      type: "sessions",
      id: "session-event",
      state: { view: null, autoStart: true },
    });
  });

  test("notification_option_selected with mic_detected source sets triggerAppIds", async () => {
    const handler = await renderNotificationHandler();

    handler({
      payload: {
        type: "notification_option_selected",
        selected_index: 0,
        source: {
          type: "mic_detected",
          app_names: ["Zoom"],
          app_ids: ["us.zoom.xos"],
          event_ids: [],
        },
      },
    });

    expect(setTriggerAppIdsMock).toHaveBeenCalledWith(["us.zoom.xos"]);
    await vi.waitFor(() => expect(openNewMock).toHaveBeenCalledTimes(1));
  });

  test.each([
    {
      name: "upcoming",
      now: "2026-05-15T12:00:00.000Z",
      startedAt: "2026-05-15T12:02:00.000Z",
      autoStart: null,
    },
    {
      name: "started",
      now: "2026-05-15T12:02:00.000Z",
      startedAt: "2026-05-15T12:00:00.000Z",
      autoStart: true,
    },
  ])(
    "notification_confirm with $name calendar_event sets autoStart $autoStart",
    async ({ now, startedAt, autoStart }) => {
      vi.spyOn(Date, "now").mockReturnValue(new Date(now).getTime());
      getCalendarEventStartedAtMock.mockResolvedValue(startedAt);
      const handler = await renderNotificationHandler();

      handler({
        payload: {
          type: "notification_confirm",
          source: { type: "calendar_event", event_id: "evt-1" },
        },
      });

      await vi.waitFor(() =>
        expect(openNewMock).toHaveBeenCalledWith({
          type: "sessions",
          id: "session-event",
          state: { view: null, autoStart },
        }),
      );
      expect(openUrlMock).not.toHaveBeenCalled();
      expect(setTriggerAppIdsMock).not.toHaveBeenCalled();
    },
  );

  test("cleans up an updater subscription that resolves after unmount", async () => {
    let resolveUpdater: ((unlisten: () => void) => void) | undefined;
    updaterListenMock.mockReturnValue(
      new Promise<() => void>((resolve) => {
        resolveUpdater = resolve;
      }),
    );
    const unlisten = vi.fn();

    const { unmount } = render(<EventListeners />);
    await vi.waitFor(() => expect(updaterListenMock).toHaveBeenCalledOnce());
    unmount();
    resolveUpdater?.(unlisten);

    await vi.waitFor(() => expect(unlisten).toHaveBeenCalledOnce());
    expect(maybeEmitUpdatedMock).not.toHaveBeenCalled();
  });
});
