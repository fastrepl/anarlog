import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
} from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  newNote: vi.fn(),
  openSearch: vi.fn(),
  openNew: vi.fn(),
  invalidateResource: vi.fn(),
  clearSelection: vi.fn(),
  addDeletion: vi.fn(),
}));

vi.mock("~/shared/config", () => ({
  useConfigValue: () => undefined,
}));

vi.mock("~/shared/hooks/useNativeContextMenu", () => ({
  useNativeContextMenu: () => vi.fn(),
}));

vi.mock("~/shared/useNewNote", () => ({
  useNewNote: () => mocks.newNote,
}));

vi.mock("~/shared/open-note-dialog", () => ({
  useOpenNoteDialog: () => ({
    open: mocks.openSearch,
  }),
}));

vi.mock("~/store/tinybase/hooks", () => ({
  useIgnoredEvents: () => ({
    isIgnored: () => false,
  }),
}));

vi.mock("~/store/tinybase/store/deleteSession", () => ({
  captureSessionData: vi.fn(),
  deleteSessionCascade: vi.fn(),
  finalizeSessionDeletion: vi.fn(),
}));

vi.mock("~/store/tinybase/store/main", () => ({
  QUERIES: {
    timelineEvents: "timelineEvents",
    timelineSessions: "timelineSessions",
  },
  STORE_ID: "main",
  UI: {
    useIndexes: () => null,
    useResultTable: () => ({}),
    useStore: () => null,
  },
}));

vi.mock("~/store/zustand/tabs", () => ({
  useTabs: (selector: (state: unknown) => unknown) =>
    selector({
      currentTab: { type: "empty" },
      invalidateResource: mocks.invalidateResource,
      openNew: mocks.openNew,
    }),
}));

vi.mock("~/store/zustand/timeline-selection", () => ({
  useTimelineSelection: (selector: (state: unknown) => unknown) =>
    selector({
      clear: mocks.clearSelection,
      selectedIds: [],
    }),
}));

vi.mock("~/store/zustand/undo-delete", () => ({
  useUndoDelete: (selector: (state: unknown) => unknown) =>
    selector({
      addDeletion: mocks.addDeletion,
    }),
}));

vi.mock("./anchor", async () => {
  const React = await vi.importActual<typeof import("react")>("react");

  return {
    useAnchor: () => ({
      anchorNode: null,
      containerRef: React.useRef<HTMLDivElement>(null),
      isAnchorVisible: true,
      isScrolledPastAnchor: false,
      registerAnchor: vi.fn(),
      scrollToAnchor: vi.fn(),
    }),
    useAutoScrollToAnchor: vi.fn(),
  };
});

vi.mock("./item", () => ({
  TimelineItemComponent: () => <div data-testid="timeline-item" />,
}));

vi.mock("./realtime", async () => {
  const React = await vi.importActual<typeof import("react")>("react");

  return {
    CurrentTimeIndicator: React.forwardRef<HTMLDivElement>(
      function CurrentTimeIndicator(_props, ref) {
        return <div ref={ref} data-testid="current-time-indicator" />;
      },
    ),
    useCurrentTimeMs: () => Date.now(),
    useSmartCurrentTime: () => Date.now(),
  };
});

import { TimelineView } from ".";

describe("TimelineView", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  afterEach(() => {
    cleanup();
    vi.useRealTimers();
  });

  it("shows sidebar actions for creating and searching notes", () => {
    render(<TimelineView topChromeInset />);

    fireEvent.click(screen.getByRole("button", { name: "New note" }));
    fireEvent.click(screen.getByRole("button", { name: "Search" }));

    expect(mocks.newNote).toHaveBeenCalledTimes(1);
    expect(mocks.openSearch).toHaveBeenCalledTimes(1);
  });

  it("hides sidebar actions briefly after scrolling down", () => {
    vi.useFakeTimers();

    const { container } = render(<TimelineView topChromeInset />);
    const actions = getSidebarActions();
    const scroller = container.querySelector("[data-sidebar-timeline-scroll]");

    expect(scroller).toBeInstanceOf(HTMLDivElement);
    expect(actions.className).not.toContain("opacity-0");

    Object.defineProperty(scroller, "clientHeight", {
      configurable: true,
      value: 200,
    });
    Object.defineProperty(scroller, "scrollHeight", {
      configurable: true,
      value: 1200,
    });
    scroller!.scrollTop = 120;
    fireEvent.scroll(scroller!);

    expect(actions.className).toContain("opacity-0");

    act(() => {
      vi.advanceTimersByTime(900);
    });

    expect(actions.className).not.toContain("opacity-0");
  });
});

function getSidebarActions() {
  const actions = screen
    .getByRole("button", { name: "New note" })
    .closest("[data-sidebar-timeline-actions]");

  expect(actions).toBeInstanceOf(HTMLDivElement);

  return actions as HTMLDivElement;
}
