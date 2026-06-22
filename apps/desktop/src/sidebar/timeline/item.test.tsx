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
  nativeContextMenus: [] as Array<
    Array<{
      id?: string;
      text?: string;
      action?: () => void;
      separator?: boolean;
    }>
  >,
  timelineSelection: {
    selectedIds: [] as string[],
    setAnchor: vi.fn(),
    selectRange: vi.fn(),
    toggleSelect: vi.fn(),
  },
  windowShow: vi.fn(() => Promise.resolve({ status: "ok", data: null })),
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

vi.mock("@hypr/plugin-windows", () => ({
  commands: {
    windowShow: mocks.windowShow,
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

vi.mock("~/session/hooks/useEnhancedNotes", () => ({
  useIsSessionEnhancing: () => false,
}));

vi.mock("~/shared/hooks/useNativeContextMenu", () => ({
  useNativeContextMenu: (
    menu: Array<{
      id?: string;
      text?: string;
      action?: () => void;
      separator?: boolean;
    }>,
  ) => {
    mocks.nativeContextMenus.push(menu);
    return vi.fn();
  },
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
    mocks.storeTitle = "Live Note";
    mocks.stop.mockClear();
    mocks.openCurrent.mockClear();
    mocks.openNew.mockClear();
    mocks.windowShow.mockClear();
    mocks.nativeContextMenus = [];
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

    expect(rowButton?.className).toContain("bg-destructive");
    expect(rowButton?.className).toContain("text-destructive-foreground");
    expect(rowButton?.className).not.toContain("bg-accent");
    expect(screen.getByTestId("dancing-sticks").dataset.amplitude).toBe("0.5");

    const stopButton = screen.getByRole("button", { name: "Stop listening" });
    expect(stopButton.className).toContain("text-white/80");
    expect(stopButton.className).toContain("hover:text-white");

    fireEvent.click(stopButton);

    expect(mocks.stop).toHaveBeenCalledOnce();
    expect(mocks.openCurrent).not.toHaveBeenCalled();
  });

  it("exposes the selected session row for sidebar scroll anchoring", () => {
    const selectedNodeRef = vi.fn();

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
        selectedNodeRef={selectedNodeRef}
        timezone="UTC"
        multiSelected={false}
        flatItemKeys={["session-session-live"]}
      />,
    );

    const row = screen
      .getByText("Live Note")
      .closest("[data-sidebar-timeline-session-id]");

    expect(row?.getAttribute("data-sidebar-timeline-session-id")).toBe(
      "session-live",
    );
    expect(selectedNodeRef.mock.calls.some(([node]) => node === row)).toBe(
      true,
    );
  });

  it("highlights an upcoming meeting row", () => {
    render(
      <TimelineItemComponent
        item={{
          type: "event",
          id: "event-standup",
          data: {
            title: "Team standup",
            started_at: "2024-01-15T10:30:00.000Z",
            ended_at: "2024-01-15T11:00:00.000Z",
            tracking_id_event: "tracking-standup",
            has_recurrence_rules: false,
          },
        }}
        precision="time"
        selected={false}
        timezone="UTC"
        multiSelected={false}
        flatItemKeys={["event-event-standup"]}
        isUpcoming
      />,
    );

    const rowButton = screen.getByText("Team standup").closest("button");

    expect(rowButton?.className).toContain("bg-destructive/8");
    expect(rowButton?.className).toContain("motion-safe:animate-pulse");
    expect(rowButton?.className).not.toContain("shadow-[0_0_22px");
    expect(rowButton?.className).not.toContain("ring-1");
    expect(rowButton?.className).not.toContain("opacity-65");
  });

  it("exposes an arbitrary timeline row for visibility checks", () => {
    const itemNodeRef = vi.fn();

    render(
      <TimelineItemComponent
        item={{
          type: "event",
          id: "event-standup",
          data: {
            title: "Team standup",
            started_at: "2024-01-15T10:30:00.000Z",
            ended_at: "2024-01-15T11:00:00.000Z",
            tracking_id_event: "tracking-standup",
            has_recurrence_rules: false,
          },
        }}
        precision="time"
        selected={false}
        timezone="UTC"
        multiSelected={false}
        flatItemKeys={["event-event-standup"]}
        itemNodeRef={itemNodeRef}
      />,
    );

    const row = screen
      .getByText("Team standup")
      .closest("button")?.parentElement;

    expect(itemNodeRef.mock.calls.some(([node]) => node === row)).toBe(true);
  });

  it("renders finalizing session spinner at the end of the row", () => {
    mocks.sessionMode = "finalizing";
    mocks.storeTitle = "Finalizing Note";

    render(
      <TimelineItemComponent
        item={{
          type: "session",
          id: "session-finalizing",
          data: {
            title: "Finalizing Note",
            created_at: "2024-01-15T10:30:00.000Z",
          },
        }}
        precision="time"
        selected={false}
        timezone="UTC"
        multiSelected={false}
        flatItemKeys={["session-session-finalizing"]}
      />,
    );

    const rowButton = screen.getByText("Finalizing Note").closest("button");
    const spinnerSlot = screen.getByTestId("spinner").parentElement;

    expect(rowButton?.className).toContain("pr-10");
    expect(spinnerSlot?.className).toContain("absolute");
    expect(spinnerSlot?.className).toContain("right-3");
  });

  it("opens the current tab after a single-click on a session row", () => {
    render(
      <TimelineItemComponent
        item={{
          type: "session",
          id: "session-note",
          data: {
            title: "Window Note",
            created_at: "2024-01-15T10:30:00.000Z",
          },
        }}
        precision="time"
        selected={false}
        timezone="UTC"
        multiSelected={false}
        flatItemKeys={["session-session-note"]}
      />,
    );

    const rowButton = screen.getByText("Live Note").closest("button");
    fireEvent.click(rowButton!, { detail: 1 });

    expect(mocks.timelineSelection.setAnchor).toHaveBeenCalledWith(
      "session-session-note",
    );
    expect(mocks.openCurrent).toHaveBeenCalledWith({
      id: "session-note",
      type: "sessions",
    });
  });

  it("opens a standalone note window when a session row is double-clicked", () => {
    render(
      <TimelineItemComponent
        item={{
          type: "session",
          id: "session-note-window",
          data: {
            title: "Window Note",
            created_at: "2024-01-15T10:30:00.000Z",
          },
        }}
        precision="time"
        selected={false}
        timezone="UTC"
        multiSelected={false}
        flatItemKeys={["session-session-note-window"]}
      />,
    );

    const rowButton = screen.getByText("Live Note").closest("button");
    fireEvent.click(rowButton!, { detail: 1 });
    fireEvent.click(rowButton!, { detail: 2 });
    fireEvent.doubleClick(rowButton!);

    expect(mocks.openCurrent).toHaveBeenCalledTimes(1);
    expect(mocks.openCurrent).toHaveBeenCalledWith({
      id: "session-note-window",
      type: "sessions",
    });
    expect(mocks.timelineSelection.setAnchor).toHaveBeenCalledTimes(1);
    expect(mocks.timelineSelection.setAnchor).toHaveBeenCalledWith(
      "session-session-note-window",
    );
    expect(mocks.windowShow).toHaveBeenCalledWith({
      type: "note",
      value: "session-note-window",
    });
  });

  it("offers a standalone window action instead of a new tab action for session rows", () => {
    render(
      <TimelineItemComponent
        item={{
          type: "session",
          id: "session-note-window",
          data: {
            title: "Window Note",
            created_at: "2024-01-15T10:30:00.000Z",
          },
        }}
        precision="time"
        selected={false}
        timezone="UTC"
        multiSelected={false}
        flatItemKeys={["session-session-note-window"]}
      />,
    );

    const menu = mocks.nativeContextMenus.find((items) =>
      items.some((item) => item.id === "open-new-window"),
    );

    expect(menu?.some((item) => item.text === "Open in New Tab")).toBe(false);
    const openWindowItem = menu?.find((item) => item.id === "open-new-window");

    expect(openWindowItem).toMatchObject({
      id: "open-new-window",
      text: "Open in New Window",
    });

    openWindowItem?.action?.();

    expect(mocks.windowShow).toHaveBeenCalledWith({
      type: "note",
      value: "session-note-window",
    });
  });
});
