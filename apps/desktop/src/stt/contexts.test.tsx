import { resolveResource } from "@tauri-apps/api/path";
import { cleanup, render } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";

import {
  cancelAutoStopEndedNotification,
  consumeAutoStopEndedNotificationKey,
  createAutoStopEndedNotificationKey,
  parseAutoStopEndedNotificationKey,
} from "./auto-stop-notification";
import {
  AUTO_STOP_CONFIRM_DELAY_MS,
  AUTO_STOP_EVENT_END_GRACE_MS,
  AUTO_STOP_NETWORK_HOLD_MS,
  AUTO_STOP_RECENT_OFFLINE_MS,
  ListenerProvider,
} from "./contexts";

import { createListenerStore } from "~/store/zustand/listener";

const {
  listMicUsingApplicationsMock,
  inspectMeetingAccessibilityMock,
  listenMock,
  clearNotificationsMock,
  showNotificationMock,
  useStoreMock,
  useConfigValueMock,
  getNearbyCalendarEventsMock,
  loadSessionEventMock,
} = vi.hoisted(() => ({
  listMicUsingApplicationsMock: vi.fn(),
  inspectMeetingAccessibilityMock: vi.fn(),
  listenMock: vi.fn(),
  clearNotificationsMock: vi.fn(),
  showNotificationMock: vi.fn(),
  useStoreMock: vi.fn(() => null),
  useConfigValueMock: vi.fn((key: string) => key !== "notification_disabled"),
  getNearbyCalendarEventsMock: vi.fn(),
  loadSessionEventMock: vi.fn(),
}));

vi.mock("@anlg/plugin-detect", () => ({
  commands: {
    inspectMeetingAccessibility: inspectMeetingAccessibilityMock,
    listMicUsingApplications: listMicUsingApplicationsMock,
  },
  events: {
    detectEvent: {
      listen: listenMock,
    },
  },
}));

vi.mock("@anlg/plugin-notification", () => ({
  commands: {
    clearNotifications: clearNotificationsMock,
    showNotification: showNotificationMock,
  },
}));

vi.mock("~/calendar/queries", () => ({
  getNearbyCalendarEvents: getNearbyCalendarEventsMock,
}));

vi.mock("~/session/queries", () => ({
  loadSessionEvent: loadSessionEventMock,
}));

vi.mock("~/shared/config", () => ({
  useConfigValue: useConfigValueMock,
}));

function setStoreActive(
  store: ReturnType<typeof createListenerStore>,
  sessionId = "session-1",
) {
  store.setState((state) => ({
    live: { ...state.live, sessionId, status: "active" },
  }));
}

async function renderProvider(store: ReturnType<typeof createListenerStore>) {
  render(
    <ListenerProvider store={store}>
      <div>child</div>
    </ListenerProvider>,
  );
  await vi.waitFor(() => expect(listenMock).toHaveBeenCalledTimes(1));
  return listenMock.mock.calls[0]?.[0];
}

function mockSessionEventStore(event: {
  started_at: string;
  ended_at: string;
  is_all_day?: boolean;
}) {
  return {
    getRow: vi.fn((table: string, rowId: string) =>
      table === "sessions" && rowId === "session-1"
        ? {
            event_json: JSON.stringify({
              tracking_id: "tracking-1",
              calendar_id: "calendar-1",
              title: "Design sync",
              has_recurrence_rules: false,
              ...event,
            }),
          }
        : undefined,
    ),
    forEachRow: vi.fn(),
  };
}

function mockNearbyEventStore(event: {
  id?: string;
  title?: string;
  started_at: string;
  meeting_link?: string;
  location?: string;
  description?: string;
  participants_json?: string;
  is_all_day?: boolean;
}) {
  return mockNearbyEventStoreMany([event]);
}

function mockNearbyEventStoreMany(
  events: Array<{
    id?: string;
    title?: string;
    started_at: string;
    meeting_link?: string;
    location?: string;
    description?: string;
    participants_json?: string;
    is_all_day?: boolean;
  }>,
) {
  return {
    getRow: vi.fn((table: string, rowId: string) => {
      const event = events.find(
        (event, index) => (event.id ?? `event-${index + 1}`) === rowId,
      );
      if (table !== "events" || !event) {
        return undefined;
      }

      return {
        title: event.title ?? "Design sync",
        started_at: event.started_at,
        meeting_link: event.meeting_link,
        location: event.location,
        description: event.description,
        participants_json: event.participants_json,
        is_all_day: event.is_all_day ?? false,
      };
    }),
    forEachRow: vi.fn((table: string, callback: (rowId: string) => void) => {
      if (table === "events") {
        events.forEach((event, index) =>
          callback(event.id ?? `event-${index + 1}`),
        );
      }
    }),
  };
}

async function readConfiguredSessionEvent(sessionId: string) {
  const store = useStoreMock() as any;
  const row = store?.getRow?.("sessions", sessionId);
  if (!row?.event_json) return null;
  return JSON.parse(row.event_json);
}

async function readConfiguredNearbyEvents(nowMs: number, windowMs: number) {
  const store = useStoreMock() as any;
  if (!store) return [];

  const rows: Array<{
    id: string;
    title: string;
    meetingLink?: string;
    location?: string;
    description?: string;
    participantNames: string[];
    startedAt: number;
  }> = [];
  store.forEachRow?.("events", (eventId: string) => {
    const event = store.getRow?.("events", eventId);
    if (!event?.started_at || event.is_all_day) return;
    const startedAt = new Date(event.started_at).getTime();
    if (Number.isNaN(startedAt) || Math.abs(startedAt - nowMs) > windowMs) {
      return;
    }

    let participants: Array<{ name?: string; is_current_user?: boolean }> = [];
    try {
      const parsed = JSON.parse(event.participants_json || "[]");
      if (Array.isArray(parsed)) participants = parsed;
    } catch {}

    rows.push({
      id: eventId,
      title: event.title || "Untitled Event",
      meetingLink: event.meeting_link || undefined,
      location: event.location || undefined,
      description: event.description || undefined,
      participantNames: [
        ...new Set(
          participants
            .filter((participant) => !participant.is_current_user)
            .map((participant) => participant.name?.trim() || "")
            .filter(Boolean),
        ),
      ],
      startedAt,
    });
  });

  rows.sort(
    (a, b) =>
      Math.abs(a.startedAt - nowMs) - Math.abs(b.startedAt - nowMs) ||
      a.startedAt - b.startedAt,
  );
  return rows.map(({ startedAt: _startedAt, ...event }) => event);
}

