import { render } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";

import { EventListeners } from "./event-listeners";

import { createAutoStopEndedNotificationKey } from "~/stt/auto-stop-notification";
import { createBatchCompletedNotificationKey } from "~/stt/batch-completed-notification";

const {
  notificationListenMock,
  updaterListenMock,
  maybeEmitUpdatedMock,
  getCurrentWebviewWindowLabelMock,
  useMainStoreMock,
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
  useMainStoreMock: vi.fn(() => null),
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

vi.mock("@hypr/plugin-notification", () => ({
  events: {
    notificationEvent: {
      listen: notificationListenMock,
    },
  },
}));

vi.mock("@hypr/plugin-updater2", () => ({
  commands: {
    maybeEmitUpdated: maybeEmitUpdatedMock,
  },
  events: {
    updatedEvent: {
      listen: updaterListenMock,
    },
  },
}));

vi.mock("@hypr/plugin-windows", () => ({
  getCurrentWebviewWindowLabel: getCurrentWebviewWindowLabelMock,
}));

vi.mock("~/store/tinybase/store/main", () => ({
  STORE_ID: "main-store",
  UI: {
    useStore: useMainStoreMock,
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
  },
}));

function createMainStore(overrides: Record<string, unknown> = {}) {
  return {
    addTableListener: vi.fn(() => "main-listener"),
    delListener: vi.fn(),
    forEachRow: vi.fn(),
    getValue: vi.fn(),
    ...overrides,
  };
}

describe("EventListeners notification events", () => {
  beforeEach(() => {
    notificationListenMock.mockReset();
    updaterListenMock.mockReset();
    maybeEmitUpdatedMock.mockReset();
    getCurrentWebviewWindowLabelMock.mockReset();
    useMainStoreMock.mockReset();
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
    useMainStoreMock.mockReturnValue(null);
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
      live: { status: "active", sessionId: "session-1" },
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
    useMainStoreMock.mockReturnValue(createMainStore() as never);

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

  test("live capture config sync mounts without auth providers", async () => {
    vi.useFakeTimers();

    const mainStore = {
      addTableListener: vi.fn(() => "main-listener"),
      delListener: vi.fn(),
      forEachRow: vi.fn(),
      getValue: vi.fn((key: string) =>
        key === "user_id" ? "human-self" : undefined,
      ),
    };
    useMainStoreMock.mockReturnValue(mainStore as never);
    useConfigValuesMock.mockReturnValue({
      ai_language: "ko",
      spoken_languages: ["ko"],
      current_stt_provider: "soniox",
      current_stt_model: "stt-v4",
    });

    render(<EventListeners />);

    expect(mainStore.addTableListener).toHaveBeenCalledWith(
      "mapping_session_participant",
      expect.any(Function),
    );
    await vi.runOnlyPendingTimersAsync();

    expect(updateCaptureConfigMock).toHaveBeenCalledWith({
      session_id: "session-1",
      languages: ["ko"],
      participant_human_ids: [],
      self_human_id: "human-self",
    });
  });

  test("notification_confirm with auto-stop prompt ignores collapsed body click", async () => {
    useMainStoreMock.mockReturnValue(createMainStore() as never);

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
    useMainStoreMock.mockReturnValue(null);

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
    useMainStoreMock.mockReturnValue(createMainStore() as never);

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
    useMainStoreMock.mockReturnValue(createMainStore() as never);

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
    useMainStoreMock.mockReturnValue(createMainStore() as never);

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
    useMainStoreMock.mockReturnValue(null);

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
});
