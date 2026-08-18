import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
} from "@testing-library/react";
import type { ReactElement } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { EditorView } from "~/store/zustand/tabs/schema";

const mocks = vi.hoisted(() => ({
  leftsidebar: {
    expanded: true,
    toggleExpanded: vi.fn(),
  },
  canGoBack: false,
  canGoNext: false,
  goBack: vi.fn(),
  goNext: vi.fn(),
  sessionModes: {} as Record<string, string>,
  sessionEvents: {} as Record<string, any>,
  nowMs: new Date("2026-06-05T09:50:00.000Z").getTime(),
  openUrl: vi.fn(),
  startCallbackServer: vi.fn(),
  getScheme: vi.fn(),
  startListening: vi.fn(),
  stopListening: vi.fn(),
  stopTranscription: vi.fn(),
  requestMainListenerControl: vi.fn(),
  isMainWebviewWindow: true,
  audioExists: false,
  hasTranscriptBySession: {} as Record<string, boolean>,
  configValues: {
    auto_join_scheduled_meetings: false,
    auto_start_scheduled_meetings: false,
  } as Record<string, boolean>,
  overflowProps: [] as Array<{
    allowListening?: boolean;
    standaloneWindow?: boolean;
  }>,
  shareSessionIds: [] as string[],
  windowControlsGutter: true,
}));

vi.mock("./metadata", () => ({
  MetadataButton: ({
    renderTrigger,
  }: {
    renderTrigger?: (props: { open: boolean; label: string }) => ReactElement;
  }) =>
    renderTrigger ? (
      renderTrigger({ open: false, label: "Open event metadata" })
    ) : (
      <button
        type="button"
        data-tauri-drag-region="false"
        aria-label="Open event metadata"
      >
        <svg aria-hidden="true" data-testid="metadata-calendar-icon" />
      </button>
    ),
}));

vi.mock("./overflow", () => ({
  OverflowButton: (props: {
    allowListening?: boolean;
    standaloneWindow?: boolean;
  }) => {
    mocks.overflowProps.push(props);
    return <button type="button">More</button>;
  },
}));

vi.mock("~/session-sharing", () => ({
  SessionShareButton: ({ sessionId }: { sessionId: string }) => {
    mocks.shareSessionIds.push(sessionId);
    return <button type="button">Share</button>;
  },
}));

vi.mock("../shared", () => ({
  RecordingIcon: () => <div data-testid="recording-icon" />,
  useHasTranscript: (sessionId: string) =>
    mocks.hasTranscriptBySession[sessionId] ?? false,
}));

vi.mock("@anlg/plugin-opener2", () => ({
  commands: {
    openUrl: mocks.openUrl,
  },
}));

vi.mock("@anlg/plugin-deeplink2", () => ({
  commands: {
    startCallbackServer: mocks.startCallbackServer,
  },
}));

vi.mock("~/calendar/hooks", () => ({
  useNow: () => new Date(mocks.nowMs),
}));

vi.mock("~/audio-player", () => ({
  useAudioPlayer: () => ({ audioExists: mocks.audioExists }),
}));

vi.mock("~/contexts/shell", () => ({
  useShell: () => ({
    leftsidebar: mocks.leftsidebar,
  }),
}));

vi.mock("~/session/hooks/useSessionEvent", () => ({
  useSessionEvent: (sessionId: string) =>
    mocks.sessionEvents[sessionId] ?? null,
}));

vi.mock("~/shared/config", () => ({
  useConfigValue: (key: string) => mocks.configValues[key],
}));

vi.mock("~/shared/hooks/useWindowControlsGutter", () => ({
  useWindowControlsGutter: () => mocks.windowControlsGutter,
}));

vi.mock("~/shared/utils", async (importOriginal) => ({
  ...(await importOriginal<typeof import("~/shared/utils")>()),
  getScheme: mocks.getScheme,
}));

vi.mock("~/store/zustand/tabs", () => ({
  useTabs: vi.fn((selector: (state: unknown) => unknown) =>
    selector({
      canGoBack: mocks.canGoBack,
      canGoNext: mocks.canGoNext,
      goBack: mocks.goBack,
      goNext: mocks.goNext,
    }),
  ),
}));

