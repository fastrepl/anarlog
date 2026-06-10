import { cleanup, fireEvent, render, screen } from "@testing-library/react";
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
  stopListening: vi.fn(),
  nowMs: new Date("2026-06-05T09:50:00.000Z").getTime(),
  openUrl: vi.fn(),
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
  OverflowButton: () => <button type="button">More</button>,
}));

vi.mock("@hypr/ui/components/ui/dancing-sticks", () => ({
  DancingSticks: () => <span data-testid="dancing-sticks" />,
}));

vi.mock("@hypr/plugin-opener2", () => ({
  commands: {
    openUrl: mocks.openUrl,
  },
}));

vi.mock("~/calendar/hooks", () => ({
  useNow: () => new Date(mocks.nowMs),
}));

vi.mock("~/contexts/shell", () => ({
  useShell: () => ({
    leftsidebar: mocks.leftsidebar,
  }),
}));

vi.mock("~/store/tinybase/hooks", () => ({
  useSessionEvent: (sessionId: string) =>
    mocks.sessionEvents[sessionId] ?? null,
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
      live: {
        amplitude: {
          mic: 0.5,
          speaker: 0.25,
        },
        degraded: null,
        muted: false,
      },
      stop: mocks.stopListening,
    }),
  ),
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
    mocks.stopListening.mockClear();
    mocks.nowMs = new Date("2026-06-05T09:50:00.000Z").getTime();
    mocks.openUrl.mockClear();
  });

  afterEach(() => {
    cleanup();
  });

  it("shows a stop listening button for active sessions while the sidebar is collapsed", () => {
    mocks.leftsidebar.expanded = false;
    mocks.sessionModes = { "session-1": "active" };

    render(
      <OuterHeader
        sessionId="session-1"
        currentView={{ type: "raw" } as EditorView}
        title={<span>Session title</span>}
      />,
    );

    const stopButton = screen.getByRole("button", {
      name: "Stop listening",
    });

    fireEvent.click(stopButton);

    expect(screen.getByTestId("dancing-sticks")).not.toBeNull();
    expect(stopButton.className).toContain("h-7");
    expect(stopButton.className).toContain("w-20");
    expect(stopButton.className).toContain("rounded-full");
    expect(stopButton.textContent).toContain("Stop");
    expect(mocks.stopListening).toHaveBeenCalledTimes(1);
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
    expect(header?.className).toContain("h-[52px]");
    expect(header?.className).toContain("pb-1");
    expect(titleWrapper?.className).toContain("w-full");
    expect(titleWrapper?.className).not.toContain("max-w-[680px]");
    expect(titleSlot?.className).toContain("left-[104px]");
    expect(titleSlot?.className).toContain("-translate-y-1");
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

  it("shows a header join control before a remote meeting", () => {
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

    const joinButton = screen.getByRole("button", { name: "Join Meet" });

    fireEvent.click(joinButton);

    expect(joinButton.textContent).toContain("Join Meet");
    expect(
      screen.getByRole("button", { name: "Open event metadata" }),
    ).not.toBeNull();
    expect(mocks.openUrl).toHaveBeenCalledWith(
      "https://meet.google.com/abc-defg-hij",
      null,
    );
  });

  it("shows metadata without the join control while the meeting is in progress", () => {
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

    expect(screen.queryByRole("button", { name: "Join Meet" })).toBeNull();
    expect(
      screen.getByRole("button", { name: "Open event metadata" }),
    ).not.toBeNull();
  });

  it("shows the calendar metadata button after the meeting is over", () => {
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

    expect(screen.queryByRole("button", { name: "Join Meet" })).toBeNull();
    expect(metadataButton.textContent).toBe("Metadata");
  });
});