describe("ListenerProvider detect events", () => {
  beforeEach(() => {
    cancelAutoStopEndedNotification("session-1");
    listenMock.mockReset();
    clearNotificationsMock.mockReset();
    showNotificationMock.mockReset();
    useStoreMock.mockReset();
    useConfigValueMock.mockReset();
    getNearbyCalendarEventsMock.mockReset();
    loadSessionEventMock.mockReset();
    useStoreMock.mockReturnValue(null);
    useConfigValueMock.mockImplementation(
      (key: string) => key !== "notification_disabled",
    );
    getNearbyCalendarEventsMock.mockImplementation(readConfiguredNearbyEvents);
    loadSessionEventMock.mockImplementation(readConfiguredSessionEvent);
    listenMock.mockResolvedValue(() => {});
    inspectMeetingAccessibilityMock.mockReset().mockResolvedValue({
      status: "ok",
      data: [],
    });
    listMicUsingApplicationsMock.mockResolvedValue({ status: "ok", data: [] });
    Object.defineProperty(window.navigator, "onLine", {
      configurable: true,
      value: true,
    });
    vi.useRealTimers();
  });

  afterEach(() => {
    cleanup();
    vi.clearAllTimers();
    vi.useRealTimers();
  });

  test("invalidates and clears pending notifications when listening becomes active", async () => {
    const store = createListenerStore();

    render(
      <ListenerProvider store={store}>
        <div>child</div>
      </ListenerProvider>,
    );

    expect(clearNotificationsMock).not.toHaveBeenCalled();
    const notificationKey = createAutoStopEndedNotificationKey("session-1");

    setStoreActive(store);

    await vi.waitFor(() =>
      expect(clearNotificationsMock).toHaveBeenCalledTimes(1),
    );
    expect(consumeAutoStopEndedNotificationKey(notificationKey)).toBeNull();
  });

  test("does not stop listening on MicStopped when no trigger apps are set (manual session — regression: #5120)", async () => {
    const store = createListenerStore();
    const stopSpy = vi.fn();

    store.setState({ stop: stopSpy });

    render(
      <ListenerProvider store={store}>
        <div>child</div>
      </ListenerProvider>,
    );

    await vi.waitFor(() => expect(listenMock).toHaveBeenCalledTimes(1));

    const handler = listenMock.mock.calls[0]?.[0];
    expect(handler).toBeTypeOf("function");

    handler({
      payload: {
        type: "micStopped",
        apps: [
          { id: "/opt/homebrew/bin/ffmpeg", name: "ffmpeg" },
          { id: "us.zoom.xos", name: "Zoom" },
        ],
      },
    });

    expect(stopSpy).not.toHaveBeenCalled();
  });

  test("stops listening after confirming a trigger app remains stopped", async () => {
    const store = createListenerStore();
    const stopSpy = vi.fn();

    store.setState({ stop: stopSpy });
    store.getState().setTriggerAppIds(["us.zoom.xos"]);
    setStoreActive(store);

    render(
      <ListenerProvider store={store}>
        <div>child</div>
      </ListenerProvider>,
    );

    await vi.waitFor(() => expect(listenMock).toHaveBeenCalledTimes(1));

    const handler = listenMock.mock.calls[0]?.[0];
    expect(handler).toBeTypeOf("function");

    vi.useFakeTimers();
    listMicUsingApplicationsMock.mockClear();

    handler({
      payload: {
        type: "micStopped",
        apps: [{ id: "us.zoom.xos", name: "Zoom" }],
      },
    });

    expect(stopSpy).not.toHaveBeenCalled();

    await vi.advanceTimersByTimeAsync(AUTO_STOP_CONFIRM_DELAY_MS);

    expect(listMicUsingApplicationsMock).toHaveBeenCalledTimes(1);
    expect(stopSpy).toHaveBeenCalledTimes(1);
  });

  test("does not stop when a trigger app resumes during the auto-stop grace period", async () => {
    const store = createListenerStore();
    const stopSpy = vi.fn();

    store.setState({ stop: stopSpy });
    store.getState().setTriggerAppIds(["us.zoom.xos"]);
    setStoreActive(store);
    listMicUsingApplicationsMock.mockResolvedValue({
      status: "ok",
      data: [{ id: "us.zoom.xos", name: "Zoom" }],
    });

    render(
      <ListenerProvider store={store}>
        <div>child</div>
      </ListenerProvider>,
    );

    await vi.waitFor(() => expect(listenMock).toHaveBeenCalledTimes(1));

    const handler = listenMock.mock.calls[0]?.[0];
    expect(handler).toBeTypeOf("function");

    vi.useFakeTimers();
    listMicUsingApplicationsMock.mockClear();

    handler({
      payload: {
        type: "micStopped",
        apps: [{ id: "us.zoom.xos", name: "Zoom" }],
      },
    });

    await vi.advanceTimersByTimeAsync(AUTO_STOP_CONFIRM_DELAY_MS);

    expect(listMicUsingApplicationsMock).toHaveBeenCalledTimes(1);
    expect(stopSpy).not.toHaveBeenCalled();
  });

  test("holds a network-interrupted meeting until its event end grace expires", async () => {
    const store = createListenerStore();
    const stopSpy = vi.fn();
    const now = new Date("2026-05-19T10:05:00.000Z");
    const endedAtMs = new Date("2026-05-19T10:30:00.000Z").getTime();
    const deadlineMs = endedAtMs + AUTO_STOP_EVENT_END_GRACE_MS;

    store.setState({ stop: stopSpy });
    store.getState().setTriggerAppIds(["us.zoom.xos"]);
    setStoreActive(store);
    (useStoreMock as any).mockReturnValue(
      mockSessionEventStore({
        started_at: "2026-05-19T10:00:00.000Z",
        ended_at: "2026-05-19T10:30:00.000Z",
      }),
    );

    vi.useFakeTimers();
    vi.setSystemTime(now);

    render(
      <ListenerProvider store={store}>
        <div>child</div>
      </ListenerProvider>,
    );

    await vi.waitFor(() => expect(listenMock).toHaveBeenCalledTimes(1));
    const handler = listenMock.mock.calls[0]?.[0];

    window.dispatchEvent(new Event("offline"));
    handler({
      payload: {
        type: "micStopped",
        apps: [{ id: "us.zoom.xos", name: "Zoom" }],
      },
    });
    window.dispatchEvent(new Event("online"));

    await vi.advanceTimersByTimeAsync(AUTO_STOP_CONFIRM_DELAY_MS);

    expect(stopSpy).not.toHaveBeenCalled();

    await vi.advanceTimersByTimeAsync(deadlineMs - Date.now() - 1);
    expect(stopSpy).not.toHaveBeenCalled();
    expect(showNotificationMock).not.toHaveBeenCalled();

    await vi.advanceTimersByTimeAsync(1);
    expect(stopSpy).not.toHaveBeenCalled();
    expect(showNotificationMock).toHaveBeenCalledWith(
      expect.objectContaining({
        key: expect.stringContaining("auto-stop-ended:session-1"),
      }),
    );
  });

  test("cancels a network interruption hold when the meeting resumes during event grace", async () => {
    const store = createListenerStore();
    const stopSpy = vi.fn();
    const now = new Date("2026-05-19T10:05:00.000Z");
    const endedAtMs = new Date("2026-05-19T10:30:00.000Z").getTime();
    const deadlineMs = endedAtMs + AUTO_STOP_EVENT_END_GRACE_MS;

    store.setState({ stop: stopSpy });
    store.getState().setTriggerAppIds(["us.zoom.xos"]);
    setStoreActive(store);
    (useStoreMock as any).mockReturnValue(
      mockSessionEventStore({
        started_at: "2026-05-19T10:00:00.000Z",
        ended_at: "2026-05-19T10:30:00.000Z",
      }),
    );

    vi.useFakeTimers();
    vi.setSystemTime(now);

    render(
      <ListenerProvider store={store}>
        <div>child</div>
      </ListenerProvider>,
    );

    await vi.waitFor(() => expect(listenMock).toHaveBeenCalledTimes(1));
    const handler = listenMock.mock.calls[0]?.[0];

    window.dispatchEvent(new Event("offline"));
    handler({
      payload: {
        type: "micStopped",
        apps: [{ id: "us.zoom.xos", name: "Zoom" }],
      },
    });

    await vi.advanceTimersByTimeAsync(AUTO_STOP_CONFIRM_DELAY_MS);
    await vi.advanceTimersByTimeAsync(
      endedAtMs + AUTO_STOP_EVENT_END_GRACE_MS / 2 - Date.now(),
    );

    window.dispatchEvent(new Event("online"));
    handler({
      payload: {
        type: "micDetected",
        key: "mic-resumed",
        apps: [{ id: "us.zoom.xos", name: "Zoom" }],
        duration_secs: 15,
      },
    });

    await vi.advanceTimersByTimeAsync(deadlineMs - Date.now());
    expect(stopSpy).not.toHaveBeenCalled();
  });

  test("holds an offline ad-hoc meeting, then prompts instead of stopping", async () => {
    const store = createListenerStore();
    const stopSpy = vi.fn();

    store.setState({ stop: stopSpy });
    store.getState().setTriggerAppIds(["us.zoom.xos"]);
    setStoreActive(store);

    render(
      <ListenerProvider store={store}>
        <div>child</div>
      </ListenerProvider>,
    );

    await vi.waitFor(() => expect(listenMock).toHaveBeenCalledTimes(1));
    const handler = listenMock.mock.calls[0]?.[0];

    vi.useFakeTimers();
    window.dispatchEvent(new Event("offline"));
    handler({
      payload: {
        type: "micStopped",
        apps: [{ id: "us.zoom.xos", name: "Zoom" }],
      },
    });

    await vi.advanceTimersByTimeAsync(AUTO_STOP_CONFIRM_DELAY_MS);
    expect(stopSpy).not.toHaveBeenCalled();
    expect(showNotificationMock).not.toHaveBeenCalled();

    await vi.advanceTimersByTimeAsync(AUTO_STOP_NETWORK_HOLD_MS - 1);
    expect(stopSpy).not.toHaveBeenCalled();
    expect(showNotificationMock).not.toHaveBeenCalled();

    await vi.advanceTimersByTimeAsync(1);
    expect(stopSpy).not.toHaveBeenCalled();
    expect(showNotificationMock).toHaveBeenCalledWith(
      expect.objectContaining({
        key: expect.stringContaining("auto-stop-ended:session-1"),
      }),
    );
  });

  test.each([
    { name: "coming back online", outageMs: 0 },
    {
      name: "a long outage reconnects",
      outageMs: AUTO_STOP_RECENT_OFFLINE_MS + 1,
    },
  ])(
    "holds auto-stop when micStopped arrives shortly after $name",
    async ({ outageMs }) => {
      const store = createListenerStore();
      const stopSpy = vi.fn();

      store.setState({ stop: stopSpy });
      store.getState().setTriggerAppIds(["us.zoom.xos"]);
      setStoreActive(store);

      const handler = await renderProvider(store);

      vi.useFakeTimers();
      window.dispatchEvent(new Event("offline"));
      if (outageMs > 0) {
        await vi.advanceTimersByTimeAsync(outageMs);
      }
      window.dispatchEvent(new Event("online"));
      handler({
        payload: {
          type: "micStopped",
          apps: [{ id: "us.zoom.xos", name: "Zoom" }],
        },
      });

      await vi.advanceTimersByTimeAsync(AUTO_STOP_CONFIRM_DELAY_MS);
      expect(stopSpy).not.toHaveBeenCalled();
      expect(showNotificationMock).not.toHaveBeenCalled();

      await vi.advanceTimersByTimeAsync(AUTO_STOP_NETWORK_HOLD_MS);
      expect(stopSpy).not.toHaveBeenCalled();
      expect(showNotificationMock).toHaveBeenCalledTimes(1);
    },
  );

  test("does not hold auto-stop after the recent-offline window expires", async () => {
    const store = createListenerStore();
    const stopSpy = vi.fn();

    store.setState({ stop: stopSpy });
    store.getState().setTriggerAppIds(["us.zoom.xos"]);
    setStoreActive(store);

    render(
      <ListenerProvider store={store}>
        <div>child</div>
      </ListenerProvider>,
    );

    await vi.waitFor(() => expect(listenMock).toHaveBeenCalledTimes(1));
    const handler = listenMock.mock.calls[0]?.[0];

    vi.useFakeTimers();
    window.dispatchEvent(new Event("offline"));
    await vi.advanceTimersByTimeAsync(AUTO_STOP_RECENT_OFFLINE_MS + 1);
    window.dispatchEvent(new Event("online"));
    await vi.advanceTimersByTimeAsync(AUTO_STOP_RECENT_OFFLINE_MS + 1);
    handler({
      payload: {
        type: "micStopped",
        apps: [{ id: "us.zoom.xos", name: "Zoom" }],
      },
    });

    await vi.advanceTimersByTimeAsync(AUTO_STOP_CONFIRM_DELAY_MS);
    expect(stopSpy).toHaveBeenCalledTimes(1);
    expect(showNotificationMock).not.toHaveBeenCalled();
  });

  test("does not let an interrupted meeting timer stop a replacement session", async () => {
    const store = createListenerStore();
    const stopSpy = vi.fn();
    const now = new Date("2026-05-19T10:05:00.000Z");
    const deadlineMs =
      new Date("2026-05-19T10:30:00.000Z").getTime() +
      AUTO_STOP_EVENT_END_GRACE_MS;

    store.setState({ stop: stopSpy });
    store.getState().setTriggerAppIds(["us.zoom.xos"]);
    setStoreActive(store);
    (useStoreMock as any).mockReturnValue(
      mockSessionEventStore({
        started_at: "2026-05-19T10:00:00.000Z",
        ended_at: "2026-05-19T10:30:00.000Z",
      }),
    );

    vi.useFakeTimers();
    vi.setSystemTime(now);

    render(
      <ListenerProvider store={store}>
        <div>child</div>
      </ListenerProvider>,
    );

    await vi.waitFor(() => expect(listenMock).toHaveBeenCalledTimes(1));
    const handler = listenMock.mock.calls[0]?.[0];

    window.dispatchEvent(new Event("offline"));
    handler({
      payload: {
        type: "micStopped",
        apps: [{ id: "us.zoom.xos", name: "Zoom" }],
      },
    });
    await vi.advanceTimersByTimeAsync(AUTO_STOP_CONFIRM_DELAY_MS);

    setStoreActive(store, "session-2");
    await vi.advanceTimersByTimeAsync(deadlineMs - Date.now());

    expect(stopSpy).not.toHaveBeenCalled();
  });

  test("does not stop on MicStopped when auto-stop is disabled", async () => {
    const store = createListenerStore();
    const stopSpy = vi.fn();

    store.setState({ stop: stopSpy });
    store.getState().setTriggerAppIds(["us.zoom.xos"]);
    useConfigValueMock.mockReturnValue(false);

    render(
      <ListenerProvider store={store}>
        <div>child</div>
      </ListenerProvider>,
    );

    await vi.waitFor(() => expect(listenMock).toHaveBeenCalledTimes(1));

    const handler = listenMock.mock.calls[0]?.[0];
    expect(handler).toBeTypeOf("function");

    handler({
      payload: {
        type: "micStopped",
        apps: [{ id: "us.zoom.xos", name: "Zoom" }],
      },
    });

    expect(stopSpy).not.toHaveBeenCalled();
  });

  test("does not stop on MicStopped when only a non-trigger app stops (auto-session — regression: #4846)", async () => {
    const store = createListenerStore();
    const stopSpy = vi.fn();

    store.setState({ stop: stopSpy });
    store.getState().setTriggerAppIds(["us.zoom.xos"]);

    render(
      <ListenerProvider store={store}>
        <div>child</div>
      </ListenerProvider>,
    );

    await vi.waitFor(() => expect(listenMock).toHaveBeenCalledTimes(1));

    const handler = listenMock.mock.calls[0]?.[0];
    expect(handler).toBeTypeOf("function");

    handler({
      payload: {
        type: "micStopped",
        apps: [{ id: "/opt/homebrew/bin/ffmpeg", name: "ffmpeg" }],
      },
    });

    expect(stopSpy).not.toHaveBeenCalled();
  });

  test("does not stop after non-trigger MicStopped when a trigger app is still active", async () => {
    const store = createListenerStore();
    const stopSpy = vi.fn();

    store.setState({ stop: stopSpy });
    store.getState().setTriggerAppIds(["us.zoom.xos"]);
    setStoreActive(store);
    listMicUsingApplicationsMock.mockResolvedValue({
      status: "ok",
      data: [{ id: "us.zoom.xos", name: "Zoom" }],
    });

    render(
      <ListenerProvider store={store}>
        <div>child</div>
      </ListenerProvider>,
    );

    await vi.waitFor(() => expect(listenMock).toHaveBeenCalledTimes(1));

    const handler = listenMock.mock.calls[0]?.[0];
    expect(handler).toBeTypeOf("function");

    vi.useFakeTimers();
    listMicUsingApplicationsMock.mockClear();

    handler({
      payload: {
        type: "micStopped",
        apps: [{ id: "/opt/homebrew/bin/ffmpeg", name: "ffmpeg" }],
      },
    });

    await vi.advanceTimersByTimeAsync(AUTO_STOP_CONFIRM_DELAY_MS);

    expect(listMicUsingApplicationsMock).toHaveBeenCalledTimes(1);
    expect(stopSpy).not.toHaveBeenCalled();
  });

  test.each([
    [[{ id: "com.kakao.KakaoTalkMac", name: "KakaoTalk" }]],
    [[{ id: "pid:42", name: "KakaoTalk Helper" }]],
  ])(
    "does not auto-stop KakaoTalk sessions from screen-share mic transitions",
    async (stoppedApps) => {
      const store = createListenerStore();
      const stopSpy = vi.fn();

      store.setState({ stop: stopSpy });
      store.getState().setTriggerAppIds(["com.kakao.KakaoTalkMac"]);
      setStoreActive(store);

      render(
        <ListenerProvider store={store}>
          <div>child</div>
        </ListenerProvider>,
      );

      await vi.waitFor(() => expect(listenMock).toHaveBeenCalledTimes(1));

      const handler = listenMock.mock.calls[0]?.[0];
      expect(handler).toBeTypeOf("function");

      vi.useFakeTimers();
      listMicUsingApplicationsMock.mockClear();

      handler({
        payload: {
          type: "micStopped",
          apps: stoppedApps,
        },
      });

      await vi.advanceTimersByTimeAsync(AUTO_STOP_CONFIRM_DELAY_MS);

      expect(listMicUsingApplicationsMock).not.toHaveBeenCalled();
      expect(stopSpy).not.toHaveBeenCalled();
    },
  );

  test("does not auto-stop co-trigger sessions while KakaoTalk remains active after a helper stop", async () => {
    const store = createListenerStore();
    const stopSpy = vi.fn();

    store.setState({ stop: stopSpy });
    store
      .getState()
      .setTriggerAppIds(["com.kakao.KakaoTalkMac", "us.zoom.xos"]);
    setStoreActive(store);
    listMicUsingApplicationsMock.mockResolvedValue({
      status: "ok",
      data: [{ id: "com.kakao.KakaoTalkMac", name: "KakaoTalk" }],
    });

    render(
      <ListenerProvider store={store}>
        <div>child</div>
      </ListenerProvider>,
    );

    await vi.waitFor(() => expect(listenMock).toHaveBeenCalledTimes(1));

    const handler = listenMock.mock.calls[0]?.[0];
    expect(handler).toBeTypeOf("function");

    vi.useFakeTimers();
    listMicUsingApplicationsMock.mockClear();

    handler({
      payload: {
        type: "micStopped",
        apps: [{ id: "pid:42", name: "KakaoTalk Helper" }],
      },
    });

    await vi.advanceTimersByTimeAsync(AUTO_STOP_CONFIRM_DELAY_MS);

    expect(listMicUsingApplicationsMock).toHaveBeenCalledTimes(1);
    expect(stopSpy).not.toHaveBeenCalled();
  });

  test("auto-stops co-trigger sessions after a helper stop when no trigger app remains active", async () => {
    const store = createListenerStore();
    const stopSpy = vi.fn();

    store.setState({ stop: stopSpy });
    store
      .getState()
      .setTriggerAppIds(["com.kakao.KakaoTalkMac", "us.zoom.xos"]);
    setStoreActive(store);

    render(
      <ListenerProvider store={store}>
        <div>child</div>
      </ListenerProvider>,
    );

    await vi.waitFor(() => expect(listenMock).toHaveBeenCalledTimes(1));

    const handler = listenMock.mock.calls[0]?.[0];
    expect(handler).toBeTypeOf("function");

    vi.useFakeTimers();
    listMicUsingApplicationsMock.mockClear();

    handler({
      payload: {
        type: "micStopped",
        apps: [{ id: "pid:42", name: "KakaoTalk Helper" }],
      },
    });

    await vi.advanceTimersByTimeAsync(AUTO_STOP_CONFIRM_DELAY_MS);

    expect(listMicUsingApplicationsMock).toHaveBeenCalledTimes(1);
    expect(stopSpy).toHaveBeenCalledTimes(1);
  });

  test("auto-stops when MicStopped omits the trigger app and no trigger app remains active (regression: #5436)", async () => {
    const store = createListenerStore();
    const stopSpy = vi.fn();

    store.setState({ stop: stopSpy });
    store.getState().setTriggerAppIds(["com.microsoft.teams2"]);
    setStoreActive(store);

    render(
      <ListenerProvider store={store}>
        <div>child</div>
      </ListenerProvider>,
    );

    await vi.waitFor(() => expect(listenMock).toHaveBeenCalledTimes(1));

    const handler = listenMock.mock.calls[0]?.[0];
    expect(handler).toBeTypeOf("function");

    vi.useFakeTimers();
    listMicUsingApplicationsMock.mockClear();

    handler({
      payload: {
        type: "micStopped",
        apps: [{ id: "pid:42", name: "Microsoft Teams Helper" }],
      },
    });

    await vi.advanceTimersByTimeAsync(AUTO_STOP_CONFIRM_DELAY_MS);

    expect(listMicUsingApplicationsMock).toHaveBeenCalledTimes(1);
    expect(stopSpy).toHaveBeenCalledTimes(1);
  });

  test("asks before stopping Teams running in a browser when the browser no longer uses the mic", async () => {
    const store = createListenerStore();
    const stopSpy = vi.fn();

    store.setState({ stop: stopSpy });
    store.getState().setTriggerAppIds(["company.thebrowser.Browser"]);
    setStoreActive(store);

    render(
      <ListenerProvider store={store}>
        <div>child</div>
      </ListenerProvider>,
    );

    await vi.waitFor(() => expect(listenMock).toHaveBeenCalledTimes(1));

    const handler = listenMock.mock.calls[0]?.[0];
    expect(handler).toBeTypeOf("function");

    vi.useFakeTimers();
    listMicUsingApplicationsMock.mockClear();

    handler({
      payload: {
        type: "micStopped",
        apps: [{ id: "company.thebrowser.Browser", name: "Arc" }],
      },
    });

    await vi.advanceTimersByTimeAsync(AUTO_STOP_CONFIRM_DELAY_MS);

    expect(listMicUsingApplicationsMock).toHaveBeenCalledTimes(1);
    expect(stopSpy).not.toHaveBeenCalled();
    expect(showNotificationMock).toHaveBeenCalledWith(
      expect.objectContaining({
        key: expect.stringContaining("auto-stop-ended:session-1"),
      }),
    );
  });

  test("does not show mic-detected prompts when detection notifications are disabled", async () => {
    const store = createListenerStore();
    useConfigValueMock.mockImplementation((key: string) =>
      key === "notification_detect" ? false : true,
    );

    render(
      <ListenerProvider store={store}>
        <div>child</div>
      </ListenerProvider>,
    );

    await vi.waitFor(() => expect(listenMock).toHaveBeenCalledTimes(1));

    const handler = listenMock.mock.calls[0]?.[0];
    expect(handler).toBeTypeOf("function");

    handler({
      payload: {
        type: "micDetected",
        key: "mic-1",
        apps: [{ id: "us.zoom.xos", name: "Zoom" }],
        duration_secs: 15,
      },
    });

    await Promise.resolve();

    expect(showNotificationMock).not.toHaveBeenCalled();
    expect(getNearbyCalendarEventsMock).not.toHaveBeenCalled();
  });

  test.each([
    {
      name: "nearby Meet link",
      events: [
        {
          title: "Design sync",
          started_at: "2026-06-24T02:09:00.000Z",
          meeting_link: "https://meet.google.com/abc-defg-hij",
        },
      ],
      apps: [{ id: "at.studio.AsideBrowser", name: "Aside" }],
      appNames: ["Google Meet"],
      eventIds: ["event-1"],
    },
    {
      name: "Teams live join link",
      events: [
        {
          title: "Partner sync",
          started_at: "2026-06-24T02:09:00.000Z",
          meeting_link: "https://teams.live.com/meet/1234567890",
        },
      ],
      apps: [{ id: "com.google.Chrome", name: "Google Chrome" }],
      appNames: ["Microsoft Teams"],
      eventIds: ["event-1"],
    },
    {
      name: "explicit link beats earlier nearby text",
      events: [
        {
          title: "Discord planning",
          started_at: "2026-06-24T02:08:00.000Z",
        },
        {
          title: "Design sync",
          started_at: "2026-06-24T02:09:00.000Z",
          meeting_link: "https://meet.google.com/abc-defg-hij",
        },
      ],
      apps: [{ id: "com.google.Chrome", name: "Google Chrome" }],
      appNames: ["Google Meet"],
      eventIds: ["event-2"],
    },
    {
      name: "a different nearby event does not infer a platform",
      events: [
        {
          title: "Sales sync",
          started_at: "2026-06-24T02:09:00.000Z",
        },
        {
          title: "Design sync",
          started_at: "2026-06-24T02:10:00.000Z",
          meeting_link: "https://meet.google.com/abc-defg-hij",
        },
      ],
      apps: [{ id: "com.google.Chrome", name: "Google Chrome" }],
      appNames: ["Google Chrome"],
      eventIds: ["event-1"],
    },
    {
      name: "incidental calendar text does not infer a chat platform",
      events: [
        {
          title: "Quarterly signal review",
          started_at: "2026-06-24T02:09:00.000Z",
          description: "Discuss discordance in metrics with the messenger team",
        },
      ],
      apps: [{ id: "com.google.Chrome", name: "Google Chrome" }],
      appNames: ["Google Chrome"],
      eventIds: ["event-1"],
    },
    {
      name: "calendar video link does not override a detected native meeting app",
      events: [
        {
          title: "Design sync",
          started_at: "2026-06-24T02:09:00.000Z",
          meeting_link: "https://meet.google.com/abc-defg-hij",
        },
      ],
      apps: [
        { id: "com.tinyspeck.slackmacgap", name: "Slack" },
        { id: "com.google.Chrome", name: "Google Chrome" },
      ],
      appNames: ["Slack", "Google Chrome"],
      eventIds: ["event-1"],
    },
    {
      name: "cal.com video link",
      events: [
        {
          title: "Founder call",
          started_at: "2026-06-24T02:09:00.000Z",
          meeting_link: "https://app.cal.com/video/founder-call",
        },
      ],
      apps: [{ id: "at.studio.AsideBrowser", name: "Aside" }],
      appNames: ["Cal Video"],
      eventIds: ["event-1"],
    },
    {
      name: "protocol-less cal.com video text",
      events: [
        {
          title: "Founder call",
          started_at: "2026-06-24T02:09:00.000Z",
          location: "cal.com/video/founder-call",
        },
      ],
      apps: [{ id: "at.studio.AsideBrowser", name: "Aside" }],
      appNames: ["Cal Video"],
      eventIds: ["event-1"],
    },
  ])(
    "infers the meeting platform for mic notifications: $name",
    async ({ events, apps, appNames, eventIds }) => {
      const store = createListenerStore();

      vi.useFakeTimers();
      vi.setSystemTime(new Date("2026-06-24T02:09:00.000Z"));
      (useStoreMock as any).mockReturnValue(mockNearbyEventStoreMany(events));

      const handler = await renderProvider(store);
      expect(handler).toBeTypeOf("function");

      handler({
        payload: {
          type: "micDetected",
          key: "mic-1",
          apps,
          duration_secs: 15,
        },
      });

      await vi.waitFor(() =>
        expect(showNotificationMock).toHaveBeenCalledWith(
          expect.objectContaining({
            source: expect.objectContaining({
              app_names: appNames,
              event_ids: eventIds,
            }),
          }),
        ),
      );
    },
  );

  test("does not show a stale mic prompt when listening starts while icons resolve", async () => {
    const store = createListenerStore();
    let iconResolverReady = false;
    let resolveIcon = () => {};

    vi.mocked(resolveResource).mockImplementationOnce(
      (path: string) =>
        new Promise((resolve) => {
          resolveIcon = () => {
            resolve(`/resources/${path}`);
          };
          iconResolverReady = true;
        }),
    );
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-06-24T02:09:00.000Z"));
    (useStoreMock as any).mockReturnValue(
      mockNearbyEventStore({
        title: "Customer call",
        started_at: "2026-06-24T02:09:00.000Z",
        meeting_link: "https://webex.com/meet/customer-call",
      }),
    );

    render(
      <ListenerProvider store={store}>
        <div>child</div>
      </ListenerProvider>,
    );

    await vi.waitFor(() => expect(listenMock).toHaveBeenCalledTimes(1));

    const handler = listenMock.mock.calls[0]?.[0];
    expect(handler).toBeTypeOf("function");

    handler({
      payload: {
        type: "micDetected",
        key: "mic-1",
        apps: [{ id: "com.google.Chrome", name: "Google Chrome" }],
        duration_secs: 15,
      },
    });

    await vi.waitFor(() => expect(iconResolverReady).toBe(true));

    setStoreActive(store);
    resolveIcon();

    await vi.waitFor(() =>
      expect(store.getState().live.triggerAppIds).toEqual([
        "com.google.Chrome",
      ]),
    );
    expect(showNotificationMock).not.toHaveBeenCalled();
  });

  test("does not show duplicate mic prompts while icons resolve", async () => {
    const store = createListenerStore();
    let iconResolverReady = false;
    let resolveIcon = () => {};

    vi.mocked(resolveResource).mockImplementationOnce(
      (path: string) =>
        new Promise((resolve) => {
          resolveIcon = () => {
            resolve(`/resources/${path}`);
          };
          iconResolverReady = true;
        }),
    );
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-06-24T02:09:00.000Z"));
    (useStoreMock as any).mockReturnValue(
      mockNearbyEventStore({
        title: "Product review",
        started_at: "2026-06-24T02:09:00.000Z",
        meeting_link: "https://meet.jit.si/product-review",
      }),
    );

    render(
      <ListenerProvider store={store}>
        <div>child</div>
      </ListenerProvider>,
    );

    await vi.waitFor(() => expect(listenMock).toHaveBeenCalledTimes(1));

    const handler = listenMock.mock.calls[0]?.[0];
    expect(handler).toBeTypeOf("function");

    handler({
      payload: {
        type: "micDetected",
        key: "mic-1",
        apps: [{ id: "com.google.Chrome", name: "Google Chrome" }],
        duration_secs: 15,
      },
    });

    await vi.waitFor(() => expect(iconResolverReady).toBe(true));

    handler({
      payload: {
        type: "micDetected",
        key: "mic-2",
        apps: [{ id: "com.google.Chrome", name: "Google Chrome" }],
        duration_secs: 15,
      },
    });

    resolveIcon();

    await vi.waitFor(() =>
      expect(showNotificationMock).toHaveBeenCalledTimes(1),
    );
    expect(showNotificationMock).toHaveBeenCalledWith(
      expect.objectContaining({
        key: "mic-1",
        source: expect.objectContaining({
          app_names: ["Jitsi"],
        }),
      }),
    );
  });

  test.each([
    {
      name: "already listening",
      prime: (store: ReturnType<typeof createListenerStore>) =>
        setStoreActive(store),
    },
    {
      name: "listening is starting",
      prime: (store: ReturnType<typeof createListenerStore>) =>
        store.setState((state) => ({
          live: {
            ...state.live,
            loading: true,
            sessionId: "session-1",
            status: "inactive" as const,
          },
        })),
    },
  ])(
    "records trigger app ids from micDetected while $name",
    async ({ prime }) => {
      const store = createListenerStore();
      prime(store);

      const handler = await renderProvider(store);
      expect(handler).toBeTypeOf("function");

      handler({
        payload: {
          type: "micDetected",
          key: "mic-1",
          apps: [
            { id: "pid:42", name: "Chrome Helper" },
            { id: "com.google.Chrome", name: "Google Chrome" },
          ],
          duration_secs: 15,
        },
      });

      expect(showNotificationMock).not.toHaveBeenCalled();
      expect(store.getState().live.triggerAppIds).toEqual([
        "com.google.Chrome",
      ]);
    },
  );

  test("auto-stops after a trigger app learned during active listening stops", async () => {
    const store = createListenerStore();
    const stopSpy = vi.fn();

    store.setState({ stop: stopSpy });
    setStoreActive(store);

    const handler = await renderProvider(store);
    expect(handler).toBeTypeOf("function");

    vi.useFakeTimers();
    listMicUsingApplicationsMock.mockClear();

    handler({
      payload: {
        type: "micDetected",
        key: "mic-1",
        apps: [{ id: "us.zoom.xos", name: "Zoom" }],
        duration_secs: 15,
      },
    });

    handler({
      payload: {
        type: "micStopped",
        apps: [{ id: "us.zoom.xos", name: "Zoom" }],
      },
    });

    await vi.advanceTimersByTimeAsync(AUTO_STOP_CONFIRM_DELAY_MS);

    expect(listMicUsingApplicationsMock).toHaveBeenCalledTimes(1);
    expect(stopSpy).toHaveBeenCalledTimes(1);
  });

  test.each([
    {
      name: "a browser meeting without calendar context",
      browser: { id: "com.google.Chrome", name: "Google Chrome" },
      now: undefined,
      withSessionEvent: false,
    },
    {
      name: "a browser meeting well before the scheduled end",
      browser: { id: "com.google.Chrome", name: "Google Chrome" },
      now: "2026-05-19T10:05:00.000Z",
      withSessionEvent: true,
    },
    {
      name: "a browser meeting near the scheduled end",
      browser: { id: "com.google.Chrome", name: "Google Chrome" },
      now: "2026-05-19T10:29:00.000Z",
      withSessionEvent: true,
    },
  ])(
    "asks before stopping $name",
    async ({ browser, now, withSessionEvent }) => {
      const store = createListenerStore();
      const stopSpy = vi.fn();

      store.setState({ stop: stopSpy });
      store.getState().setTriggerAppIds([browser.id]);
      setStoreActive(store);
      if (withSessionEvent) {
        (useStoreMock as any).mockReturnValue(
          mockSessionEventStore({
            started_at: "2026-05-19T10:00:00.000Z",
            ended_at: "2026-05-19T10:30:00.000Z",
          }),
        );
      }

      const handler = await renderProvider(store);
      expect(handler).toBeTypeOf("function");

      vi.useFakeTimers();
      if (now) {
        vi.setSystemTime(new Date(now));
      }
      listMicUsingApplicationsMock.mockClear();

      handler({
        payload: {
          type: "micStopped",
          apps: [browser],
        },
      });

      await vi.advanceTimersByTimeAsync(AUTO_STOP_CONFIRM_DELAY_MS);

      expect(stopSpy).not.toHaveBeenCalled();
      const notification = showNotificationMock.mock.calls[0]?.[0];
      expect(parseAutoStopEndedNotificationKey(notification?.key)).toBe(
        "session-1",
      );
      expect(notification).toEqual(
        expect.objectContaining({
          key: expect.stringContaining("auto-stop-ended:session-1"),
          action_label: "Stop",
          action_variant: "destructive",
        }),
      );
    },
  );

  test("rechecks after accessibility confirms the meeting is active", async () => {
    const store = createListenerStore();
    const stopSpy = vi.fn();

    store.setState({ stop: stopSpy });
    store.getState().setTriggerAppIds(["com.google.Chrome"]);
    setStoreActive(store);
    inspectMeetingAccessibilityMock
      .mockResolvedValueOnce({
        status: "ok",
        data: [
          {
            activeCall: true,
            app: { id: "com.google.Chrome", name: "Google Chrome" },
            pid: 42,
            platform: "googleMeet",
            surface: "web",
            accessibilityTrusted: true,
            windowTitle: "Team sync - Google Meet",
            remoteScreenShare: false,
            warnings: [],
          },
        ],
      })
      .mockResolvedValue({ status: "ok", data: [] });

    render(
      <ListenerProvider store={store}>
        <div>child</div>
      </ListenerProvider>,
    );
    await vi.waitFor(() => expect(listenMock).toHaveBeenCalledTimes(1));
    const handler = listenMock.mock.calls[0]?.[0];

    vi.useFakeTimers();
    handler({
      payload: {
        type: "micStopped",
        apps: [{ id: "com.google.Chrome", name: "Google Chrome" }],
      },
    });
    await vi.advanceTimersByTimeAsync(AUTO_STOP_CONFIRM_DELAY_MS);

    expect(inspectMeetingAccessibilityMock).toHaveBeenCalledTimes(1);
    expect(stopSpy).not.toHaveBeenCalled();
    expect(showNotificationMock).not.toHaveBeenCalled();

    await vi.advanceTimersByTimeAsync(AUTO_STOP_CONFIRM_DELAY_MS);

    expect(inspectMeetingAccessibilityMock).toHaveBeenCalledTimes(2);
    expect(stopSpy).not.toHaveBeenCalled();
    expect(showNotificationMock).toHaveBeenCalledWith(
      expect.objectContaining({
        key: expect.stringContaining("auto-stop-ended:session-1"),
      }),
    );
  });

  test("does not recheck after the meeting resumes during accessibility inspection", async () => {
    const store = createListenerStore();
    const stopSpy = vi.fn();
    let resolveInspection = (
      _result: Awaited<ReturnType<typeof inspectMeetingAccessibilityMock>>,
    ) => {};

    store.setState({ stop: stopSpy });
    store.getState().setTriggerAppIds(["com.google.Chrome"]);
    setStoreActive(store);
    inspectMeetingAccessibilityMock.mockImplementation(
      () =>
        new Promise((resolve) => {
          resolveInspection = resolve;
        }),
    );

    render(
      <ListenerProvider store={store}>
        <div>child</div>
      </ListenerProvider>,
    );
    await vi.waitFor(() => expect(listenMock).toHaveBeenCalledTimes(1));
    const handler = listenMock.mock.calls[0]?.[0];

    vi.useFakeTimers();
    handler({
      payload: {
        type: "micStopped",
        apps: [{ id: "com.google.Chrome", name: "Google Chrome" }],
      },
    });
    await vi.advanceTimersByTimeAsync(AUTO_STOP_CONFIRM_DELAY_MS);

    expect(inspectMeetingAccessibilityMock).toHaveBeenCalledTimes(1);

    handler({
      payload: {
        type: "micDetected",
        key: "mic-resumed",
        apps: [{ id: "com.google.Chrome", name: "Google Chrome" }],
        duration_secs: 15,
      },
    });
    resolveInspection({
      status: "ok",
      data: [
        {
          app: { id: "com.google.Chrome", name: "Google Chrome" },
          pid: 42,
          platform: "googleMeet",
          surface: "web",
          accessibilityTrusted: true,
          windowTitle: "Team sync - Google Meet",
          remoteScreenShare: false,
          warnings: [],
        },
      ],
    });
    await vi.advanceTimersByTimeAsync(AUTO_STOP_CONFIRM_DELAY_MS);

    expect(inspectMeetingAccessibilityMock).toHaveBeenCalledTimes(1);
    expect(stopSpy).not.toHaveBeenCalled();
    expect(showNotificationMock).not.toHaveBeenCalled();
  });

  test("cancels pending auto-stop when a browser trigger restarts", async () => {
    const store = createListenerStore();
    const stopSpy = vi.fn();

    store.setState({ stop: stopSpy });
    store.getState().setTriggerAppIds(["com.google.Chrome"]);
    setStoreActive(store);

    render(
      <ListenerProvider store={store}>
        <div>child</div>
      </ListenerProvider>,
    );

    await vi.waitFor(() => expect(listenMock).toHaveBeenCalledTimes(1));

    const handler = listenMock.mock.calls[0]?.[0];
    expect(handler).toBeTypeOf("function");

    vi.useFakeTimers();
    listMicUsingApplicationsMock.mockClear();

    handler({
      payload: {
        type: "micStopped",
        apps: [{ id: "com.google.Chrome", name: "Google Chrome" }],
      },
    });

    await vi.advanceTimersByTimeAsync(AUTO_STOP_CONFIRM_DELAY_MS - 1);

    handler({
      payload: {
        type: "micDetected",
        key: "mic-1",
        apps: [{ id: "com.google.Chrome", name: "Google Chrome" }],
        duration_secs: 15,
      },
    });

    await vi.advanceTimersByTimeAsync(1);

    expect(listMicUsingApplicationsMock).not.toHaveBeenCalled();
    expect(stopSpy).not.toHaveBeenCalled();
  });

  test("invalidates a browser stop prompt when the meeting resumes", async () => {
    const store = createListenerStore();
    const stopSpy = vi.fn();

    store.setState({ stop: stopSpy });
    store.getState().setTriggerAppIds(["com.google.Chrome"]);
    setStoreActive(store);

    render(
      <ListenerProvider store={store}>
        <div>child</div>
      </ListenerProvider>,
    );
    await vi.waitFor(() => expect(listenMock).toHaveBeenCalledTimes(1));
    clearNotificationsMock.mockClear();
    const handler = listenMock.mock.calls[0]?.[0];

    vi.useFakeTimers();
    handler({
      payload: {
        type: "micStopped",
        apps: [{ id: "com.google.Chrome", name: "Google Chrome" }],
      },
    });
    await vi.advanceTimersByTimeAsync(AUTO_STOP_CONFIRM_DELAY_MS);
    const notificationKey = showNotificationMock.mock.calls[0]?.[0]?.key;

    handler({
      payload: {
        type: "micDetected",
        key: "mic-resumed",
        apps: [{ id: "com.google.Chrome", name: "Google Chrome" }],
        duration_secs: 15,
      },
    });

    expect(clearNotificationsMock).toHaveBeenCalledTimes(1);
    expect(consumeAutoStopEndedNotificationKey(notificationKey)).toBeNull();
    expect(stopSpy).not.toHaveBeenCalled();
  });

  test("stops listening when sleep starts", async () => {
    const store = createListenerStore();
    const stopSpy = vi.fn();

    store.setState({ stop: stopSpy });

    render(
      <ListenerProvider store={store}>
        <div>child</div>
      </ListenerProvider>,
    );

    await vi.waitFor(() => expect(listenMock).toHaveBeenCalledTimes(1));

    const handler = listenMock.mock.calls[0]?.[0];
    expect(handler).toBeTypeOf("function");

    handler({
      payload: {
        type: "sleepStateChanged",
        value: true,
      },
    });

    expect(stopSpy).toHaveBeenCalledTimes(1);
  });
});
