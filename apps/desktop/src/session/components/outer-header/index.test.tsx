import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
} from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { EditorView } from "~/store/zustand/tabs/schema";

const mocks = vi.hoisted(() => ({
  leftsidebar: { expanded: true },
  sessionModes: {} as Record<string, string>,
  sessionEvents: {} as Record<string, any>,
  nowMs: 0,
  openUrl: vi.fn(),
  startCallbackServer: vi.fn(),
  getScheme: vi.fn(),
  startListening: vi.fn(),
  stopListening: vi.fn(),
  requestMainListenerControl: vi.fn(),
  isMainWebviewWindow: true,
  audioExists: false,
  hasTranscriptBySession: {} as Record<string, boolean>,
  configValues: {} as Record<string, boolean>,
  meetingMicInUse: false,
}));

vi.mock("../title-input", () => ({
  TitleInput: () => <input aria-label="Session title" placeholder="Untitled" />,
}));

vi.mock("~/session/queries", () => ({
  useSession: () => ({ folder_id: "" }),
  useFolderIcons: () => ({}),
  useFolderPaths: () => [],
  useUpdateSession: () => vi.fn(),
}));

vi.mock("./overflow", () => ({
  OverflowButton: () => <button type="button">More</button>,
}));

vi.mock("~/session-sharing", () => ({
  SessionShareButton: () => (
    <button type="button" aria-label="Share note">
      Share
    </button>
  ),
}));

vi.mock("../shared", () => ({
  RecordingIcon: () => <div />,
  useHasTranscript: (sessionId: string) =>
    mocks.hasTranscriptBySession[sessionId] ?? false,
}));

vi.mock("@anlg/plugin-opener2", () => ({
  commands: { openUrl: mocks.openUrl },
}));

vi.mock("@anlg/plugin-deeplink2", () => ({
  commands: { startCallbackServer: mocks.startCallbackServer },
}));

vi.mock("~/calendar/hooks", () => ({
  useNow: () => new Date(mocks.nowMs),
}));

vi.mock("~/audio-player", () => ({
  useAudioPlayer: () => ({ audioExists: mocks.audioExists }),
}));

vi.mock("~/contexts/shell", () => ({
  useShell: () => ({ leftsidebar: mocks.leftsidebar }),
}));

vi.mock("~/session/hooks/useSessionEvent", () => ({
  useSessionEvent: (sessionId: string) =>
    mocks.sessionEvents[sessionId] ?? null,
}));

vi.mock("~/session/hooks/useMeetingMicInUse", () => ({
  useMeetingMicInUse: () => mocks.meetingMicInUse,
}));

vi.mock("~/shared/config", () => ({
  useConfigValue: (key: string) => mocks.configValues[key],
}));

vi.mock("~/shared/utils", async (importOriginal) => ({
  ...(await importOriginal<typeof import("~/shared/utils")>()),
  getScheme: mocks.getScheme,
}));

vi.mock("~/store/zustand/tabs", () => ({
  useTabs: vi.fn((selector: (state: unknown) => unknown) => selector({})),
}));

vi.mock("~/stt/contexts", () => ({
  useListener: vi.fn((selector: (state: unknown) => unknown) =>
    selector({
      getSessionMode: (sessionId: string) =>
        mocks.sessionModes[sessionId] ?? "inactive",
      stop: mocks.stopListening,
    }),
  ),
}));

vi.mock("~/stt/useStartListening", () => ({
  useStartListening: () => mocks.startListening,
}));

vi.mock("~/stt/window-control", () => ({
  isMainWebviewWindow: () => mocks.isMainWebviewWindow,
  requestMainListenerControl: mocks.requestMainListenerControl,
}));

import { OuterHeader } from "./index";

