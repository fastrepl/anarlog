import { act, cleanup, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  currentTab: {
    active: true,
    pinned: false,
    slotId: "slot-home",
    type: "empty",
  } as ({ type: string } & Record<string, unknown>) | null,
  leftsidebar: {
    expanded: true,
    toggleExpanded: vi.fn(),
  },
  onPanelLayout: null as null | ((sizes: number[]) => void),
}));

vi.mock("@hypr/ui/components/ui/resizable", () => ({
  ResizablePanelGroup: ({
    autoSaveId,
    children,
    className,
    direction,
    onLayout,
  }: {
    autoSaveId?: string;
    children: React.ReactNode;
    className?: string;
    direction: string;
    onLayout?: (sizes: number[]) => void;
  }) => {
    mocks.onPanelLayout = onLayout ?? null;

    return (
      <div
        data-auto-save-id={autoSaveId}
        data-class-name={className}
        data-direction={direction}
        data-testid="panel-group"
      >
        {children}
      </div>
    );
  },
  ResizablePanel: ({
    children,
    className,
    defaultSize,
    maxSize,
    minSize,
    style,
  }: {
    children: React.ReactNode;
    className?: string;
    defaultSize?: number;
    maxSize?: number;
    minSize?: number;
    style?: React.CSSProperties;
  }) => (
    <div
      data-class-name={className}
      data-default-size={defaultSize}
      data-max-size={maxSize}
      data-min-size={minSize}
      data-max-width={style?.maxWidth}
      data-min-width={style?.minWidth}
      data-testid="panel"
    >
      {children}
    </div>
  ),
  ResizableHandle: ({ className }: { className?: string }) => (
    <div data-class-name={className} data-testid="resize-handle" />
  ),
}));

vi.mock("~/contexts/shell", () => ({
  useShell: () => ({
    leftsidebar: mocks.leftsidebar,
  }),
}));

vi.mock("~/store/zustand/tabs", () => ({
  uniqueIdfromTab: (tab: { type: string }) => tab.type,
  useTabs: (
    selector: (state: { currentTab: typeof mocks.currentTab }) => unknown,
  ) => selector({ currentTab: mocks.currentTab }),
}));

vi.mock("./shell-sidebar", () => ({
  ClassicMainSidebar: () =>
    mocks.leftsidebar.expanded && mocks.currentTab?.type !== "onboarding" ? (
      <aside data-testid="classic-main-sidebar" />
    ) : null,
}));

vi.mock("./tab-content", () => ({
  ClassicMainTabContent: ({ tab }: { tab: { type: string } }) => (
    <div data-tab-type={tab.type} data-testid="tab-content" />
  ),
}));

vi.mock("./update-banner", () => ({
  SidebarTimelineUpdateButton: () => <button type="button">Update</button>,
  useDesktopUpdateControl: () => ({ status: null, version: null }),
}));

vi.mock("./useTabsShortcuts", () => ({
  useClassicMainTabsShortcuts: () => ({ runEscapeShortcut: vi.fn() }),
}));

vi.mock("~/session/components/bottom-accessory/global-live", () => ({
  GlobalLiveTranscriptAccessory: ({
    children,
  }: {
    children: React.ReactNode;
  }) => <div data-testid="global-live-accessory">{children}</div>,
}));

vi.mock("~/shared/open-note-dialog", () => ({
  useOpenNoteDialog: () => ({ open: vi.fn() }),
}));

vi.mock("~/shared/useNewNote", () => ({
  useNewNote: () => vi.fn(),
}));

vi.mock("~/sidebar/timeline/upcoming-meeting", () => ({
  useSidebarUpcomingMeetingStatus: () => null,
}));

import { ClassicMainBody } from "./body";

describe("ClassicMainBody", () => {
  beforeEach(() => {
    cleanup();
    mocks.currentTab = {
      active: true,
      pinned: false,
      slotId: "slot-home",
      type: "empty",
    };
    mocks.leftsidebar.expanded = true;
    mocks.leftsidebar.toggleExpanded.mockClear();
    mocks.onPanelLayout = null;
  });

  it("wraps the expanded left sidebar in a persistent resizable panel", () => {
    render(<ClassicMainBody />);

    expect(screen.getByTestId("panel-group").dataset.direction).toBe(
      "horizontal",
    );
    expect(screen.getByTestId("panel-group").dataset.autoSaveId).toBe(
      "classic-main-sidebar",
    );
    expect(screen.getByTestId("classic-main-sidebar")).toBeTruthy();
    expect(screen.getByTestId("resize-handle").dataset.className).toContain(
      "after:w-2",
    );

    const panels = screen.getAllByTestId("panel");
    expect(panels).toHaveLength(2);
    expect(panels[0]?.dataset.defaultSize).toBe("20");
    expect(panels[0]?.dataset.minSize).toBe("12");
    expect(panels[0]?.dataset.maxSize).toBe("32");
    expect(panels[0]?.dataset.minWidth).toBe("200");
    expect(panels[0]?.dataset.maxWidth).toBe("360");

    const sidebarChrome = document.querySelector<HTMLElement>(
      "[data-left-sidebar-chrome]",
    );

    expect(sidebarChrome?.style.width).toBe("20%");
    expect(sidebarChrome?.style.minWidth).toBe("200px");
    expect(sidebarChrome?.style.maxWidth).toBe("360px");
    expect(sidebarChrome?.className).not.toContain("w-[200px]");

    act(() => {
      mocks.onPanelLayout?.([24, 76]);
    });

    expect(sidebarChrome?.style.width).toBe("24%");
  });

  it("keeps the collapsed layout free of the sidebar resize handle", () => {
    mocks.leftsidebar.expanded = false;

    render(<ClassicMainBody />);

    expect(screen.queryByTestId("classic-main-sidebar")).toBeNull();
    expect(screen.queryByTestId("resize-handle")).toBeNull();
    expect(screen.getAllByTestId("panel")).toHaveLength(1);
  });

  it("does not reserve a sidebar panel during onboarding", () => {
    mocks.currentTab = { type: "onboarding" };

    render(<ClassicMainBody />);

    expect(screen.queryByTestId("classic-main-sidebar")).toBeNull();
    expect(screen.queryByTestId("resize-handle")).toBeNull();
    expect(screen.getAllByTestId("panel")).toHaveLength(1);
  });
});
