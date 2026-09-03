import { render } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";

import { EventListeners } from "./event-listeners";

import {
  cancelAutoStopEndedNotification,
  createAutoStopEndedNotificationKey,
} from "~/stt/auto-stop-notification";
import { createBatchCompletedNotificationKey } from "~/stt/batch-completed-notification";

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

    render(<EventListeners />);

    await vi.waitFor(() =>
      expect(notificationListenMock).toHaveBeenCalledTimes(1),
    );

    const handler = notificationListenMock.mock.calls[0]?.[0];
    expect(handler).toBeTypeOf("function");

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

  test("notification_accept with auto-stop prompt stops the active session", async () => {
    render(<EventListeners />);

    await vi.waitFor(() =>
      expect(notificationListenMock).toHaveBeenCalledTimes(1),
    );

    const handler = notificationListenMock.mock.calls[0]?.[0];
    expect(handler).toBeTypeOf("function");

    handler({
      payload: {
        type: "notification_accept",
        key: createAutoStopEndedNotificationKey("session-1"),
        source: null,
      },
    });

    expect(stopMock).toHaveBeenCalledTimes(1);
    expect(createSessionMock).not.toHaveBeenCalled();
    expect(openNewMock).not.toHaveBeenCalled();
  });

  test("notification_timeout with auto-stop prompt stops the active session", async () => {
    render(<EventListeners />);

    await vi.waitFor(() =>
      expect(notificationListenMock).toHaveBeenCalledTimes(1),
    );

    const handler = notificationListenMock.mock.calls[0]?.[0];
    expect(handler).toBeTypeOf("function");

    handler({
      payload: {
        type: "notification_timeout",
        key: createAutoStopEndedNotificationKey("session-1"),
        source: null,
      },
    });

    expect(stopMock).toHaveBeenCalledTimes(1);
    expect(createSessionMock).not.toHaveBeenCalled();
    expect(openNewMock).not.toHaveBeenCalled();
  });

  test("notification_timeout ignores a stale auto-stop session", async () => {
    render(<EventListeners />);

    await vi.waitFor(() =>
      expect(notificationListenMock).toHaveBeenCalledTimes(1),
    );

    const handler = notificationListenMock.mock.calls[0]?.[0];
    expect(handler).toBeTypeOf("function");

    handler({
      payload: {
        type: "notification_timeout",
        key: createAutoStopEndedNotificationKey("session-old"),
        source: null,
      },
    });

    expect(stopMock).not.toHaveBeenCalled();
    expect(createSessionMock).not.toHaveBeenCalled();
    expect(openNewMock).not.toHaveBeenCalled();
  });

  test("live capture config sync mounts without auth providers", async () => {
    vi.useFakeTimers();
    useConfigValuesMock.mockReturnValue({
      ai_language: "ko",
      spoken_languages: ["ko"],
      current_stt_provider: "soniox",
      current_stt_model: "stt-v4",
    });

    render(<EventListeners />);

    await vi.waitFor(() =>
      expect(liveQuerySubscribeMock).toHaveBeenCalledTimes(2),
    );
    const handlers = findLiveQueryHandlers("session_participants");
    handlers.onData([
      {
        session_id: "session-1",
        owner_user_id: "human-self",
        human_id: "human-remote",
      },
    ]);
    await vi.runOnlyPendingTimersAsync();

    expect(updateCaptureConfigMock).toHaveBeenCalledWith({
      session_id: "session-1",
      languages: ["ko"],
      participant_human_ids: ["human-remote"],
      self_human_id: "human-self",
      speaker_assignments: [],
    });
  });

  test("live capture config sync waits for the transcript snapshot before pushing", async () => {
    vi.useFakeTimers();
    useConfigValuesMock.mockReturnValue({
      ai_language: "ko",
      spoken_languages: ["ko"],
      current_stt_provider: "soniox",
      current_stt_model: "stt-v4",
    });
    // The transcript query answers later than the participant query here.
    liveQuerySubscribeMock.mockImplementation(
      async (sql, _params, handlers) => {
        if (!String(sql).includes("FROM transcripts")) {
          handlers.onData([]);
        }
        return async () => {};
      },
    );

    render(<EventListeners />);

    await vi.waitFor(() =>
      expect(liveQuerySubscribeMock).toHaveBeenCalledTimes(2),
    );
    findLiveQueryHandlers("session_participants").onData([
      {
        session_id: "session-1",
        owner_user_id: "human-self",
        human_id: "human-remote",
      },
    ]);
    await vi.runOnlyPendingTimersAsync();

    expect(updateCaptureConfigMock).not.toHaveBeenCalled();

    findLiveQueryHandlers("FROM transcripts").onData([]);
    await vi.runOnlyPendingTimersAsync();

    expect(updateCaptureConfigMock).toHaveBeenCalledTimes(1);
    expect(updateCaptureConfigMock).toHaveBeenCalledWith(
      expect.objectContaining({
        session_id: "session-1",
        speaker_assignments: [],
      }),
    );
  });

  test("live capture config sync runs without names when the transcript read fails", async () => {
    vi.useFakeTimers();
    vi.spyOn(console, "error").mockImplementation(() => {});
    useConfigValuesMock.mockReturnValue({
      ai_language: "ko",
      spoken_languages: ["ko"],
      current_stt_provider: "soniox",
      current_stt_model: "stt-v4",
    });
    liveQuerySubscribeMock.mockImplementation(
      async (sql, _params, handlers) => {
        if (String(sql).includes("FROM transcripts")) {
          handlers.onError("no such table: transcripts");
        } else {
          handlers.onData([]);
        }
        return async () => {};
      },
    );

    render(<EventListeners />);

    await vi.waitFor(() =>
      expect(liveQuerySubscribeMock).toHaveBeenCalledTimes(2),
    );
    findLiveQueryHandlers("session_participants").onData([
      {
        session_id: "session-1",
        owner_user_id: "human-self",
        human_id: "human-remote",
      },
    ]);
    await vi.runOnlyPendingTimersAsync();

    expect(updateCaptureConfigMock).toHaveBeenCalledTimes(1);
    expect(updateCaptureConfigMock).toHaveBeenCalledWith({
      session_id: "session-1",
      languages: ["ko"],
      participant_human_ids: ["human-remote"],
      self_human_id: "human-self",
      speaker_assignments: [],
    });
  });

  test("live capture config sync runs without names when the transcript subscription rejects", async () => {
    vi.useFakeTimers();
    vi.spyOn(console, "error").mockImplementation(() => {});
    useConfigValuesMock.mockReturnValue({
      ai_language: "ko",
      spoken_languages: ["ko"],
      current_stt_provider: "soniox",
      current_stt_model: "stt-v4",
    });
    liveQuerySubscribeMock.mockImplementation(
      async (sql, _params, handlers) => {
        if (String(sql).includes("FROM transcripts")) {
          throw new Error("subscribe failed");
        }
        handlers.onData([]);
        return async () => {};
      },
    );

    render(<EventListeners />);

    await vi.waitFor(() =>
      expect(liveQuerySubscribeMock).toHaveBeenCalledTimes(2),
    );
    findLiveQueryHandlers("session_participants").onData([
      {
        session_id: "session-1",
        owner_user_id: "human-self",
        human_id: "human-remote",
      },
    ]);
    await vi.runOnlyPendingTimersAsync();

    expect(updateCaptureConfigMock).toHaveBeenCalledTimes(1);
    expect(updateCaptureConfigMock).toHaveBeenCalledWith(
      expect.objectContaining({
        session_id: "session-1",
        participant_human_ids: ["human-remote"],
        speaker_assignments: [],
      }),
    );
  });

  test("live capture config sync pushes the active transcript's speaker assignments", async () => {
    vi.useFakeTimers();
    useConfigValuesMock.mockReturnValue({
      ai_language: "en",
      spoken_languages: ["en"],
      current_stt_provider: "soniox",
      current_stt_model: "stt-v4",
    });

    render(<EventListeners />);

    await vi.waitFor(() =>
      expect(liveQuerySubscribeMock).toHaveBeenCalledTimes(2),
    );
    const transcriptCall = liveQuerySubscribeMock.mock.calls.find(([sql]) =>
      String(sql).includes("FROM transcripts"),
    );
    expect(transcriptCall?.[1]).toEqual(["session-1"]);

    findLiveQueryHandlers("session_participants").onData([
      {
        session_id: "session-1",
        owner_user_id: "human-self",
        human_id: "human-artem",
      },
      {
        session_id: "session-1",
        owner_user_id: "human-self",
        human_id: "human-guest",
      },
    ]);
    findLiveQueryHandlers("FROM transcripts").onData([
      {
        id: "transcript-1",
        started_at_ms: 1_000,
        words_json: JSON.stringify([
          { id: "w1", text: " hello", start_ms: 0, end_ms: 100, channel: 1 },
          { id: "w2", text: " there", start_ms: 100, end_ms: 200, channel: 1 },
        ]),
        speaker_hints_json: JSON.stringify([
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
        ]),
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
        {
          human_id: "human-artem",
          scope: {
            kind: "channel_speaker",
            channel: "RemoteParty",
            speaker_index: 0,
          },
        },
        {
          human_id: "human-guest",
          scope: { kind: "words", word_ids: ["w2"] },
        },
      ],
    });
  });

  test("live capture config sync pushes again after a restart on the same session", async () => {
    vi.useFakeTimers();
    useConfigValuesMock.mockReturnValue({
      ai_language: "en",
      spoken_languages: ["en"],
      current_stt_provider: "soniox",
      current_stt_model: "stt-v4",
    });
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
        id: "transcript-1",
        started_at_ms: 1_000,
        words_json: JSON.stringify([
          { id: "w1", text: " hello", start_ms: 0, end_ms: 100, channel: 1 },
        ]),
        speaker_hints_json: JSON.stringify([
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
        ]),
      },
    ];

    render(<EventListeners />);

    await vi.waitFor(() =>
      expect(liveQuerySubscribeMock).toHaveBeenCalledTimes(2),
    );
    const storeListener = listenerSubscribeMock.mock.calls[0]?.[0];
    expect(storeListener).toBeTypeOf("function");

    findLiveQueryHandlers("session_participants").onData([
      {
        session_id: "session-1",
        owner_user_id: "human-self",
        human_id: "human-artem",
      },
    ]);
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
    expect(updateCaptureConfigMock.mock.calls[1]?.[0]).toEqual(
      updateCaptureConfigMock.mock.calls[0]?.[0],
    );
    expect(updateCaptureConfigMock.mock.calls[1]?.[0]).toMatchObject({
      speaker_assignments: [
        {
          human_id: "human-artem",
          scope: {
            kind: "channel_speaker",
            channel: "RemoteParty",
            speaker_index: 0,
          },
        },
      ],
    });
  });

  test("notification_confirm with auto-stop prompt ignores collapsed body click", async () => {
    render(<EventListeners />);

    await vi.waitFor(() =>
      expect(notificationListenMock).toHaveBeenCalledTimes(1),
    );

    const handler = notificationListenMock.mock.calls[0]?.[0];
    expect(handler).toBeTypeOf("function");

    handler({
      payload: {
        type: "notification_confirm",
        key: createAutoStopEndedNotificationKey("session-1"),
        source: null,
      },
    });

    expect(stopMock).not.toHaveBeenCalled();
    expect(createSessionMock).not.toHaveBeenCalled();
    expect(openNewMock).not.toHaveBeenCalled();
  });

  test("notification_confirm with session source opens that session", async () => {
    render(<EventListeners />);

    await vi.waitFor(() =>
      expect(notificationListenMock).toHaveBeenCalledTimes(1),
    );

    const handler = notificationListenMock.mock.calls[0]?.[0];
    expect(handler).toBeTypeOf("function");

    handler({
      payload: {
        type: "notification_confirm",
        key: "batch-completed-session-1",
        source: { type: "session", session_id: "session-1" },
      },
    });

    expect(createSessionMock).not.toHaveBeenCalled();
    expect(openNewMock).toHaveBeenCalledWith({
      type: "sessions",
      id: "session-1",
      state: { view: null, autoStart: null },
    });
  });

  test("notification_confirm with batch key opens that session without source", async () => {
    render(<EventListeners />);

    await vi.waitFor(() =>
      expect(notificationListenMock).toHaveBeenCalledTimes(1),
    );

    const handler = notificationListenMock.mock.calls[0]?.[0];
    expect(handler).toBeTypeOf("function");

    handler({
      payload: {
        type: "notification_confirm",
        key: createBatchCompletedNotificationKey("session-1"),
        source: null,
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
    render(<EventListeners />);

    await vi.waitFor(() =>
      expect(notificationListenMock).toHaveBeenCalledTimes(1),
    );

    const handler = notificationListenMock.mock.calls[0]?.[0];
    expect(handler).toBeTypeOf("function");

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
    render(<EventListeners />);

    await vi.waitFor(() =>
      expect(notificationListenMock).toHaveBeenCalledTimes(1),
    );

    const handler = notificationListenMock.mock.calls[0]?.[0];
    expect(handler).toBeTypeOf("function");

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

  test("notification_confirm opens without waiting for the legacy store", async () => {
    render(<EventListeners />);

    await vi.waitFor(() =>
      expect(notificationListenMock).toHaveBeenCalledTimes(1),
    );

    const handler = notificationListenMock.mock.calls[0]?.[0];
    expect(handler).toBeTypeOf("function");

    handler({
      payload: {
        type: "notification_confirm",
        source: {
          type: "mic_detected",
          app_names: ["Zoom"],
          app_ids: ["us.zoom.xos"],
          event_ids: [],
        },
      },
    });

    await vi.waitFor(() =>
      expect(setTriggerAppIdsMock).toHaveBeenCalledWith(["us.zoom.xos"]),
    );
    expect(openNewMock).toHaveBeenCalledTimes(1);
  });

  test("notification_confirm with upcoming calendar_event opens notes without auto-start", async () => {
    vi.spyOn(Date, "now").mockReturnValue(
      new Date("2026-05-15T12:00:00.000Z").getTime(),
    );
    getCalendarEventStartedAtMock.mockResolvedValue("2026-05-15T12:02:00.000Z");

    render(<EventListeners />);

    await vi.waitFor(() =>
      expect(notificationListenMock).toHaveBeenCalledTimes(1),
    );

    const handler = notificationListenMock.mock.calls[0]?.[0];
    expect(handler).toBeTypeOf("function");

    handler({
      payload: {
        type: "notification_confirm",
        source: { type: "calendar_event", event_id: "evt-1" },
      },
    });

    await vi.waitFor(() => expect(openNewMock).toHaveBeenCalledTimes(1));
    expect(setTriggerAppIdsMock).not.toHaveBeenCalled();
    expect(openNewMock).toHaveBeenCalledWith({
      type: "sessions",
      id: "session-event",
      state: { view: null, autoStart: null },
    });
  });

  test("notification_confirm with started calendar_event starts listening", async () => {
    vi.spyOn(Date, "now").mockReturnValue(
      new Date("2026-05-15T12:02:00.000Z").getTime(),
    );
    getCalendarEventStartedAtMock.mockResolvedValue("2026-05-15T12:00:00.000Z");

    render(<EventListeners />);

    await vi.waitFor(() =>
      expect(notificationListenMock).toHaveBeenCalledTimes(1),
    );

    const handler = notificationListenMock.mock.calls[0]?.[0];
    expect(handler).toBeTypeOf("function");

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
        state: { view: null, autoStart: true },
      }),
    );
  });

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