const MEET_LINK = "https://meet.google.com/abc-defg-hij";
const DEMO_EVENT = {
  tracking_id: "anarlog-onboarding-demo-v1",
  meeting_link: "https://anarlog.so/onboarding-demo/",
};
const scheduled = (meeting_link?: string) => ({
  title: "Design Review",
  started_at: "2026-06-05T10:00:00.000Z",
  ended_at: "2026-06-05T10:30:00.000Z",
  meeting_link,
});
const BEFORE = "2026-06-05T09:55:00.000Z";
const DURING = "2026-06-05T10:05:00.000Z";
const AFTER = "2026-06-05T10:31:00.000Z";

type Scenario = {
  mode?: string;
  event?: Record<string, unknown>;
  now?: string;
  transcript?: boolean;
  audio?: boolean;
  micInUse?: boolean;
};

function arrange({
  mode,
  event,
  now = BEFORE,
  transcript,
  audio,
  micInUse,
}: Scenario) {
  if (mode) mocks.sessionModes = { "session-1": mode };
  if (event) mocks.sessionEvents = { "session-1": event };
  mocks.nowMs = new Date(now).getTime();
  mocks.hasTranscriptBySession = { "session-1": !!transcript };
  mocks.audioExists = !!audio;
  mocks.meetingMicInUse = !!micInUse;
}

function renderHeader(
  props: {
    view?: EditorView;
    withTab?: boolean;
    viewSwitcher?: ReactNode;
    standaloneWindow?: boolean;
  } = {},
) {
  const view = props.view ?? ({ type: "raw" } as EditorView);
  return render(
    <OuterHeader
      sessionId="session-1"
      currentView={view}
      standaloneWindow={props.standaloneWindow}
      viewSwitcher={props.viewSwitcher}
      tab={
        props.withTab
          ? {
              active: true,
              id: "session-1",
              pinned: false,
              slotId: "slot-1",
              state: { autoStart: null, view },
              type: "sessions",
            }
          : undefined
      }
    />,
  );
}

const ACTIONS = ["Record", "Join & record", "Stop", "Share note"] as const;

function visibleAction() {
  const visible = ACTIONS.filter((name) =>
    screen.queryByRole("button", { name }),
  );
  expect(visible.length).toBeLessThanOrEqual(1);
  return visible[0] ?? null;
}

