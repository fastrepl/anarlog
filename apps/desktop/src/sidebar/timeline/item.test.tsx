import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  addDeletion: vi.fn(),
  ignoreEvent: vi.fn(),
  invalidateResource: vi.fn(),
  isIgnored: vi.fn(() => false),
  openCurrent: vi.fn(),
  openNew: vi.fn(),
  platform: "macos",
  preloadSession: vi.fn(() => Promise.resolve(null)),
  sessionMode: "inactive",
  stop: vi.fn(),
  getOrCreateSessionForEventId: vi.fn(() => Promise.resolve("session-event")),
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
  authAvailable: false as boolean | null,
  revealedNoteIds: {} as Record<string, true>,
  configValues: {} as Record<string, boolean | undefined>,
  seriesRules: [] as Array<{ series_id: string; folder_path: string }>,
}));

vi.mock("@tauri-apps/plugin-os", () => ({
  platform: () => mocks.platform,
}));

vi.mock("@anlg/plugin-fs-sync", () => ({
  commands: {
    sessionDir: vi.fn(() => Promise.resolve({ status: "ok", data: "" })),
  },
}));

vi.mock("@anlg/plugin-opener2", () => ({
  commands: {
    openPath: vi.fn(() => Promise.resolve()),
  },
}));

vi.mock("@anlg/plugin-windows", () => ({
  commands: {
    windowShow: mocks.windowShow,
  },
}));

vi.mock("@anlg/ui/components/ui/dancing-sticks", () => ({
  DancingSticks: ({ amplitude }: { amplitude: number }) => (
    <span data-amplitude={amplitude} data-testid="dancing-sticks" />
  ),
}));

vi.mock("@anlg/ui/components/ui/spinner", () => ({
  Spinner: () => <span data-testid="spinner" />,
}));

vi.mock("@anlg/ui/components/ui/tooltip", () => ({
  Tooltip: ({ children }: { children: ReactNode }) => <>{children}</>,
  TooltipContent: ({ children }: { children: ReactNode }) => <>{children}</>,
  TooltipTrigger: ({ children }: { children: ReactNode }) => <>{children}</>,
}));

vi.mock("~/session/hooks/useEnhancedNotes", () => ({
  useIsSessionEnhancing: () => false,
}));

vi.mock("~/session/queries", () => ({
  getOrCreateSessionForEventId: mocks.getOrCreateSessionForEventId,
  preloadSession: mocks.preloadSession,
}));

vi.mock("~/lock/notes", () => ({
  revealLockedNote: vi.fn(() => Promise.resolve(true)),
  setSessionLocked: vi.fn(() => Promise.resolve(true)),
}));

