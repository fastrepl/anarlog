import { cleanup, fireEvent, render, screen } from "@testing-library/react";
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
  sidebarTimelineEnabled: false,
  stopListening: vi.fn(),
}));

vi.mock("./metadata", () => ({
  MetadataButton: () => <button type="button">Metadata</button>,
}));

vi.mock("./overflow", () => ({
  OverflowButton: () => <button type="button">More</button>,
}));

vi.mock("@hypr/ui/components/ui/dancing-sticks", () => ({
  DancingSticks: () => <span data-testid="dancing-sticks" />,
}));

vi.mock("~/contexts/shell", () => ({
  useShell: () => ({
    leftsidebar: mocks.leftsidebar,
  }),
}));

vi.mock("~/shared/config", () => ({
  useConfigValue: () => mocks.sidebarTimelineEnabled,
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
    mocks.sidebarTimelineEnabled = false;
    mocks.stopListening.mockClear();
  });

  afterEach(() => {
    cleanup();
  });

  it("shows a stop listening button for active sessions in collapsed sidebar timeline mode", () => {
    mocks.sidebarTimelineEnabled = true;
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

  it("adds a left gutter for anchored sidebar chrome when collapsed", () => {
    mocks.sidebarTimelineEnabled = true;
    mocks.leftsidebar.expanded = false;

    render(
      <OuterHeader
        sessionId="session-1"
        currentView={{ type: "raw" } as EditorView}
        title={<span>Session title</span>}
      />,
    );

    const title = screen.getByText("Session title");
    const header =
      title.parentElement?.parentElement?.parentElement?.parentElement;
    const titleRow = title.parentElement?.parentElement;

    expect(header?.className).toContain("pl-[156px]");
    expect(titleRow?.className).toContain("items-center");
    expect(screen.queryByRole("button", { name: "Show sidebar" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Go back" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Go forward" })).toBeNull();
  });

  it("keeps sidebar timeline header controls hidden while the sidebar is expanded", () => {
    mocks.sidebarTimelineEnabled = true;
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

  it("keeps the header content row full width", () => {
    const { container } = render(
      <OuterHeader
        sessionId="session-1"
        currentView={{ type: "raw" } as EditorView}
        title={<span>Session title</span>}
      />,
    );

    expect(container.firstElementChild?.firstElementChild?.className).toContain(
      "w-full",
    );
  });

  it("keeps the dedicated stop button hidden outside sidebar timeline mode", () => {
    mocks.sidebarTimelineEnabled = false;
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
});
