import { cleanup, render } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";

import { parseAutoStopEndedNotificationKey } from "./auto-stop-notification";
import {
  AUTO_STOP_CALENDAR_EARLY_END_THRESHOLD_MS,
  AUTO_STOP_CONFIRM_DELAY_MS,
  ListenerProvider,
} from "./contexts";

import { createListenerStore } from "~/store/zustand/listener";

const {
  listMicUsingApplicationsMock,
  listenMock,
  showNotificationMock,
  useStoreMock,
  useSettingsStoreMock,
} = vi.hoisted(() => ({
  listMicUsingApplicationsMock: vi.fn(),
  listenMock: vi.fn(),
  showNotificationMock: vi.fn(),
  useStoreMock: vi.fn(() => null),
  useSettingsStoreMock: vi.fn(() => null),
}));

vi.mock("@hypr/plugin-detect", () => ({
  commands: {
    listMicUsingApplications: listMicUsingApplicationsMock,
  },
  events: {
    detectEvent: {
      listen: listenMock,
    },
  },
}));

vi.mock("@hypr/plugin-notification", () => ({
  commands: {
    showNotification: showNotificationMock,
  },
}));

vi.mock("~/store/tinybase/store/main", () => ({
  STORE_ID: "test-store",
  UI: {
    useStore: useStoreMock,
  },
}));

vi.mock("~/store/tinybase/store/settings", () => ({
  STORE_ID: "settings-store",
  UI: {
    useStore: useSettingsStoreMock,
  },
}));

