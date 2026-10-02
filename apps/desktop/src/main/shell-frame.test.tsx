import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  currentTab: { type: "empty" } as { type: string } | null,
  platform: "macos" as "linux" | "macos" | "windows",
  leftsidebar: {
    expanded: true,
    setExpanded: vi.fn(),
    setLocked: vi.fn(),
  },
}));

vi.mock("@tauri-apps/plugin-os", () => ({
  platform: () => mocks.platform,
}));

vi.mock("./body", () => ({
  ClassicMainBody: () => null,
}));

vi.mock("./windows-title-bar", () => ({
  WindowsTitleBar: () => <div data-testid="windows-title-bar" />,
}));

vi.mock("~/shared/main", () => ({
  MainShellBodyFrame: ({ children }: { children: React.ReactNode }) => children,
  MainShellScaffold: ({ children }: { children: React.ReactNode }) => children,
}));

vi.mock("~/sidebar/toast", () => ({
  ToastNotifications: () => null,
}));

vi.mock("~/devtools-bar", () => ({
  DevtoolsStatusBar: () => null,
}));

vi.mock("~/contexts/shell", () => ({
  useShell: () => ({ leftsidebar: mocks.leftsidebar }),
}));

vi.mock("~/store/zustand/tabs", () => ({
  useTabs: (
    selector: (state: { currentTab: typeof mocks.currentTab }) => unknown,
  ) => selector({ currentTab: mocks.currentTab }),
}));

import { ClassicMainShellFrame } from "./shell-frame";

describe("ClassicMainShellFrame", () => {
  afterEach(cleanup);

  beforeEach(() => {
    mocks.currentTab = { type: "empty" };
    mocks.platform = "macos";
    mocks.leftsidebar.expanded = true;
    mocks.leftsidebar.setExpanded.mockClear();
    mocks.leftsidebar.setLocked.mockClear();
  });

  it("opens the settings sidebar and restores its previous state on exit", () => {
    mocks.currentTab = { type: "settings" };
    mocks.leftsidebar.expanded = false;

    const { rerender } = render(<ClassicMainShellFrame />);

    expect(mocks.leftsidebar.setExpanded).toHaveBeenCalledWith(true);
    expect(mocks.leftsidebar.setLocked).toHaveBeenCalledWith(true);

    mocks.currentTab = { type: "empty" };
    rerender(<ClassicMainShellFrame />);

    expect(mocks.leftsidebar.setExpanded).toHaveBeenLastCalledWith(false);
    expect(mocks.leftsidebar.setLocked).toHaveBeenLastCalledWith(false);
  });

  it("unlocks the custom sidebar when the shell unmounts", () => {
    mocks.currentTab = { type: "calendar" };
    mocks.leftsidebar.expanded = false;

    const { unmount } = render(<ClassicMainShellFrame />);
    unmount();

    expect(mocks.leftsidebar.setExpanded).toHaveBeenLastCalledWith(false);
    expect(mocks.leftsidebar.setLocked).toHaveBeenLastCalledWith(false);
  });

  it.each([
    ["windows", true],
    ["linux", true],
    ["macos", false],
  ] as const)(
    "shows the custom title bar on %s",
    (runtimePlatform, visible) => {
      mocks.platform = runtimePlatform;

      render(<ClassicMainShellFrame />);

      if (visible) {
        expect(screen.getByTestId("windows-title-bar")).toBeTruthy();
      } else {
        expect(screen.queryByTestId("windows-title-bar")).toBeNull();
      }
    },
  );
});
