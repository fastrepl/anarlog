import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { EditorView } from "~/store/zustand/tabs/schema";

const mocks = vi.hoisted(() => ({
  leftsidebar: {
    expanded: true,
    showDevtool: false,
  },
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
    mocks.leftsidebar.showDevtool = false;
    mocks.sessionModes = {};
    mocks.sidebarTimelineEnabled = false;
    mocks.stopListening.mockClear();
  });

  afterEach(() => {
    cleanup();
  });

  it("shows a stop listening button for active sessions in sidebar timeline mode", () => {
    mocks.sidebarTimelineEnabled = true;
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
