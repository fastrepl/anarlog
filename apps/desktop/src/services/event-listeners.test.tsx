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
  useSettingsStoreMock,
  openNewMock,
  createSessionMock,
  getOrCreateSessionForEventIdMock,
  setTriggerAppIdsMock,
  stopMock,
  getListenerStateMock,
} = vi.hoisted(() => ({
  notificationListenMock: vi.fn(),
  updaterListenMock: vi.fn(),
  maybeEmitUpdatedMock: vi.fn(),
  getCurrentWebviewWindowLabelMock: vi.fn(() => "main"),
  useMainStoreMock: vi.fn(() => null),
  useSettingsStoreMock: vi.fn(() => null),
  openNewMock: vi.fn(),
  createSessionMock: vi.fn(() => "session-new"),
  getOrCreateSessionForEventIdMock: vi.fn(() => "session-event"),
  setTriggerAppIdsMock: vi.fn(),
  stopMock: vi.fn(),
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

vi.mock("~/store/tinybase/store/settings", () => ({
  STORE_ID: "settings-store",
  UI: {
    useStore: useSettingsStoreMock,
  },
}));

vi.mock("~/store/tinybase/store/sessions", () => ({
  createSession: createSessionMock,
  getOrCreateSessionForEventId: getOrCreateSessionForEventIdMock,
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

describe("EventListeners notification events", () => {
  beforeEach(() => {
    notificationListenMock.mockReset();
    updaterListenMock.mockReset();
    maybeEmitUpdatedMock.mockReset();
    getCurrentWebviewWindowLabelMock.mockReset();
    useMainStoreMock.mockReset();
    useSettingsStoreMock.mockReset();
    openNewMock.mockReset();
    createSessionMock.mockReset();
    getOrCreateSessionForEventIdMock.mockReset();
    setTriggerAppIdsMock.mockReset();
    stopMock.mockReset();
    getListenerStateMock.mockReset();

    getCurrentWebviewWindowLabelMock.mockReturnValue("main");
    notificationListenMock.mockResolvedValue(() => {});
    updaterListenMock.mockResolvedValue(() => {});
    createSessionMock.mockReturnValue("session-new");
    getOrCreateSessionForEventIdMock.mockReturnValue("session-event");
    useMainStoreMock.mockReturnValue(null);
    useSettingsStoreMock.mockReturnValue(null);
    getListenerStateMock.mockReturnValue({
      setTriggerAppIds: setTriggerAppIdsMock,
      stop: stopMock,
      live: { status: "active", sessionId: "session-1" },
    });
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  test("stores mic-detected footer actions as ignored platforms", async () => {
    const settingsStore = {
      getValue: vi.fn(() => JSON.stringify(["com.existing.app"])),
      setValue: vi.fn(),
    };
    useSettingsStoreMock.mockReturnValue(settingsStore as never);

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

    expect(settingsStore.setValue).toHaveBeenCalledWith(
      "ignored_platforms",
      JSON.stringify(["com.existing.app", "us.zoom.xos"]),
    );
    expect(openNewMock).not.toHaveBeenCalled();
  });

  test("notification_accept with auto-stop prompt stops the active session", async () => {
    useMainStoreMock.mockReturnValue({} as never);

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

  test("notification_confirm with auto-stop prompt ignores collapsed body click", async () => {
    useMainStoreMock.mockReturnValue({} as never);

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
    useMainStoreMock.mockReturnValue({} as never);

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

  test("notification_confirm with mic_detected source sets triggerAppIds (regression: #5120 confirm path)", async () => {
    useMainStoreMock.mockReturnValue({} as never);

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

    expect(setTriggerAppIdsMock).toHaveBeenCalledWith(["us.zoom.xos"]);
    expect(openNewMock).toHaveBeenCalledTimes(1);
  });

  test("notification_option_selected with mic_detected source sets triggerAppIds", async () => {
    useMainStoreMock.mockReturnValue({} as never);

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
    expect(openNewMock).toHaveBeenCalledTimes(1);
  });

  test("notification_confirm with mic_detected source preserves triggerAppIds across pending-auto-start (regression: bugbot follow-up)", async () => {
    useMainStoreMock.mockReturnValue(null);

    const { rerender } = render(<EventListeners />);

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

    expect(setTriggerAppIdsMock).not.toHaveBeenCalled();
    expect(openNewMock).not.toHaveBeenCalled();

    useMainStoreMock.mockReturnValue({} as never);
    rerender(<EventListeners />);

    await vi.waitFor(() =>
      expect(setTriggerAppIdsMock).toHaveBeenCalledWith(["us.zoom.xos"]),
    );
    expect(openNewMock).toHaveBeenCalledTimes(1);
  });

  test("notification_confirm with upcoming calendar_event opens notes without auto-start", async () => {
    vi.spyOn(Date, "now").mockReturnValue(
      new Date("2026-05-15T12:00:00.000Z").getTime(),
    );
    useMainStoreMock.mockReturnValue({
      getRow: vi.fn((table: string, rowId: string) =>
        table === "events" && rowId === "evt-1"
          ? { started_at: "2026-05-15T12:02:00.000Z" }
          : undefined,
      ),
    } as never);

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
    useMainStoreMock.mockReturnValue({
      getRow: vi.fn((table: string, rowId: string) =>
        table === "events" && rowId === "evt-1"
          ? { started_at: "2026-05-15T12:00:00.000Z" }
          : undefined,
      ),
    } as never);

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

    expect(openNewMock).toHaveBeenCalledWith({
      type: "sessions",
      id: "session-event",
      state: { view: null, autoStart: true },
    });
  });
});
