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
      <button type="button" aria-label="Open event metadata">
        Metadata
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

vi.mock("../shared", () => ({
  RecordingIcon: () => <div data-testid="recording-icon" />,
  useHasTranscript: (sessionId: string) =>
    mocks.hasTranscriptBySession[sessionId] ?? false,
}));

vi.mock("@hypr/plugin-opener2", () => ({
  commands: {
    openUrl: mocks.openUrl,
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
    expect(titleSlot?.className).toContain("right-[70px]");
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
    expect(titleSlot?.className).toContain("right-[70px]");
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
    expect(titleSlot?.className).toContain("right-[70px]");
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
    expect(titleSlot?.className).toContain("right-[70px]");
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
    expect(titleSlot?.className).toContain("right-[70px]");
    expect(titleSlot?.className).not.toContain("right-[153px]");
    expect(screen.queryByRole("button", { name: "Stop listening" })).toBeNull();

    const overflowProps = mocks.overflowProps[mocks.overflowProps.length - 1];
    expect(overflowProps?.standaloneWindow).toBe(true);
    expect(overflowProps?.allowListening).toBeUndefined();
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
    expect(titleSlot?.className).toContain("right-[70px]");
  });

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

    fireEvent.click(joinButton);

    expect(joinButton.getAttribute("aria-label")).toBe("Join & record");
    expect(joinButton.textContent).toContain("Join & record");
    expect(joinButton.getAttribute("data-tauri-drag-region")).toBe("false");
    expect(metadataButton.getAttribute("data-tauri-drag-region")).toBe("false");
    expect(mocks.openUrl).toHaveBeenCalledWith(
      "https://meet.google.com/abc-defg-hij",
      null,
    );
    expect(mocks.startListening).toHaveBeenCalledTimes(1);
  });

  it("shows the meeting countdown to the left of the header action", () => {
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

    const countdown = screen.getByText("starts in 4m 30s");
    const joinButton = screen.getByRole("button", { name: "Join & record" });

    expect(countdown.getAttribute("data-header-meeting-countdown")).toBe(
      "true",
    );
    expect(countdown.className).toContain("font-mono");
    expect(
      countdown.compareDocumentPosition(joinButton) &
        Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBe(Node.DOCUMENT_POSITION_FOLLOWING);
    expect(joinButton.textContent).not.toContain("starts in");
  });

  it("starts listening without joining when only scheduled listening is enabled", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-06-05T09:59:58.000Z"));
    mocks.nowMs = Date.now();
    mocks.configValues.auto_start_scheduled_meetings = true;
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

    expect(mocks.startListening).toHaveBeenCalledTimes(1);
    expect(mocks.openUrl).not.toHaveBeenCalled();
  });

  it("joins and starts when both scheduled meeting settings are enabled", () => {
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

    expect(mocks.openUrl).toHaveBeenCalledWith(
      "https://meet.google.com/abc-defg-hij",
      null,
    );
    expect(mocks.startListening).toHaveBeenCalledTimes(1);
  });

  it("does not join or start when scheduled listening is disabled", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-06-05T09:59:58.000Z"));
    mocks.nowMs = Date.now();
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

    expect(mocks.openUrl).not.toHaveBeenCalled();
    expect(mocks.startListening).not.toHaveBeenCalled();
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

  it("shows resume when an inactive session already has a transcript", () => {
    mocks.hasTranscriptBySession = { "session-1": true };

    render(
      <OuterHeader
        sessionId="session-1"
        currentView={{ type: "transcript" } as EditorView}
        title={<span>Session title</span>}
      />,
    );

    const resumeButton = screen.getByRole("button", { name: "Resume" });

    fireEvent.click(resumeButton);

    expect(resumeButton.title).toBe("Resume listening");
    expect(screen.queryByRole("button", { name: "Record" })).toBeNull();
    expect(screen.getByTestId("recording-icon")).not.toBeNull();
    expect(mocks.startListening).toHaveBeenCalledTimes(1);
  });

  it("shows resume when an inactive session has audio without a transcript", () => {
    mocks.audioExists = true;

    render(
      <OuterHeader
        sessionId="session-1"
        currentView={{ type: "transcript" } as EditorView}
        title={<span>Session title</span>}
      />,
    );

    const resumeButton = screen.getByRole("button", { name: "Resume" });

    fireEvent.click(resumeButton);

    expect(resumeButton.title).toBe("Resume listening");
    expect(screen.queryByRole("button", { name: "Record" })).toBeNull();
    expect(mocks.startListening).toHaveBeenCalledTimes(1);
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

  it("shows resume after the meeting is over", () => {
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

    fireEvent.click(screen.getByRole("button", { name: "Resume" }));

    expect(screen.getByTestId("recording-icon")).not.toBeNull();
    expect(screen.queryByRole("button", { name: "Join & record" })).toBeNull();
    expect(metadataButton.getAttribute("data-tauri-drag-region")).toBe("false");
    expect(mocks.startListening).toHaveBeenCalledTimes(1);
  });
});
