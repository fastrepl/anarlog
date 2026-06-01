import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  addDeletion: vi.fn(),
  amplitude: { mic: 0.4, speaker: 0.3 },
  ignoreEvent: vi.fn(),
  invalidateResource: vi.fn(),
  isIgnored: vi.fn(() => false),
  openCurrent: vi.fn(),
  openNew: vi.fn(),
  sessionMode: "inactive",
  stop: vi.fn(),
  storeTitle: "Live Note",
  timelineSelection: {
    selectedIds: [] as string[],
    setAnchor: vi.fn(),
    selectRange: vi.fn(),
    toggleSelect: vi.fn(),
  },
}));

vi.mock("@hypr/plugin-fs-sync", () => ({
  commands: {
    sessionDir: vi.fn(() => Promise.resolve({ status: "ok", data: "" })),
  },
}));

vi.mock("@hypr/plugin-opener2", () => ({
  commands: {
    openPath: vi.fn(() => Promise.resolve()),
  },
}));

vi.mock("@hypr/ui/components/ui/dancing-sticks", () => ({
  DancingSticks: ({ amplitude }: { amplitude: number }) => (
    <span data-amplitude={amplitude} data-testid="dancing-sticks" />
  ),
}));

vi.mock("@hypr/ui/components/ui/spinner", () => ({
  Spinner: () => <span data-testid="spinner" />,
}));

vi.mock("@hypr/ui/components/ui/tooltip", () => ({
  Tooltip: ({ children }: { children: ReactNode }) => <>{children}</>,
  TooltipContent: ({ children }: { children: ReactNode }) => <>{children}</>,
  TooltipTrigger: ({ children }: { children: ReactNode }) => <>{children}</>,
}));

vi.mock("~/session/components/session-preview-card", () => ({
  SessionPreviewCard: ({ children }: { children: ReactNode }) => (
    <>{children}</>
  ),
}));

vi.mock("~/session/hooks/useEnhancedNotes", () => ({
  useIsSessionEnhancing: () => false,
}));

vi.mock("~/shared/hooks/useNativeContextMenu", () => ({
  useNativeContextMenu: () => vi.fn(),
}));

vi.mock("~/store/tinybase/hooks", () => ({
  useIgnoredEvents: () => ({
    ignoreEvent: mocks.ignoreEvent,
    ignoreSeries: vi.fn(),
    isIgnored: mocks.isIgnored,
    unignoreEvent: vi.fn(),
    unignoreSeries: vi.fn(),
  }),
}));

vi.mock("~/store/tinybase/store/deleteSession", () => ({
  captureSessionData: vi.fn(() => null),
  deleteSessionCascade: vi.fn(),
  finalizeSessionDeletion: vi.fn(),
}));

vi.mock("~/store/tinybase/store/main", () => ({
  STORE_ID: "main",
  UI: {
    useCell: () => mocks.storeTitle,
    useIndexes: () => ({}),
    useRow: () => null,
    useStore: () => ({}),
  },
}));

vi.mock("~/store/zustand/live-title", () => ({
  useSessionTitle: (_sessionId: string, storeTitle: string | undefined) =>
    storeTitle,
}));

vi.mock("~/store/zustand/tabs", () => ({
  useTabs: (
    selector: (state: {
      invalidateResource: typeof mocks.invalidateResource;
      openCurrent: typeof mocks.openCurrent;
      openNew: typeof mocks.openNew;
    }) => unknown,
  ) =>
    selector({
      invalidateResource: mocks.invalidateResource,
      openCurrent: mocks.openCurrent,
      openNew: mocks.openNew,
    }),
}));

vi.mock("~/store/zustand/timeline-selection", () => ({
  useTimelineSelection: Object.assign(
    (selector: (state: typeof mocks.timelineSelection) => unknown) =>
      selector(mocks.timelineSelection),
    {
      getState: () => mocks.timelineSelection,
    },
  ),
}));

vi.mock("~/store/zustand/undo-delete", () => ({
  useUndoDelete: (
    selector: (state: { addDeletion: typeof mocks.addDeletion }) => unknown,
  ) => selector({ addDeletion: mocks.addDeletion }),
}));

vi.mock("~/stt/contexts", () => ({
  useListener: (
    selector: (state: {
      getSessionMode: (sessionId: string) => string;
      live: { amplitude: { mic: number; speaker: number } };
      stop: typeof mocks.stop;
    }) => unknown,
  ) =>
    selector({
      getSessionMode: () => mocks.sessionMode,
      live: { amplitude: mocks.amplitude },
      stop: mocks.stop,
    }),
}));

import { TimelineItemComponent } from "./item";

describe("TimelineItemComponent", () => {
  beforeEach(() => {
    cleanup();
    mocks.amplitude = { mic: 0.4, speaker: 0.3 };
    mocks.sessionMode = "inactive";
    mocks.stop.mockClear();
    mocks.openCurrent.mockClear();
    mocks.openNew.mockClear();
    mocks.timelineSelection.selectedIds = [];
    mocks.timelineSelection.setAnchor.mockClear();
    mocks.timelineSelection.selectRange.mockClear();
    mocks.timelineSelection.toggleSelect.mockClear();
  });

  it("marks the active session row red in the sidebar timeline", () => {
    mocks.sessionMode = "active";

    render(
      <TimelineItemComponent
        item={{
          type: "session",
          id: "session-live",
          data: {
            title: "Live Note",
            created_at: "2024-01-15T10:30:00.000Z",
          },
        }}
        precision="time"
        selected
        timezone="UTC"
        multiSelected={false}
        flatItemKeys={["session-session-live"]}
      />,
    );

    const rowButton = screen.getByText("Live Note").closest("button");

    expect(rowButton?.className).toContain("bg-red-500");
    expect(rowButton?.className).toContain("text-white");
    expect(rowButton?.className).not.toContain("bg-neutral-200");
    expect(screen.getByTestId("dancing-sticks").dataset.amplitude).toBe("0.5");

    fireEvent.click(screen.getByRole("button", { name: "Stop listening" }));

    expect(mocks.stop).toHaveBeenCalledOnce();
    expect(mocks.openCurrent).not.toHaveBeenCalled();
  });
});
