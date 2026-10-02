import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
} from "@testing-library/react";
import { forwardRef, useImperativeHandle } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  runtimePlatform: null as "windows" | "linux" | null,
  currentTab: { type: "empty" } as
    | ({ type: string } & Record<string, unknown>)
    | null,
  leftsidebar: {
    expanded: true,
    setExpanded: vi.fn(),
    toggleExpanded: vi.fn(),
  },
  onPanelCollapse: null as null | (() => void),
  onPanelLayout: null as null | ((sizes: number[]) => void),
  onResizeDragging: null as null | ((isDragging: boolean) => void),
  leftSidebarPanelHandle: {
    expand: vi.fn(),
    getSize: vi.fn(() => 12.5),
    resize: vi.fn(),
  },
  tabContentRenderCount: 0,
}));

vi.mock("@tauri-apps/plugin-os", () => ({
  platform: () => {
    if (mocks.runtimePlatform === null) {
      throw new Error("Tauri runtime unavailable");
    }

    return mocks.runtimePlatform;
  },
}));

vi.mock("@anlg/ui/components/ui/resizable", () => ({
  ResizablePanelGroup: ({
    children,
    onLayout,
  }: {
    children: React.ReactNode;
    onLayout?: (sizes: number[]) => void;
  }) => {
    mocks.onPanelLayout = onLayout ?? null;
    return <div data-testid="panel-group">{children}</div>;
  },
  ResizablePanel: forwardRef<
    unknown,
    {
      children: React.ReactNode;
      id?: string;
      onCollapse?: () => void;
    }
  >(({ children, id, onCollapse }, ref) => {
    useImperativeHandle(ref, () => mocks.leftSidebarPanelHandle);

    if (id === "classic-main-sidebar-left") {
      mocks.onPanelCollapse = onCollapse ?? null;
    }

    return <div data-testid="panel">{children}</div>;
  }),
  ResizableHandle: ({
    onDragging,
  }: {
    onDragging?: (isDragging: boolean) => void;
  }) => {
    mocks.onResizeDragging = onDragging ?? null;
    return <div data-testid="resize-handle" />;
  },
}));

vi.mock("~/contexts/shell", () => ({
  useShell: () => ({ leftsidebar: mocks.leftsidebar }),
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
  ClassicMainTabContent: ({ tab }: { tab: { type: string } }) => {
    mocks.tabContentRenderCount += 1;
    return <div data-tab-type={tab.type} data-testid="tab-content" />;
  },
}));

vi.mock("~/sidebar/note-filter-menu", () => ({
  SidebarNoteFilterMenu: () => <button type="button">Sort notes</button>,
}));

vi.mock("./useShortcuts", () => ({
  useClassicMainShortcuts: () => ({ runEscapeShortcut: vi.fn() }),
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
    mocks.runtimePlatform = null;
    mocks.currentTab = { type: "empty" };
    mocks.leftsidebar.expanded = true;
    mocks.leftsidebar.setExpanded.mockClear();
    mocks.leftsidebar.toggleExpanded.mockClear();
    mocks.onPanelCollapse = null;
    mocks.onPanelLayout = null;
    mocks.onResizeDragging = null;
    mocks.leftSidebarPanelHandle.expand.mockClear();
    mocks.leftSidebarPanelHandle.getSize.mockClear();
    mocks.leftSidebarPanelHandle.resize.mockClear();
    mocks.tabContentRenderCount = 0;
  });

  it("updates sidebar sizing during drag without rerendering tab content", () => {
    render(<ClassicMainBody />);

    const bodyRoot = screen.getByTestId("panel-group").parentElement;

    act(() => {
      mocks.onResizeDragging?.(true);
    });

    const renderCountAfterDragStart = mocks.tabContentRenderCount;

    act(() => {
      mocks.onPanelLayout?.([14, 86]);
      mocks.onPanelLayout?.([16, 84]);
      mocks.onPanelLayout?.([18, 82]);
    });

    expect(bodyRoot?.style.getPropertyValue("--left-sidebar-panel-size")).toBe(
      "18",
    );
    expect(bodyRoot?.style.getPropertyValue("--left-sidebar-panel-width")).toBe(
      "18%",
    );
    expect(mocks.tabContentRenderCount).toBe(renderCountAfterDragStart);

    act(() => {
      mocks.onResizeDragging?.(false);
    });

    expect(bodyRoot?.style.getPropertyValue("--left-sidebar-panel-size")).toBe(
      "18",
    );
  });

  it("collapses the sidebar when the resize handle snaps below the threshold", () => {
    render(<ClassicMainBody />);

    act(() => {
      mocks.onResizeDragging?.(true);
      mocks.onPanelLayout?.([0, 100]);
      mocks.onPanelCollapse?.();
    });

    expect(mocks.leftsidebar.setExpanded).toHaveBeenCalledWith(false);
    expect(mocks.leftsidebar.toggleExpanded).not.toHaveBeenCalled();
    expect(mocks.leftSidebarPanelHandle.resize).toHaveBeenCalledOnce();
  });

  it("resizes the collapsed sidebar panel before reopening it", () => {
    mocks.leftsidebar.expanded = false;

    const { rerender } = render(<ClassicMainBody />);
    expect(screen.queryByTestId("classic-main-sidebar")).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: "Show sidebar" }));

    expect(mocks.leftSidebarPanelHandle.resize).toHaveBeenCalledOnce();
    expect(mocks.leftSidebarPanelHandle.expand).not.toHaveBeenCalled();
    expect(mocks.leftsidebar.toggleExpanded).toHaveBeenCalledTimes(1);

    mocks.leftsidebar.expanded = true;
    rerender(<ClassicMainBody />);

    expect(screen.getByTestId("classic-main-sidebar")).toBeTruthy();
  });

  it("does not reserve a sidebar panel during onboarding", () => {
    mocks.currentTab = { type: "onboarding" };

    render(<ClassicMainBody />);

    expect(screen.queryByTestId("classic-main-sidebar")).toBeNull();
    expect(screen.queryByTestId("resize-handle")).toBeNull();
    expect(screen.getAllByTestId("panel")).toHaveLength(1);
  });
});