function setStoreActive(
  store: ReturnType<typeof createListenerStore>,
  sessionId = "session-1",
) {
  store.setState((state) => ({
    live: { ...state.live, sessionId, status: "active" },
  }));
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

describe("ListenerProvider detect events", () => {
  beforeEach(() => {
    listenMock.mockReset();
    showNotificationMock.mockReset();
    useStoreMock.mockReset();
    useSettingsStoreMock.mockReset();
    useStoreMock.mockReturnValue(null);
    useSettingsStoreMock.mockReturnValue(null);
    listenMock.mockResolvedValue(() => {});
    listMicUsingApplicationsMock.mockResolvedValue({ status: "ok", data: [] });
    vi.useRealTimers();
  });

  afterEach(() => {
    cleanup();
    vi.clearAllTimers();
    vi.useRealTimers();
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

  test("does not stop on MicStopped when auto-stop is disabled", async () => {
    const store = createListenerStore();
    const stopSpy = vi.fn();

    store.setState({ stop: stopSpy });
    store.getState().setTriggerAppIds(["us.zoom.xos"]);
    useSettingsStoreMock.mockReturnValue({
      getValue: vi.fn((key: string) =>
        key === "auto_stop_meetings" ? false : undefined,
      ),
    } as any);

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

  test("auto-stops Teams running in a browser when the browser no longer uses the mic (regression: #5436)", async () => {
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
    expect(stopSpy).toHaveBeenCalledTimes(1);
  });

  test("keeps direct trigger auto-stop confidence when a later helper stop arrives", async () => {
    const store = createListenerStore();
    const stopSpy = vi.fn();

    store.setState({ stop: stopSpy });
    store.getState().setTriggerAppIds(["us.zoom.xos"]);
    setStoreActive(store);
    listMicUsingApplicationsMock.mockResolvedValue({
      status: "error",
      error: "failed to read mic snapshot",
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

    handler({
      payload: {
        type: "micStopped",
        apps: [{ id: "pid:42", name: "Zoom Helper" }],
      },
    });

    await vi.advanceTimersByTimeAsync(AUTO_STOP_CONFIRM_DELAY_MS);

    expect(listMicUsingApplicationsMock).toHaveBeenCalledTimes(1);
    expect(stopSpy).toHaveBeenCalledTimes(1);
  });

  test("passes ignorable app ids and footer metadata through mic-detected notifications", async () => {
    const store = createListenerStore();

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
        apps: [
          { id: "pid:42", name: "Zoom" },
          { id: "us.zoom.xos", name: "Zoom" },
        ],
        duration_secs: 15,
      },
    });

    expect(showNotificationMock).toHaveBeenCalledWith(
      expect.objectContaining({
        source: {
          type: "mic_detected",
          app_names: ["Zoom", "Zoom"],
          app_ids: ["us.zoom.xos"],
          event_ids: [],
        },
        footer: {
          text: "Ignore Zoom?",
          actionLabel: "Yes",
          icon: {
            type: "bundle_id",
            bundle_id: "us.zoom.xos",
          },
        },
        icon: {
          type: "bundle_id",
          bundle_id: "us.zoom.xos",
        },
      }),
    );
  });

  test("shows iPhone call icon and label for AV Capture mic notifications", async () => {
    const store = createListenerStore();

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
        apps: [{ id: "pid:42", name: "AV Capture" }],
        duration_secs: 15,
      },
    });

    expect(showNotificationMock).toHaveBeenCalledWith(
      expect.objectContaining({
        source: {
          type: "mic_detected",
          app_names: ["iPhone Call"],
          app_ids: [],
          event_ids: [],
        },
        footer: null,
        icon: {
          type: "system_symbol",
          name: "phone.fill",
        },
      }),
    );
  });

  test.each([
    {
      app: { id: "com.apple.FaceTime", name: "FaceTime" },
      icon: { type: "bundle_id", bundle_id: "com.apple.FaceTime" },
    },
    {
      app: { id: "com.kakao.KakaoTalkMac", name: "KakaoTalk" },
      icon: { type: "bundle_id", bundle_id: "com.kakao.KakaoTalkMac" },
    },
  ])(
    "uses app-specific icons for $app.name mic notifications",
    async ({ app, icon }) => {
      const store = createListenerStore();

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
          apps: [app],
          duration_secs: 15,
        },
      });

      expect(showNotificationMock).toHaveBeenCalledWith(
        expect.objectContaining({
          source: expect.objectContaining({
            app_names: [app.name],
            app_ids: [app.id],
          }),
          footer: expect.objectContaining({
            text: `Ignore ${app.name}?`,
            icon,
          }),
          icon,
        }),
      );
    },
  );

  test("records trigger app ids from micDetected while already listening", async () => {
    const store = createListenerStore();

    setStoreActive(store);

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
        apps: [
          { id: "pid:42", name: "Chrome Helper" },
          { id: "com.google.Chrome", name: "Google Chrome" },
        ],
        duration_secs: 15,
      },
    });

    expect(showNotificationMock).not.toHaveBeenCalled();
    expect(store.getState().live.triggerAppIds).toEqual(["com.google.Chrome"]);
  });

  test("records trigger app ids from micDetected while listening is starting", async () => {
    const store = createListenerStore();

    store.setState((state) => ({
      live: {
        ...state.live,
        loading: true,
        sessionId: "session-1",
        status: "inactive",
      },
    }));

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
        apps: [
          { id: "pid:42", name: "Chrome Helper" },
          { id: "com.google.Chrome", name: "Google Chrome" },
        ],
        duration_secs: 15,
      },
    });

    expect(showNotificationMock).not.toHaveBeenCalled();
    expect(store.getState().live.triggerAppIds).toEqual(["com.google.Chrome"]);
  });

  test("auto-stops after a trigger app learned during active listening stops", async () => {
    const store = createListenerStore();
    const stopSpy = vi.fn();

    store.setState({ stop: stopSpy });
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

  test("uses the standard auto-stop grace period for browser meeting triggers without calendar context", async () => {
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

    handler({
      payload: {
        type: "micStopped",
        apps: [{ id: "com.google.Chrome", name: "Google Chrome" }],
      },
    });

    await vi.advanceTimersByTimeAsync(AUTO_STOP_CONFIRM_DELAY_MS);
    expect(stopSpy).toHaveBeenCalledTimes(1);
  });

  test.each([
    { id: "com.google.Chrome", name: "Google Chrome" },
    { id: "at.studio.AsideBrowser", name: "Aside" },
    { id: "net.imput.helium", name: "Helium" },
  ])(
    "asks before stopping when $name stops well before the scheduled end",
    async (browser) => {
      const store = createListenerStore();
      const stopSpy = vi.fn();
      const now = new Date("2026-05-19T10:05:00.000Z");

      store.setState({ stop: stopSpy });
      store.getState().setTriggerAppIds([browser.id]);
      setStoreActive(store);
      (useStoreMock as any).mockReturnValue(
        mockSessionEventStore({
          started_at: "2026-05-19T10:00:00.000Z",
          ended_at: "2026-05-19T10:30:00.000Z",
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

      vi.useFakeTimers();
      vi.setSystemTime(now);
      listMicUsingApplicationsMock.mockClear();

      handler({
        payload: {
          type: "micStopped",
          apps: [browser],
        },
      });

      await vi.advanceTimersByTimeAsync(AUTO_STOP_CONFIRM_DELAY_MS);

      expect(listMicUsingApplicationsMock).toHaveBeenCalledTimes(1);
      expect(stopSpy).not.toHaveBeenCalled();
      const notification = showNotificationMock.mock.calls[0]?.[0];
      expect(parseAutoStopEndedNotificationKey(notification.key)).toBe(
        "session-1",
      );
      expect(notification).toEqual({
        key: expect.stringContaining("auto-stop-ended:session-1"),
        title: "Did your meeting end?",
        message: `${browser.name} stopped using the microphone before the scheduled end time.`,
        timeout: { secs: 60, nanos: 0 },
        source: null,
        start_time: null,
        participants: null,
        event_details: null,
        action_label: "Stop meeting",
        action_variant: "destructive",
        options: null,
        footer: null,
        icon: { type: "bundle_id", bundle_id: browser.id },
      });
    },
  );

  test("auto-stops browser meetings inside the scheduled end window", async () => {
    const store = createListenerStore();
    const stopSpy = vi.fn();
    const endedAtMs = new Date("2026-05-19T10:30:00.000Z").getTime();
    const now = new Date(
      endedAtMs - AUTO_STOP_CALENDAR_EARLY_END_THRESHOLD_MS + 1,
    );

    store.setState({ stop: stopSpy });
    store.getState().setTriggerAppIds(["com.google.Chrome"]);
    setStoreActive(store);
    (useStoreMock as any).mockReturnValue(
      mockSessionEventStore({
        started_at: "2026-05-19T10:00:00.000Z",
        ended_at: "2026-05-19T10:30:00.000Z",
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

    vi.useFakeTimers();
    vi.setSystemTime(now);
    listMicUsingApplicationsMock.mockClear();

    handler({
      payload: {
        type: "micStopped",
        apps: [{ id: "com.google.Chrome", name: "Google Chrome" }],
      },
    });

    await vi.advanceTimersByTimeAsync(AUTO_STOP_CONFIRM_DELAY_MS);

    expect(listMicUsingApplicationsMock).toHaveBeenCalledTimes(1);
    expect(stopSpy).toHaveBeenCalledTimes(1);
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