describe("OuterHeader", () => {
  beforeEach(() => {
    mocks.leftsidebar.expanded = true;
    mocks.sessionModes = {};
    mocks.sessionEvents = {};
    mocks.nowMs = new Date(BEFORE).getTime();
    mocks.openUrl.mockClear();
    mocks.startCallbackServer.mockReset();
    mocks.startCallbackServer.mockResolvedValue({ status: "ok", data: 43210 });
    mocks.getScheme.mockReset();
    mocks.getScheme.mockResolvedValue("anarlog-dev");
    mocks.startListening.mockClear();
    mocks.stopListening.mockClear();
    mocks.requestMainListenerControl.mockClear();
    mocks.isMainWebviewWindow = true;
    mocks.audioExists = false;
    mocks.hasTranscriptBySession = {};
    mocks.configValues = {};
    mocks.meetingMicInUse = false;
  });

  afterEach(() => {
    cleanup();
    vi.useRealTimers();
  });

  it.each<[string, Scenario, (typeof ACTIONS)[number] | null]>([
    ["new ad hoc note", {}, "Record"],
    ["active recording", { mode: "active" }, "Stop"],
    [
      "active recording past the scheduled end",
      { mode: "active", event: scheduled(MEET_LINK), now: AFTER },
      "Stop",
    ],
    ["post-meeting transcription", { mode: "running_batch", now: AFTER }, null],
    ["finalizing", { mode: "finalizing", event: scheduled(MEET_LINK) }, null],
    ["inactive with transcript", { transcript: true }, "Share note"],
    ["inactive with audio", { audio: true }, "Share note"],
    ["meeting over", { event: scheduled(MEET_LINK), now: AFTER }, "Share note"],
    [
      "recorded event without ended_at",
      {
        event: { ...scheduled(MEET_LINK), ended_at: undefined },
        now: AFTER,
        transcript: true,
      },
      "Share note",
    ],
    ["meeting without a link", { event: scheduled() }, "Record"],
    [
      "meeting with an unrecognized link",
      { event: scheduled("https://naver.me/example") },
      "Record",
    ],
    [
      "remote meeting before start",
      { event: scheduled(MEET_LINK) },
      "Join & record",
    ],
    [
      "remote meeting before start with mic in use",
      { event: scheduled(MEET_LINK), micInUse: true },
      "Join & record",
    ],
    [
      "remote meeting after start with mic free",
      { event: scheduled(MEET_LINK), now: DURING },
      "Join & record",
    ],
    [
      "remote meeting after start with mic in use",
      { event: scheduled(MEET_LINK), now: DURING, micInUse: true },
      "Record",
    ],
    [
      "welcome demo after start with mic in use",
      {
        event: { ...DEMO_EVENT, started_at: "2026-06-05T10:00:00.000Z" },
        now: DURING,
        micInUse: true,
      },
      "Join & record",
    ],
  ])("shows the right meeting action for %s", (_label, scenario, expected) => {
    arrange(scenario);
    renderHeader();

    expect(visibleAction()).toBe(expected);
    expect(screen.getByRole("button", { name: "More" })).not.toBeNull();
  });

  it("starts listening from record without opening a meeting", () => {
    arrange({ event: scheduled("https://naver.me/example") });
    renderHeader();

    fireEvent.click(screen.getByRole("button", { name: "Record" }));

    expect(mocks.startListening).toHaveBeenCalledTimes(1);
    expect(mocks.openUrl).not.toHaveBeenCalled();
  });

  it("opens the meeting link and starts listening from join & record", () => {
    arrange({ event: scheduled(MEET_LINK) });
    renderHeader();

    fireEvent.click(screen.getByRole("button", { name: "Join & record" }));

    expect(mocks.openUrl).toHaveBeenCalledWith(MEET_LINK, null);
    expect(mocks.startListening).toHaveBeenCalledTimes(1);
  });

  it("stops listening from the stop button", () => {
    arrange({ mode: "active" });
    renderHeader();

    fireEvent.click(screen.getByRole("button", { name: "Stop" }));

    expect(mocks.stopListening).toHaveBeenCalledTimes(1);
  });

  it("delegates stop to the main window from standalone windows", () => {
    arrange({ mode: "active" });
    mocks.isMainWebviewWindow = false;
    renderHeader({ standaloneWindow: true });

    fireEvent.click(screen.getByRole("button", { name: "Stop" }));

    expect(mocks.requestMainListenerControl).toHaveBeenCalledWith(
      "stop",
      "session-1",
    );
    expect(mocks.stopListening).not.toHaveBeenCalled();
  });

  it("opens the welcome demo with an automatic completion callback", async () => {
    arrange({ event: DEMO_EVENT });
    renderHeader();

    fireEvent.click(screen.getByRole("button", { name: "Join & record" }));

    expect(mocks.startListening).toHaveBeenCalledOnce();
    await vi.waitFor(() => {
      expect(mocks.startCallbackServer).toHaveBeenCalledWith(
        "anarlog-dev",
        null,
      );
      expect(mocks.openUrl).toHaveBeenCalledOnce();
    });

    const openedUrl = new URL(mocks.openUrl.mock.calls[0][0]);
    expect(openedUrl.origin + openedUrl.pathname).toBe(
      "https://anarlog.so/onboarding-demo/",
    );
    expect(openedUrl.searchParams.get("autojoin")).toBe("1");
    expect(openedUrl.searchParams.get("completion_url")).toBe(
      "http://127.0.0.1:43210/onboarding-demo/complete",
    );
  });

  it("still auto-joins the welcome demo if the completion callback cannot start", async () => {
    const consoleError = vi
      .spyOn(console, "error")
      .mockImplementation(() => {});
    mocks.startCallbackServer.mockRejectedValue(new Error("unavailable"));
    arrange({ event: DEMO_EVENT });

    try {
      renderHeader();
      fireEvent.click(screen.getByRole("button", { name: "Join & record" }));

      await vi.waitFor(() => {
        expect(mocks.openUrl).toHaveBeenCalledOnce();
      });

      const openedUrl = new URL(mocks.openUrl.mock.calls[0][0]);
      expect(openedUrl.searchParams.get("autojoin")).toBe("1");
      expect(openedUrl.searchParams.get("completion_url")).toBeNull();
    } finally {
      consoleError.mockRestore();
    }
  });

  it("ignores repeated welcome demo joins while startup is in progress", async () => {
    let resolveCallbackServer: (value: {
      status: "ok";
      data: number;
    }) => void = () => {};
    mocks.startCallbackServer.mockReturnValue(
      new Promise((resolve) => {
        resolveCallbackServer = resolve;
      }),
    );
    arrange({ event: DEMO_EVENT });
    renderHeader();

    const joinButton = screen.getByRole("button", { name: "Join & record" });
    fireEvent.click(joinButton);
    fireEvent.click(joinButton);

    expect(mocks.startListening).toHaveBeenCalledOnce();
    await vi.waitFor(() => {
      expect(mocks.startCallbackServer).toHaveBeenCalledOnce();
    });

    resolveCallbackServer({ status: "ok", data: 43210 });

    await vi.waitFor(() => {
      expect(mocks.openUrl).toHaveBeenCalledOnce();
      expect(joinButton.hasAttribute("disabled")).toBe(false);
    });
  });

  it.each([
    ["before recording", false, true],
    ["after recording", true, false],
  ])("prompts to try the welcome demo only %s", (_label, audio, prompted) => {
    arrange({ event: DEMO_EVENT, audio });
    renderHeader();

    expect(screen.queryByText("Try the demo") !== null).toBe(prompted);
  });

  it("shows the meeting countdown before start and hides it while listening", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-06-05T09:55:30.000Z"));
    arrange({ event: scheduled(MEET_LINK), now: new Date().toISOString() });
    const { unmount } = renderHeader();

    expect(screen.getByText("starts in 4m 30s")).not.toBeNull();
    unmount();

    arrange({ mode: "active", now: new Date().toISOString() });
    renderHeader();

    expect(screen.queryByText(/starts in/)).toBeNull();
  });

  // Scheduled auto-start is owned by ScheduledMeetingAutoStart so it fires
  // regardless of which tab is open; the header must not start a second one.
  it("does not auto-start when the countdown reaches the meeting start time", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-06-05T09:59:58.000Z"));
    mocks.configValues = {
      auto_start_scheduled_meetings: true,
      auto_join_scheduled_meetings: true,
    };
    arrange({ event: scheduled(MEET_LINK), now: new Date().toISOString() });
    renderHeader();

    act(() => {
      vi.advanceTimersByTime(2000);
    });

    expect(mocks.startListening).not.toHaveBeenCalled();
    expect(mocks.openUrl).not.toHaveBeenCalled();
  });

  it.each<[string, Scenario, { view?: EditorView; tabs?: boolean }, boolean]>([
    [
      "summary tab before recording",
      {},
      { view: { type: "enhanced", id: "n" } as EditorView },
      true,
    ],
    ["memo tab before recording", {}, {}, true],
    ["view switcher is shown", {}, { tabs: true }, false],
    ["transcript exists", { transcript: true }, {}, false],
    ["meeting is over", { event: scheduled(), now: AFTER }, {}, false],
    ["listening", { mode: "active" }, {}, false],
    ["finalizing", { mode: "finalizing" }, {}, false],
  ])(
    "title input visibility when %s",
    (_label, scenario, { view, tabs }, visible) => {
      arrange(scenario);
      renderHeader({
        view,
        withTab: true,
        viewSwitcher: tabs ? <div>Tabs</div> : undefined,
      });

      expect(
        screen.queryByRole("textbox", { name: "Session title" }) !== null,
      ).toBe(visible);
    },
  );
});