vi.mock("~/lock/store", () => ({
  useAppLock: (
    selector: (state: {
      available: boolean | null;
      revealedNoteIds: Record<string, true>;
    }) => unknown,
  ) =>
    selector({
      available: mocks.authAvailable,
      revealedNoteIds: mocks.revealedNoteIds,
    }),
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

vi.mock("~/calendar/ignored-events", () => ({
  useIgnoredEvents: () => ({
    ignoreEvent: mocks.ignoreEvent,
    ignoreSeries: vi.fn(),
    isIgnored: mocks.isIgnored,
    unignoreEvent: vi.fn(),
    unignoreSeries: vi.fn(),
  }),
}));

vi.mock("~/calendar/series-folders", async (importOriginal) => {
  const actual =
    await importOriginal<typeof import("~/calendar/series-folders")>();
  return {
    ...actual,
    useSeriesFolderRules: () => mocks.seriesRules,
  };
});

vi.mock("~/shared/config", () => ({
  useConfigValue: (key: string) => mocks.configValues[key],
}));

vi.mock("~/store/zustand/live-title", () => ({
  useSessionTitle: () => mocks.storeTitle,
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
      live: { amplitude: { mic: 0, speaker: 0 } },
      stop: mocks.stop,
    }),
}));

import { TimelineItemComponent } from "./item";

import { resetSidebarNotes } from "~/sidebar/note-filter";

describe("TimelineItemComponent", () => {
  beforeEach(() => {
    cleanup();
    mocks.sessionMode = "inactive";
    mocks.storeTitle = "Live Note";
    mocks.stop.mockClear();
    mocks.openCurrent.mockClear();
    mocks.openNew.mockClear();
    mocks.platform = "macos";
    mocks.preloadSession.mockReset();
    mocks.preloadSession.mockResolvedValue(null);
    mocks.authAvailable = false;
    mocks.revealedNoteIds = {};
    mocks.windowShow.mockClear();
    mocks.nativeContextMenus = [];
    mocks.timelineSelection.selectedIds = [];
    mocks.timelineSelection.setAnchor.mockClear();
    mocks.timelineSelection.selectRange.mockClear();
    mocks.timelineSelection.toggleSelect.mockClear();
    mocks.configValues = {};
    mocks.seriesRules = [];
    resetSidebarNotes();
  });

  function renderSession(
    id: string,
    data: { locked?: number; folder_id?: string; event_json?: string } = {},
    props: { selected?: boolean } = {},
  ) {
    return render(
      <TimelineItemComponent
        item={{
          type: "session",
          id,
          data: {
            title: "Note",
            created_at: "2024-01-15T10:30:00.000Z",
            ...data,
          },
        }}
        precision="time"
        selected={props.selected ?? false}
        timezone="UTC"
        multiSelected={false}
        flatItemKeys={[`session-${id}`]}
      />,
    );
  }

  function findMenuItem(id: string) {
    return mocks.nativeContextMenus.flat().find((item) => item.id === id);
  }

  function rowButton() {
    return screen.getByText("Live Note").closest("button")!;
  }

  it("stops listening from the active session row without opening it", () => {
    mocks.sessionMode = "active";
    renderSession("session-live", {}, { selected: true });

    fireEvent.click(screen.getByRole("button", { name: "Stop listening" }));

    expect(mocks.stop).toHaveBeenCalledOnce();
    expect(mocks.openCurrent).not.toHaveBeenCalled();
  });

  it("preloads a session before opening it in the current tab", async () => {
    renderSession("session-note");

    fireEvent.pointerDown(rowButton());
    fireEvent.click(rowButton(), { detail: 1 });

    expect(mocks.timelineSelection.setAnchor).toHaveBeenCalledWith(
      "session-session-note",
    );
    expect(mocks.preloadSession).toHaveBeenCalledWith("session-note");
    await waitFor(() => {
      expect(mocks.openCurrent).toHaveBeenCalledWith({
        id: "session-note",
        type: "sessions",
      });
    });
  });

  it("keeps the current note open until the target session is preloaded", async () => {
    let finishPreload: ((value: null) => void) | undefined;
    mocks.preloadSession.mockReturnValue(
      new Promise((resolve) => {
        finishPreload = resolve;
      }),
    );
    renderSession("slow-session");

    fireEvent.click(rowButton());

    expect(mocks.openCurrent).not.toHaveBeenCalled();
    expect(screen.getByTestId("spinner")).toBeTruthy();

    finishPreload?.(null);
    await waitFor(() => {
      expect(mocks.openCurrent).toHaveBeenCalledWith({
        id: "slow-session",
        type: "sessions",
      });
    });
  });

  it("opens a standalone note window when a session row is double-clicked", async () => {
    renderSession("session-note-window");

    fireEvent.click(rowButton(), { detail: 1 });
    fireEvent.click(rowButton(), { detail: 2 });
    fireEvent.doubleClick(rowButton());

    await waitFor(() => {
      expect(mocks.openCurrent).toHaveBeenCalledTimes(1);
    });
    expect(mocks.windowShow).toHaveBeenCalledWith({
      type: "note",
      value: "session-note-window",
    });
  });

  it("opens a session in a new window from the context menu", () => {
    renderSession("session-note-window");

    findMenuItem("open-new-window")?.action?.();

    expect(mocks.windowShow).toHaveBeenCalledWith({
      type: "note",
      value: "session-note-window",
    });
  });

  it.each([
    [false, false],
    [true, true],
  ])(
    "offers lock note only when device authentication is available (%s)",
    (available, offered) => {
      mocks.authAvailable = available;
      renderSession("session-lock");

      expect(Boolean(findMenuItem("lock"))).toBe(offered);
    },
  );

  it.each([
    ["hidden", {}, "Locked note", "Unlock Note"],
    [
      "revealed",
      { "session-locked": true as const },
      "Unlock Note",
      "Locked note",
    ],
  ])(
    "marks a %s locked note",
    (_, revealedNoteIds, shownLabel, hiddenLabel) => {
      mocks.revealedNoteIds = revealedNoteIds;
      renderSession("session-locked", { locked: 1 }, { selected: true });

      expect(screen.getByLabelText(shownLabel)).toBeTruthy();
      expect(screen.queryByLabelText(hiddenLabel)).toBeNull();
    },
  );

  it.each([true, false])(
    "shows the folder tag on a filed session row when selected=%s",
    (selected) => {
      mocks.configValues = { sidebar_show_folder: true };
      renderSession("session-filed", { folder_id: "Work" }, { selected });

      expect(screen.getByText("Work")).toBeTruthy();
    },
  );

  it.each([true, false])(
    "shows the folder tag on a filed event row when selected=%s",
    (selected) => {
      mocks.configValues = { sidebar_show_folder: true };
      render(
        <TimelineItemComponent
          item={{
            type: "event",
            id: "event-1",
            data: {
              title: "Standup",
              started_at: "2026-10-03T03:00:00.000Z",
              has_recurrence_rules: false,
              session_folder: "Work",
            },
          }}
          precision="time"
          selected={selected}
          timezone="UTC"
          multiSelected={false}
          flatItemKeys={["event-event-1"]}
        />,
      );

      expect(screen.getByText("Work")).toBeTruthy();
    },
  );

  it("shows the rule folder on a sessionless event row covered by a rule", () => {
    mocks.configValues = { sidebar_show_folder: true };
    mocks.seriesRules = [{ series_id: "series-1", folder_path: "Work" }];
    render(
      <TimelineItemComponent
        item={{
          type: "event",
          id: "event-1",
          data: {
            title: "Standup",
            started_at: "2026-10-03T03:00:00.000Z",
            has_recurrence_rules: true,
            recurrence_series_id: "series-1",
            session_folder: "",
          },
        }}
        precision="time"
        selected={false}
        timezone="UTC"
        multiSelected={false}
        flatItemKeys={["event-event-1"]}
      />,
    );

    expect(screen.getByText("Work")).toBeTruthy();
  });

  it("shows the rule folder on an unfiled session row covered by a rule", () => {
    mocks.configValues = { sidebar_show_folder: true };
    mocks.seriesRules = [{ series_id: "series-1", folder_path: "Work" }];
    renderSession("session-unfiled", {
      folder_id: "",
      event_json: JSON.stringify({ recurrence_series_id: "series-1" }),
    });

    expect(screen.getByText("Work")).toBeTruthy();
  });

  it("prefers the note folder over the rule folder on an overridden row", () => {
    mocks.configValues = { sidebar_show_folder: true };
    mocks.seriesRules = [{ series_id: "series-1", folder_path: "Work" }];
    renderSession("session-override", {
      folder_id: "Personal",
      event_json: JSON.stringify({ recurrence_series_id: "series-1" }),
    });

    expect(screen.getByText("Personal")).toBeTruthy();
    expect(screen.queryByText("Work")).toBeNull();
  });
});
