import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
} from "@testing-library/react";
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

vi.mock("../folder-picker", () => ({
  FolderPicker: () => (
    <button type="button" role="combobox" aria-label="Select folder">
      Folder
    </button>
  ),
}));

vi.mock("./metadata", () => ({
  MetadataButton: () => (
    <button
      type="button"
      data-tauri-drag-region="false"
      aria-label="Open event metadata"
    >
      <svg aria-hidden="true" data-testid="metadata-calendar-icon" />
    </button>
  ),
}));

vi.mock("./create-brief-button", () => ({
  CreateBriefButton: () => null,
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

    const { container } = render(
      <OuterHeader
        sessionId="session-1"
        currentView={{ type: "raw" } as EditorView}
      />,
    );

    const spacer = container.firstElementChild?.firstElementChild;

    expect(screen.queryByRole("button", { name: "Stop listening" })).toBeNull();
    expect(spacer?.className).toContain("flex-1");
    expect(spacer?.className).not.toContain("right-[140px]");
  });

  it("shows only calendar metadata after a scheduled meeting is stopped", () => {
    mocks.leftsidebar.expanded = false;
    mocks.sessionModes = { "session-1": "finalizing" };
    mocks.sessionEvents = {
      "session-1": {
        title: "Design Review",
        started_at: "2026-06-05T09:45:00.000Z",
        ended_at: "2026-06-05T10:30:00.000Z",
        meeting_link: "https://meet.google.com/abc-defg-hij",
      },
    };

    const { container } = render(
      <OuterHeader
        sessionId="session-1"
        currentView={{ type: "raw" } as EditorView}
      />,
    );

    const spacer = container.firstElementChild?.firstElementChild;

    expect(screen.queryByRole("button", { name: "Join & record" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Record" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Stop" })).toBeNull();
    expect(screen.getByTestId("metadata-calendar-icon")).not.toBeNull();
    expect(spacer?.className).toContain("flex-1");
    expect(spacer?.className).not.toContain("right-[140px]");
  });

  it("uses the collapsed sidebar gutter without a title field", () => {
    mocks.leftsidebar.expanded = false;

    const { container } = render(
      <OuterHeader
        sessionId="session-1"
        currentView={{ type: "raw" } as EditorView}
      />,
    );

    const header = container.firstElementChild;
    const spacer = header?.firstElementChild;

    expect(header?.className).toContain("pl-[116px]");
    expect(header?.className).toContain("h-12");
    expect(header?.className).not.toContain("pb-1");
    expect(spacer?.className).toContain("flex-1");
    expect(spacer?.className).not.toContain("-translate-y-1");
    expect(spacer?.className).not.toContain("right-[140px]");
    expect(screen.queryByRole("button", { name: "Show sidebar" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Go back" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Go forward" })).toBeNull();
  });

  it("uses the collapsed sidebar gutter without native window controls", () => {
    mocks.leftsidebar.expanded = false;
    mocks.windowControlsGutter = false;

    const { container } = render(
      <OuterHeader
        sessionId="session-1"
        currentView={{ type: "raw" } as EditorView}
      />,
    );

    expect(container.firstElementChild?.className).toContain("pl-[40px]");
    expect(container.firstElementChild?.className).not.toContain("pl-[116px]");
    expect(container.firstElementChild?.className).not.toContain("pl-2");
  });

  it("does not add a title offset while the sidebar is expanded", () => {
    mocks.leftsidebar.expanded = true;

    const { container } = render(
      <OuterHeader
        sessionId="session-1"
        currentView={{ type: "raw" } as EditorView}
      />,
    );

    const spacer = container.firstElementChild?.firstElementChild;

    expect(spacer?.className).toContain("flex-1");
    expect(spacer?.className).not.toContain("right-[140px]");
    expect(spacer?.className).not.toContain("justify-center");
    expect(container.firstElementChild?.className).toContain("pl-2");
    expect(container.firstElementChild?.className).not.toContain("pl-[114px]");
    expect(container.firstElementChild?.className).not.toContain("pl-[116px]");
  });

  it.each([
    ["summary", { type: "enhanced", id: "summary-1" }],
    ["memos", { type: "raw" }],
    ["transcript", { type: "transcript" }],
  ])("shows sharing from the %s view", (_label, currentView) => {
    const { container } = render(
      <OuterHeader
        sessionId="session-1"
        currentView={currentView as EditorView}
      />,
    );

    const spacer = container.firstElementChild?.firstElementChild;

    expect(mocks.shareSessionIds).toEqual(["session-1"]);
    expect(screen.getByRole("button", { name: "Share" })).not.toBeNull();
    expect(spacer?.className).toContain("flex-1");
  });

  it("keeps sidebar header controls hidden while the sidebar is expanded", () => {
    mocks.sessionModes = { "session-1": "active" };

    const { container } = render(
      <OuterHeader
        sessionId="session-1"
        currentView={{ type: "raw" } as EditorView}
      />,
    );

    expect(screen.queryByRole("button", { name: "Hide sidebar" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Go back" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Go forward" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Stop listening" })).toBeNull();
    expect(container.firstElementChild?.className).toContain("pl-2");
    expect(container.firstElementChild?.className).not.toContain("pl-[116px]");
  });

  it("keeps the session header at 48px tall", () => {
    const { container } = render(
      <OuterHeader
        sessionId="session-1"
        currentView={{ type: "raw" } as EditorView}
      />,
    );

    expect(container.firstElementChild?.className).toContain("h-12");
  });

  it("marks the spacer and action strip as draggable", () => {
    const { container } = render(
      <OuterHeader
        sessionId="session-1"
        currentView={{ type: "raw" } as EditorView}
      />,
    );

    const header = container.firstElementChild;
    const spacer = header?.firstElementChild;
    const actionStrip = header?.lastElementChild;

    expect(header?.hasAttribute("data-tauri-drag-region")).toBe(true);
    expect(spacer?.hasAttribute("data-tauri-drag-region")).toBe(true);
    expect(actionStrip?.hasAttribute("data-tauri-drag-region")).toBe(true);
  });

  it("places the folder, calendar, and share controls in order", () => {
    mocks.hasTranscriptBySession = { "session-1": true };

    const { container } = render(
      <OuterHeader
        sessionId="session-1"
        currentView={{ type: "raw" } as EditorView}
        viewSwitcher={
          <div role="group" aria-label="Session note views">
            Tabs
          </div>
        }
      />,
    );

    const header = container.firstElementChild;
    const views = screen.getByRole("group", { name: "Session note views" });
    const folder = screen.getByRole("combobox", { name: "Select folder" });
    const calendar = screen.getByRole("button", {
      name: "Open event metadata",
    });
    const share = screen.getByRole("button", { name: "Share" });
    const actionStrip = header?.lastElementChild;
    const actionChildren = [...(actionStrip?.children ?? [])];

    expect(header?.firstElementChild).toBe(views);
    expect(actionStrip?.contains(folder)).toBe(true);
    expect(actionStrip?.contains(calendar)).toBe(true);
    expect(actionStrip?.contains(share)).toBe(true);
    expect(
      actionChildren.findIndex((child) => child.contains(folder)),
    ).toBeLessThan(
      actionChildren.findIndex((child) => child.contains(calendar)),
    );
    expect(
      actionChildren.findIndex((child) => child.contains(calendar)),
    ).toBeLessThan(actionChildren.findIndex((child) => child.contains(share)));
  });

  it("places stop immediately before the folder while listening", () => {
    mocks.sessionModes = { "session-1": "active" };

    const { container } = render(
      <OuterHeader
        sessionId="session-1"
        currentView={{ type: "raw" } as EditorView}
        viewSwitcher={
          <div role="group" aria-label="Session note views">
            Tabs
          </div>
        }
      />,
    );

    const stop = screen.getByRole("button", { name: "Stop" });
    const folder = screen.getByRole("combobox", { name: "Select folder" });
    const actionStrip = container.firstElementChild?.lastElementChild;
    const actionChildren = [...(actionStrip?.children ?? [])];
    const stopIndex = actionChildren.findIndex((child) => child.contains(stop));
    const folderIndex = actionChildren.findIndex((child) =>
      child.contains(folder),
    );

    expect(actionStrip?.contains(stop)).toBe(true);
    expect(stopIndex).toBe(folderIndex - 1);
    expect(screen.getByRole("group", { name: "Session note views" })).not.toBe(
      stop.closest("[role='group']"),
    );
  });

  it("keeps the dedicated stop button visible while the sidebar is expanded", () => {
    mocks.sessionModes = { "session-1": "active" };

    render(
      <OuterHeader
        sessionId="session-1"
        currentView={{ type: "raw" } as EditorView}
      />,
    );

    expect(screen.getByRole("button", { name: "Stop" })).not.toBeNull();
  });

  it("shows stop next to folder in standalone windows", () => {
    mocks.leftsidebar.expanded = true;
    mocks.sessionModes = { "session-1": "active" };

    const { container } = render(
      <OuterHeader
        sessionId="session-1"
        currentView={{ type: "raw" } as EditorView}
        standaloneWindow
      />,
    );

    const header = container.firstElementChild;
    const stop = screen.getByRole("button", { name: "Stop" });
    const folder = screen.getByRole("combobox", { name: "Select folder" });
    const actionStrip = header?.lastElementChild;
    const actionChildren = [...(actionStrip?.children ?? [])];

    expect(header?.className).toContain("pl-[76px]");
    expect(header?.className).not.toContain("right-[153px]");
    expect(
      actionChildren.findIndex((child) => child.contains(stop)),
    ).toBeLessThan(actionChildren.findIndex((child) => child.contains(folder)));

    const overflowProps = mocks.overflowProps[mocks.overflowProps.length - 1];
    expect(overflowProps?.standaloneWindow).toBe(true);
    expect(overflowProps?.allowListening).toBeUndefined();
    expect(mocks.shareSessionIds).toContain("session-1");
  });

  it("delegates live meeting stop from the header pill in standalone windows", () => {
    mocks.sessionModes = { "session-1": "active" };
    mocks.isMainWebviewWindow = false;

    render(
      <OuterHeader
        sessionId="session-1"
        currentView={{ type: "raw" } as EditorView}
        standaloneWindow
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Stop" }));

    expect(mocks.requestMainListenerControl).toHaveBeenCalledWith(
      "stop",
      "session-1",
    );
    expect(mocks.stopListening).not.toHaveBeenCalled();
  });

  it("does not reserve collapsed sidebar gutter in standalone windows", () => {
    mocks.leftsidebar.expanded = false;

    const { container } = render(
      <OuterHeader
        sessionId="session-1"
        currentView={{ type: "raw" } as EditorView}
        standaloneWindow
      />,
    );

    const header = container.firstElementChild;

    expect(header?.className).not.toContain("pl-[116px]");
    expect(header?.className).toContain("pl-[76px]");
  });

  it.each([
    ["expanded", true],
    ["collapsed", false],
  ])(
    "drops the window controls inset in standalone windows without native chrome with the sidebar %s",
    (_state, expanded) => {
      mocks.leftsidebar.expanded = expanded;
      mocks.windowControlsGutter = false;

      const { container } = render(
        <OuterHeader
          sessionId="session-1"
          currentView={{ type: "raw" } as EditorView}
          standaloneWindow
        />,
      );

      const header = container.firstElementChild;

      expect(header?.className).toContain("pl-2");
      expect(header?.className).not.toContain("pl-[76px]");
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
      />,
    );

    const joinButton = screen.getByRole("button", { name: "Join & record" });
    const metadataButton = screen.getByRole("button", {
      name: "Open event metadata",
    });

    expect(joinButton.className).toContain("bg-primary");
    expect(joinButton.className).toContain("dark:bg-white");
    expect(joinButton.className).toContain("dark:text-black");
    expect(joinButton.className).toContain("hover:bg-primary/90");
    expect(joinButton.className).toContain("dark:hover:bg-white/90");
    expect(joinButton.querySelector("img")?.className).toContain("size-3.5");
    expect(joinButton.getAttribute("aria-label")).toBe("Join & record");
    expect(joinButton.textContent).toContain("Join & record");
    expect(joinButton.getAttribute("data-tauri-drag-region")).toBe("false");
    expect(metadataButton.getAttribute("data-tauri-drag-region")).toBe("false");
    expect(joinButton.parentElement?.contains(metadataButton)).toBe(false);

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
    expect(logo?.className).toContain("size-3.5");
    expect(mocks.startListening).toHaveBeenCalledOnce();
    await vi.waitFor(() => {
      expect(mocks.startCallbackServer).toHaveBeenCalledWith("anarlog-dev");
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
    mocks.sessionEvents = {
      "session-1": {
        tracking_id: "anarlog-onboarding-demo-v1",
        meeting_link: "https://anarlog.so/onboarding-demo/",
      },
    };

    try {
      render(
        <OuterHeader
          sessionId="session-1"
          currentView={{ type: "raw" } as EditorView}
        />,
      );

      fireEvent.click(screen.getByRole("button", { name: "Join & record" }));

      await vi.waitFor(() => {
        expect(mocks.openUrl).toHaveBeenCalledOnce();
      });

      const openedUrl = new URL(mocks.openUrl.mock.calls[0][0]);
      expect(openedUrl.origin + openedUrl.pathname).toBe(
        "https://anarlog.so/onboarding-demo/",
      );
      expect(openedUrl.searchParams.get("autojoin")).toBe("1");
      expect(openedUrl.searchParams.get("completion_url")).toBeNull();
      expect(consoleError).toHaveBeenCalled();
    } finally {
      consoleError.mockRestore();
    }
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
      />,
    );

    const recordButton = screen.getByRole("button", { name: "Record" });

    expect(recordButton.className).toContain("bg-primary");
    expect(recordButton.className).toContain("dark:bg-white");
    expect(recordButton.className).toContain("dark:text-black");
    expect(recordButton.className).toContain("hover:bg-primary/90");
    expect(recordButton.className).toContain("dark:hover:bg-white/90");
    expect(recordButton.querySelector("span")?.className).not.toContain(
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
      />,
    );

    expect(screen.queryByRole("button", { name: "Resume" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Record" })).toBeNull();
    expect(
      screen.getByRole("button", { name: "Open event metadata" }),
    ).not.toBeNull();
    expect(mocks.startListening).not.toHaveBeenCalled();
  });

  it("keeps the separate stop pill next to folder when the view switcher is present", () => {
    mocks.sessionModes = { "session-1": "active" };

    render(
      <OuterHeader
        sessionId="session-1"
        currentView={{ type: "raw" } as EditorView}
        viewSwitcher={<div>tabs</div>}
      />,
    );

    const stop = screen.getByRole("button", { name: "Stop" });
    const folder = screen.getByRole("combobox", { name: "Select folder" });

    expect(stop).not.toBeNull();
    expect(screen.getByText("tabs")).not.toBeNull();
    expect(screen.getByTestId("metadata-calendar-icon")).not.toBeNull();
    expect(
      stop.compareDocumentPosition(folder) & Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();
  });

  it("keeps stop available for an active ad hoc session", () => {
    mocks.sessionModes = { "session-1": "active" };

    render(
      <OuterHeader
        sessionId="session-1"
        currentView={{ type: "raw" } as EditorView}
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
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Stop" }));

    expect(screen.getByTestId("metadata-calendar-icon")).not.toBeNull();
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

  it("shows only the calendar metadata button when the session is recorded but ended_at is missing", () => {
    mocks.sessionEvents = {
      "session-1": {
        title: "Design Review",
        started_at: "2026-06-05T10:00:00.000Z",
        meeting_link: "https://meet.google.com/abc-defg-hij",
      },
    };
    mocks.hasTranscriptBySession = { "session-1": true };
    mocks.nowMs = new Date("2026-06-05T10:31:00.000Z").getTime();

    render(
      <OuterHeader
        sessionId="session-1"
        currentView={{ type: "raw" } as EditorView}
      />,
    );

    expect(screen.getByTestId("metadata-calendar-icon")).not.toBeNull();
    expect(screen.queryByRole("button", { name: "Join & record" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Resume" })).toBeNull();
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
      />,
    );

    expect(screen.getByRole("button", { name: "Write" })).not.toBeNull();
    expect(
      screen.getByRole("button", { name: "Open event metadata" }),
    ).not.toBeNull();
  });
});