vi.mock("~/stt/contexts", () => ({
  useListener: vi.fn((selector: (state: unknown) => unknown) =>
    selector({
      getSessionMode: (sessionId: string) =>
        mocks.sessionModes[sessionId] ?? "inactive",
      canStartLiveSession: (sessionId: string) =>
        (mocks.sessionModes[sessionId] ?? "inactive") === "inactive",
      stop: mocks.stopListening,
      stopTranscription: mocks.stopTranscription,
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

describe("OuterHeader", () => {
  beforeEach(() => {
    mocks.leftsidebar.expanded = true;
    mocks.leftsidebar.toggleExpanded.mockClear();
    mocks.canGoBack = false;
    mocks.canGoNext = false;
    mocks.goBack.mockClear();
    mocks.goNext.mockClear();
    mocks.sessionModes = {};
    mocks.sessionEvents = {};
    mocks.nowMs = new Date("2026-06-05T09:50:00.000Z").getTime();
    mocks.openUrl.mockClear();
    mocks.startCallbackServer.mockReset();
    mocks.startCallbackServer.mockResolvedValue({
      status: "ok",
      data: 43210,
    });
    mocks.getScheme.mockReset();
    mocks.getScheme.mockResolvedValue("anarlog-dev");
    mocks.startListening.mockClear();
    mocks.stopListening.mockClear();
    mocks.stopTranscription.mockClear();
    mocks.requestMainListenerControl.mockClear();
    mocks.isMainWebviewWindow = true;
    mocks.audioExists = false;
    mocks.hasTranscriptBySession = {};
    mocks.configValues = {
      auto_join_scheduled_meetings: false,
      auto_start_scheduled_meetings: false,
    };
    mocks.overflowProps = [];
    mocks.shareSessionIds = [];
    mocks.windowControlsGutter = true;
  });

  afterEach(() => {
    cleanup();
    vi.useRealTimers();
  });

  it("does not show a separate stop listening button for active sessions while the sidebar is collapsed", () => {
    mocks.leftsidebar.expanded = false;
    mocks.sessionModes = { "session-1": "active" };

    render(
      <OuterHeader
        sessionId="session-1"
        currentView={{ type: "raw" } as EditorView}
        title={<span>Session title</span>}
      />,
    );

    const title = screen.getByText("Session title");
    const titleSlot = title.parentElement?.parentElement;

    expect(screen.queryByRole("button", { name: "Stop listening" })).toBeNull();
    expect(titleSlot?.className).toContain("right-[140px]");
    expect(titleSlot?.className).not.toContain("right-[153px]");
  });

  it("hides the finalizing header button while the sidebar is collapsed", () => {
    mocks.leftsidebar.expanded = false;
    mocks.sessionModes = { "session-1": "finalizing" };

    render(
      <OuterHeader
        sessionId="session-1"
        currentView={{ type: "raw" } as EditorView}
        title={<span>Session title</span>}
      />,
    );

    const title = screen.getByText("Session title");
    const titleSlot = title.parentElement?.parentElement;

    expect(screen.queryByRole("button", { name: "Finalizing" })).toBeNull();
    expect(titleSlot?.className).toContain("right-[140px]");
    expect(titleSlot?.className).not.toContain("right-[153px]");
  });

  it("raises the tightened title field when the sidebar is collapsed", () => {
    mocks.leftsidebar.expanded = false;

    render(
      <OuterHeader
        sessionId="session-1"
        currentView={{ type: "raw" } as EditorView}
        title={<span>Session title</span>}
      />,
    );

    const title = screen.getByText("Session title");
    const titleWrapper = title.parentElement;
    const titleSlot = titleWrapper?.parentElement;
    const header = titleSlot?.parentElement;

    expect(header?.className).toContain("pl-[156px]");
    expect(header?.className).toContain("h-12");
    expect(header?.className).not.toContain("pb-1");
    expect(titleWrapper?.classList.contains("w-full")).toBe(false);
    expect(titleWrapper?.className).toContain("max-w-full");
    expect(titleWrapper?.className).not.toContain("max-w-[680px]");
    expect(titleSlot?.className).toContain("left-[104px]");
    expect(titleSlot?.className).not.toContain("-translate-y-1");
    expect(titleSlot?.className).toContain("right-[140px]");
    expect(screen.queryByRole("button", { name: "Show sidebar" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Go back" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Go forward" })).toBeNull();
  });

  it("uses a compact title offset while the sidebar is expanded", () => {
    mocks.leftsidebar.expanded = true;

    render(
      <OuterHeader
        sessionId="session-1"
        currentView={{ type: "raw" } as EditorView}
        title={<span>Session title</span>}
      />,
    );

    const title = screen.getByText("Session title");
    const titleSlot = title.parentElement?.parentElement;

    expect(titleSlot?.className).toContain("left-0");
    expect(titleSlot?.className).toContain("right-[140px]");
    expect(titleSlot?.className).not.toContain("justify-center");
  });

  it("can center the title slot for toolbar controls", () => {
    render(
      <OuterHeader
        sessionId="session-1"
        currentView={{ type: "raw" } as EditorView}
        centerTitle
        title={<span>Toolbar controls</span>}
      />,
    );

    const title = screen.getByText("Toolbar controls");
    const titleSlot = title.parentElement?.parentElement;

    expect(titleSlot?.className).toContain("justify-center");
  });

  it.each([
    ["summary", { type: "enhanced", id: "summary-1" }],
    ["memos", { type: "raw" }],
    ["transcript", { type: "transcript" }],
  ])("shows sharing from the %s view", (_label, currentView) => {
    render(
      <OuterHeader
        sessionId="session-1"
        currentView={currentView as EditorView}
        title={<span>Session title</span>}
      />,
    );

    const title = screen.getByText("Session title");
    const titleSlot = title.parentElement?.parentElement;

    expect(mocks.shareSessionIds).toEqual(["session-1"]);
    expect(screen.getByRole("button", { name: "Share" })).not.toBeNull();
    expect(titleSlot?.className).toContain("right-[140px]");
  });

  it("keeps sidebar header controls hidden while the sidebar is expanded", () => {
    mocks.sessionModes = { "session-1": "active" };

    const { container } = render(
      <OuterHeader
        sessionId="session-1"
        currentView={{ type: "raw" } as EditorView}
        title={<span>Session title</span>}
      />,
    );

    expect(screen.queryByRole("button", { name: "Hide sidebar" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Go back" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Go forward" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Stop listening" })).toBeNull();
    expect(container.firstElementChild?.className).not.toContain("pl-[156px]");
  });

  it("keeps the session header at 48px tall", () => {
    const { container } = render(
      <OuterHeader
        sessionId="session-1"
        currentView={{ type: "raw" } as EditorView}
        title={<span>Session title</span>}
      />,
    );

    expect(container.firstElementChild?.className).toContain("h-12");
  });

  it("marks the structural title and action strip as draggable", () => {
    const { container } = render(
      <OuterHeader
        sessionId="session-1"
        currentView={{ type: "raw" } as EditorView}
        title={<span>Session title</span>}
      />,
    );

    const header = container.firstElementChild;
    const title = screen.getByText("Session title");
    const titleWrapper = title.parentElement;
    const titleSlot = titleWrapper?.parentElement;
    const actionStrip = header?.lastElementChild;

    expect(header?.hasAttribute("data-tauri-drag-region")).toBe(true);
    expect(titleSlot?.hasAttribute("data-tauri-drag-region")).toBe(true);
    expect(titleWrapper?.hasAttribute("data-tauri-drag-region")).toBe(true);
    expect(actionStrip?.hasAttribute("data-tauri-drag-region")).toBe(true);
  });

  it("keeps the dedicated stop button hidden while the sidebar is expanded", () => {
    mocks.sessionModes = { "session-1": "active" };

    render(
      <OuterHeader
        sessionId="session-1"
        currentView={{ type: "raw" } as EditorView}
        title={<span>Session title</span>}
      />,
    );

    expect(screen.queryByRole("button", { name: "Stop listening" })).toBeNull();
  });

  it("does not show a separate stop button in standalone windows", () => {
    mocks.leftsidebar.expanded = true;
    mocks.sessionModes = { "session-1": "active" };

    render(
      <OuterHeader
        sessionId="session-1"
        currentView={{ type: "raw" } as EditorView}
        standaloneWindow
        title={<span>Session title</span>}
      />,
    );

    const title = screen.getByText("Session title");
    const titleSlot = title.parentElement?.parentElement;

    expect(titleSlot?.className).toContain("left-[76px]");
    expect(titleSlot?.className).toContain("right-[140px]");
    expect(titleSlot?.className).not.toContain("right-[153px]");
    expect(screen.queryByRole("button", { name: "Stop listening" })).toBeNull();

    const overflowProps = mocks.overflowProps[mocks.overflowProps.length - 1];
    expect(overflowProps?.standaloneWindow).toBe(true);
    expect(overflowProps?.allowListening).toBeUndefined();
    expect(mocks.shareSessionIds).toContain("session-1");
  });

  it("does not reserve collapsed sidebar gutter in standalone windows", () => {
    mocks.leftsidebar.expanded = false;

    const { container } = render(
      <OuterHeader
        sessionId="session-1"
        currentView={{ type: "raw" } as EditorView}
        standaloneWindow
        title={<span>Session title</span>}
      />,
    );

    const title = screen.getByText("Session title");
    const titleSlot = title.parentElement?.parentElement;
    const header = container.firstElementChild;

    expect(header?.className).not.toContain("pl-[156px]");
    expect(titleSlot?.className).toContain("left-[76px]");
    expect(titleSlot?.className).toContain("right-[140px]");
  });

  it.each([
    ["expanded", true],
    ["collapsed", false],
  ])(
    "drops the window controls inset in standalone windows without native chrome with the sidebar %s",
    (_state, expanded) => {
      mocks.leftsidebar.expanded = expanded;
      mocks.windowControlsGutter = false;

      render(
        <OuterHeader
          sessionId="session-1"
          currentView={{ type: "raw" } as EditorView}
          standaloneWindow
          title={<span>Session title</span>}
        />,
      );

      const title = screen.getByText("Session title");
      const titleSlot = title.parentElement?.parentElement;

      expect(titleSlot?.className).toContain("left-2");
      expect(titleSlot?.className).not.toContain("left-[76px]");
    },
  );

  it("shows a join-and-record pill before a remote meeting with a video link", () => {
    mocks.sessionEvents = {
      "session-1": {
        title: "Design Review",
        started_at: "2026-06-05T10:00:00.000Z",
        ended_at: "2026-06-05T10:30:00.000Z",
        meeting_link: "https://meet.google.com/abc-defg-hij",
      },
    };
    mocks.nowMs = new Date("2026-06-05T09:55:00.000Z").getTime();

    render(
      <OuterHeader
        sessionId="session-1"
        currentView={{ type: "raw" } as EditorView}
        title={<span>Session title</span>}
      />,
    );

    const joinButton = screen.getByRole("button", { name: "Join & record" });
    const metadataButton = screen.getByRole("button", {
      name: "Open event metadata",
    });
    const actionPill = joinButton.parentElement;

    expect(actionPill?.className).toContain("bg-primary");
    expect(actionPill?.className).toContain("dark:bg-white");
    expect(actionPill?.className).toContain("dark:text-black");
    expect(joinButton.className).toContain("hover:bg-primary/90");
    expect(joinButton.className).toContain("dark:hover:bg-white/90");
    expect(metadataButton.className).toContain("text-primary-foreground/70");
    expect(metadataButton.className).toContain("dark:text-black/70");
    expect(joinButton.getAttribute("aria-label")).toBe("Join & record");
    expect(joinButton.textContent).toContain("Join & record");
    expect(joinButton.getAttribute("data-tauri-drag-region")).toBe("false");
    expect(metadataButton.getAttribute("data-tauri-drag-region")).toBe("false");

    fireEvent.click(joinButton);

    expect(mocks.openUrl).toHaveBeenCalledWith(
      "https://meet.google.com/abc-defg-hij",
      null,
    );
    expect(mocks.startListening).toHaveBeenCalledTimes(1);
  });

  it("opens the welcome demo with an automatic completion callback", async () => {
    mocks.sessionEvents = {
      "session-1": {
        tracking_id: "anarlog-onboarding-demo-v1",
        meeting_link: "https://anarlog.so/onboarding-demo/",
      },
    };

    render(
      <OuterHeader
        sessionId="session-1"
        currentView={{ type: "raw" } as EditorView}
      />,
    );

    const joinButton = screen.getByRole("button", { name: "Join & record" });
    const logo = joinButton.querySelector("img");

    fireEvent.click(joinButton);

    expect(logo?.getAttribute("src")).toBe("/assets/anarlog-icon.png");
    expect(logo?.getAttribute("alt")).toBe("");
    expect(mocks.startListening).toHaveBeenCalledOnce();
    await vi.waitFor(() => {
      expect(mocks.startCallbackServer).toHaveBeenCalledWith("anarlog-dev");
      expect(mocks.openUrl).toHaveBeenCalledOnce();
    });

    const openedUrl = new URL(mocks.openUrl.mock.calls[0][0]);
    expect(openedUrl.origin + openedUrl.pathname).toBe(
      "https://anarlog.so/onboarding-demo/",
    );
    expect(openedUrl.searchParams.get("completion_url")).toBe(
      "http://127.0.0.1:43210/onboarding-demo/complete",
    );
  });

  it("prompts new users to try the prerecorded welcome demo", () => {
    mocks.sessionEvents = {
      "session-1": {
        tracking_id: "anarlog-onboarding-demo-v1",
        meeting_link: "https://anarlog.so/onboarding-demo/",
      },
    };

    render(
      <OuterHeader
        sessionId="session-1"
        currentView={{ type: "raw" } as EditorView}
      />,
    );

    const prompt = screen
      .getByText("Try the demo")
      .closest("[data-welcome-demo-prompt]");

    expect(prompt).not.toBeNull();
    expect(prompt?.textContent).toContain(
      "This is a prerecorded demo, so your camera stays off.",
    );
    expect(prompt?.textContent).toContain(
      "Click Join & record to see Anarlog in action.",
    );
    expect(
      prompt?.querySelector("[data-welcome-demo-prompt-tail]"),
    ).not.toBeNull();
    expect(prompt?.parentElement?.parentElement).toBe(document.body);
  });

  it("does not prompt users who have already recorded the welcome demo", () => {
    mocks.audioExists = true;
    mocks.sessionEvents = {
      "session-1": {
        tracking_id: "anarlog-onboarding-demo-v1",
        meeting_link: "https://anarlog.so/onboarding-demo/",
      },
    };

    render(
      <OuterHeader
        sessionId="session-1"
        currentView={{ type: "raw" } as EditorView}
      />,
    );

    expect(screen.queryByText("Try the demo")).toBeNull();
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
    mocks.sessionEvents = {
      "session-1": {
        tracking_id: "anarlog-onboarding-demo-v1",
        meeting_link: "https://anarlog.so/onboarding-demo/",
      },
    };

    render(
      <OuterHeader
        sessionId="session-1"
        currentView={{ type: "raw" } as EditorView}
      />,
    );

    const joinButton = screen.getByRole("button", { name: "Join & record" });

    fireEvent.click(joinButton);
    fireEvent.click(joinButton);

    expect(joinButton.hasAttribute("disabled")).toBe(true);
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

  it("shows the meeting countdown in a callout below the header action", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-06-05T09:55:30.000Z"));
    mocks.nowMs = Date.now();
    mocks.sessionEvents = {
      "session-1": {
        title: "Design Review",
        started_at: "2026-06-05T10:00:00.000Z",
        ended_at: "2026-06-05T10:30:00.000Z",
        meeting_link: "https://meet.google.com/abc-defg-hij",
      },
    };

    render(
      <OuterHeader
        sessionId="session-1"
        currentView={{ type: "raw" } as EditorView}
        title={<span>Session title</span>}
      />,
    );

    const countdown = screen
      .getByText("starts in 4m 30s")
      .closest("[data-header-meeting-countdown]");
    const joinButton = screen.getByRole("button", { name: "Join & record" });

    expect(countdown).not.toBeNull();
    if (!countdown) throw new Error("meeting countdown is missing");
    expect(countdown.getAttribute("data-header-meeting-countdown")).toBe(
      "true",
    );
    expect(countdown.className).toContain("font-mono");
    expect(countdown.className).toContain("rounded-md");
    expect(countdown.className).toContain("border");
    expect(countdown.className).toContain("shadow-sm");
    expect(countdown.className).toContain("tabular-nums");
    expect(countdown.className).toContain("absolute");
    expect(countdown.className).toContain("top-full");
    expect(countdown.className).toContain("left-1/2");
    expect(
      countdown.querySelector("[data-header-meeting-countdown-tail]"),
    ).not.toBeNull();
    expect(countdown.parentElement?.className).toContain("relative");
    expect(joinButton.textContent).not.toContain("starts in");
  });

  // Scheduled auto-start is owned by ScheduledMeetingAutoStart so it fires
  // regardless of which tab is open; the header must not start a second one.
  it("does not auto-start when the countdown reaches the meeting start time", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-06-05T09:59:58.000Z"));
    mocks.nowMs = Date.now();
    mocks.configValues.auto_start_scheduled_meetings = true;
    mocks.configValues.auto_join_scheduled_meetings = true;
    mocks.sessionEvents = {
      "session-1": {
        started_at: "2026-06-05T10:00:00.000Z",
        ended_at: "2026-06-05T10:30:00.000Z",
        meeting_link: "https://meet.google.com/abc-defg-hij",
      },
    };

    render(
      <OuterHeader
        sessionId="session-1"
        currentView={{ type: "raw" } as EditorView}
      />,
    );

    act(() => {
      vi.advanceTimersByTime(2000);
    });

    expect(mocks.startListening).not.toHaveBeenCalled();
    expect(mocks.openUrl).not.toHaveBeenCalled();
  });

  it("hides the meeting countdown while listening is active", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-06-05T09:55:30.000Z"));
    mocks.nowMs = Date.now();
    mocks.sessionModes = { "session-1": "active" };
    mocks.sessionEvents = {
      "session-1": {
        title: "Design Review",
        started_at: "2026-06-05T10:00:00.000Z",
        ended_at: "2026-06-05T10:30:00.000Z",
        meeting_link: "https://meet.google.com/abc-defg-hij",
      },
    };

    render(
      <OuterHeader
        sessionId="session-1"
        currentView={{ type: "raw" } as EditorView}
        title={<span>Session title</span>}
      />,
    );

    expect(screen.getByRole("button", { name: "Stop" })).not.toBeNull();
    expect(
      document.querySelector("[data-header-meeting-countdown]"),
    ).toBeNull();
  });

  it("shows record before a meeting without a video link", () => {
    mocks.sessionEvents = {
      "session-1": {
        title: "Design Review",
        started_at: "2026-06-05T10:00:00.000Z",
        ended_at: "2026-06-05T10:30:00.000Z",
      },
    };
    mocks.nowMs = new Date("2026-06-05T09:55:00.000Z").getTime();

    render(
      <OuterHeader
        sessionId="session-1"
        currentView={{ type: "raw" } as EditorView}
        title={<span>Session title</span>}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Record" }));

    expect(mocks.startListening).toHaveBeenCalledTimes(1);
    expect(
      screen.getByRole("button", { name: "Open event metadata" }),
    ).not.toBeNull();
  });

  it("shows record before a meeting with an unrecognized video link", () => {
    mocks.sessionEvents = {
      "session-1": {
        title: "Design Review",
        started_at: "2026-06-05T10:00:00.000Z",
        ended_at: "2026-06-05T10:30:00.000Z",
        meeting_link: "https://naver.me/example",
      },
    };
    mocks.nowMs = new Date("2026-06-05T09:55:00.000Z").getTime();

    render(
      <OuterHeader
        sessionId="session-1"
        currentView={{ type: "raw" } as EditorView}
        title={<span>Session title</span>}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Record" }));

    expect(screen.queryByRole("button", { name: "Join & record" })).toBeNull();
    expect(mocks.startListening).toHaveBeenCalledTimes(1);
    expect(mocks.openUrl).not.toHaveBeenCalled();
  });

  it("shows record for a new ad hoc meeting note", () => {
    render(
      <OuterHeader
        sessionId="session-1"
        currentView={{ type: "raw" } as EditorView}
        title={<span>Session title</span>}
      />,
    );

    const recordButton = screen.getByRole("button", { name: "Record" });

    expect(recordButton.parentElement?.className).toContain("bg-card");
    expect(recordButton.parentElement?.className).not.toContain("bg-primary");
    expect(recordButton.querySelector("span")?.className).toContain(
      "@max-[480px]:sr-only",
    );
    fireEvent.click(recordButton);

    expect(mocks.startListening).toHaveBeenCalledTimes(1);
    expect(
      screen.getByRole("button", { name: "Open event metadata" }),
    ).not.toBeNull();
  });

  it("shows only metadata for an inactive ad hoc session with a transcript", () => {
    mocks.hasTranscriptBySession = { "session-1": true };

    render(
      <OuterHeader
        sessionId="session-1"
        currentView={{ type: "transcript" } as EditorView}
        title={<span>Session title</span>}
      />,
    );

    expect(screen.queryByRole("button", { name: "Resume" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Record" })).toBeNull();
    expect(
      screen.getByRole("button", { name: "Open event metadata" }),
    ).not.toBeNull();
    expect(mocks.startListening).not.toHaveBeenCalled();
  });

  it("shows only metadata for an inactive ad hoc session with audio", () => {
    mocks.audioExists = true;

    render(
      <OuterHeader
        sessionId="session-1"
        currentView={{ type: "transcript" } as EditorView}
        title={<span>Session title</span>}
      />,
    );

    expect(screen.queryByRole("button", { name: "Resume" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Record" })).toBeNull();
    expect(
      screen.getByRole("button", { name: "Open event metadata" }),
    ).not.toBeNull();
    expect(mocks.startListening).not.toHaveBeenCalled();
  });

  it("keeps stop available for an active ad hoc session", () => {
    mocks.sessionModes = { "session-1": "active" };

    render(
      <OuterHeader
        sessionId="session-1"
        currentView={{ type: "raw" } as EditorView}
        title={<span>Session title</span>}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Stop" }));

    expect(
      screen.getByRole("button", { name: "Open event metadata" }),
    ).not.toBeNull();
    expect(mocks.stopListening).toHaveBeenCalledTimes(1);
  });

  it("shows stop while the meeting is in progress", () => {
    mocks.sessionEvents = {
      "session-1": {
        title: "Design Review",
        started_at: "2026-06-05T10:00:00.000Z",
        ended_at: "2026-06-05T10:30:00.000Z",
        meeting_link: "https://meet.google.com/abc-defg-hij",
      },
    };
    mocks.sessionModes = { "session-1": "active" };

    render(
      <OuterHeader
        sessionId="session-1"
        currentView={{ type: "raw" } as EditorView}
        title={<span>Session title</span>}
      />,
    );

    const stopButton = screen.getByRole("button", { name: "Stop" });

    fireEvent.click(stopButton);

    expect(stopButton.querySelector("svg")?.getAttribute("class")).toContain(
      "text-red-500",
    );
    expect(screen.queryByRole("button", { name: "Join & record" })).toBeNull();
    expect(
      screen.getByRole("button", { name: "Open event metadata" }),
    ).not.toBeNull();
    expect(mocks.stopListening).toHaveBeenCalledTimes(1);
  });

  it("keeps stop available when recording runs past the scheduled end", () => {
    mocks.sessionEvents = {
      "session-1": {
        title: "Design Review",
        started_at: "2026-06-05T10:00:00.000Z",
        ended_at: "2026-06-05T10:30:00.000Z",
        meeting_link: "https://meet.google.com/abc-defg-hij",
      },
    };
    mocks.sessionModes = { "session-1": "active" };
    mocks.nowMs = new Date("2026-06-05T10:31:00.000Z").getTime();

    render(
      <OuterHeader
        sessionId="session-1"
        currentView={{ type: "raw" } as EditorView}
        title={<span>Session title</span>}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Stop" }));

    expect(screen.queryByTestId("metadata-calendar-icon")).toBeNull();
    expect(mocks.stopListening).toHaveBeenCalledTimes(1);
  });

  it("shows only the calendar metadata button after the meeting is over", () => {
    mocks.sessionEvents = {
      "session-1": {
        title: "Design Review",
        started_at: "2026-06-05T10:00:00.000Z",
        ended_at: "2026-06-05T10:30:00.000Z",
        meeting_link: "https://meet.google.com/abc-defg-hij",
      },
    };
    mocks.nowMs = new Date("2026-06-05T10:31:00.000Z").getTime();

    render(
      <OuterHeader
        sessionId="session-1"
        currentView={{ type: "raw" } as EditorView}
        title={<span>Session title</span>}
      />,
    );

    const metadataButton = screen.getByRole("button", {
      name: "Open event metadata",
    });

    expect(screen.getByTestId("metadata-calendar-icon")).not.toBeNull();
    expect(screen.queryByRole("button", { name: "Join & record" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Resume" })).toBeNull();
    expect(metadataButton.getAttribute("data-tauri-drag-region")).toBe("false");
    expect(metadataButton.parentElement?.className).not.toContain("mr-1");
    expect(mocks.startListening).not.toHaveBeenCalled();
  });

  it("shows transcript editing in the meeting-action slot after an ad hoc meeting", () => {
    mocks.hasTranscriptBySession = { "session-1": true };
    const onTranscriptEditModeChange = vi.fn();
    const view = render(
      <OuterHeader
        sessionId="session-1"
        currentView={{ type: "transcript" } as EditorView}
        transcriptEditMode={false}
        onTranscriptEditModeChange={onTranscriptEditModeChange}
        title={<span>Session title</span>}
      />,
    );

    const writeButton = screen.getByRole("button", { name: "Write" });
    expect(writeButton.className).toContain("@max-[480px]:w-7");
    expect(writeButton.querySelector("span")?.className).toContain(
      "@max-[480px]:sr-only",
    );
    fireEvent.click(writeButton);
    expect(onTranscriptEditModeChange).toHaveBeenCalledWith(true);
    expect(screen.queryByRole("button", { name: "Record" })).toBeNull();
    expect(
      screen.getByRole("button", { name: "Open event metadata" }),
    ).not.toBeNull();

    view.rerender(
      <OuterHeader
        sessionId="session-1"
        currentView={{ type: "transcript" } as EditorView}
        transcriptEditMode
        onTranscriptEditModeChange={onTranscriptEditModeChange}
        title={<span>Session title</span>}
      />,
    );

    const doneButton = screen.getByRole("button", { name: "Done writing" });
    expect(doneButton.getAttribute("aria-pressed")).toBe("true");
    fireEvent.click(doneButton);
    expect(onTranscriptEditModeChange).toHaveBeenLastCalledWith(false);
  });

  it("does not show transcript editing outside the transcript tab", () => {
    mocks.hasTranscriptBySession = { "session-1": true };

    render(
      <OuterHeader
        sessionId="session-1"
        currentView={{ type: "raw" } as EditorView}
        onTranscriptEditModeChange={vi.fn()}
        title={<span>Session title</span>}
      />,
    );

    expect(screen.queryByRole("button", { name: "Write" })).toBeNull();
  });

  it("does not show transcript editing while the meeting is active", () => {
    mocks.hasTranscriptBySession = { "session-1": true };
    mocks.sessionModes = { "session-1": "active" };

    render(
      <OuterHeader
        sessionId="session-1"
        currentView={{ type: "transcript" } as EditorView}
        onTranscriptEditModeChange={vi.fn()}
        title={<span>Session title</span>}
      />,
    );

    expect(screen.queryByRole("button", { name: "Write" })).toBeNull();
    expect(screen.getByRole("button", { name: "Stop" })).not.toBeNull();
  });

  it("shows transcript editing alongside metadata after a scheduled meeting", () => {
    mocks.hasTranscriptBySession = { "session-1": true };
    mocks.sessionEvents = {
      "session-1": {
        title: "Design Review",
        started_at: "2026-06-05T10:00:00.000Z",
        ended_at: "2026-06-05T10:30:00.000Z",
      },
    };
    mocks.nowMs = new Date("2026-06-05T10:31:00.000Z").getTime();

    render(
      <OuterHeader
        sessionId="session-1"
        currentView={{ type: "transcript" } as EditorView}
        onTranscriptEditModeChange={vi.fn()}
        title={<span>Session title</span>}
      />,
    );

    expect(screen.getByRole("button", { name: "Write" })).not.toBeNull();
    expect(
      screen.getByRole("button", { name: "Open event metadata" }),
    ).not.toBeNull();
  });
});
