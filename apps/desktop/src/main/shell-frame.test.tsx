import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  currentTab: { type: "empty" } as { type: string } | null,
  platform: "macos" as "linux" | "macos" | "windows",
  leftsidebar: {
    expanded: true,
  },
}));

vi.mock("@tauri-apps/plugin-os", () => ({
  platform: () => mocks.platform,
}));

vi.mock("./body", () => ({
  ClassicMainBody: ({ showSyncStatus }: { showSyncStatus?: boolean }) => (
    <div data-testid="classic-main-body">
      {showSyncStatus ? <div data-testid="sync-status-indicator" /> : null}
    </div>
  ),
}));

vi.mock("./windows-title-bar", () => ({
  WindowsTitleBar: () => <div data-testid="windows-title-bar" />,
}));

vi.mock("~/shared/main", () => ({
  MainShellBodyFrame: ({ children }: { children: React.ReactNode }) => (
    <div data-testid="main-shell-body-frame">{children}</div>
  ),
  MainShellScaffold: ({
    children,
    edgeToEdge,
    mainSurfaceChrome,
  }: {
    children: React.ReactNode;
    edgeToEdge?: boolean;
    mainSurfaceChrome?: "default" | "top" | "top-borderless" | "left";
  }) => (
    <div
      data-edge-to-edge={String(edgeToEdge)}
      data-main-surface-chrome={mainSurfaceChrome}
      data-testid="main-shell-scaffold"
    >
      {children}
    </div>
  ),
}));

vi.mock("~/contexts/shell", () => ({
  useShell: () => ({
    leftsidebar: mocks.leftsidebar,
  }),
}));

vi.mock("~/sidebar/toast", () => ({
  ToastNotifications: () => <div data-testid="toast-notifications" />,
}));

vi.mock("~/store/zustand/tabs", () => ({
  useTabs: (
    selector: (state: { currentTab: typeof mocks.currentTab }) => unknown,
  ) => selector({ currentTab: mocks.currentTab }),
}));

import { ClassicMainShellFrame } from "./shell-frame";

describe("ClassicMainShellFrame", () => {
  afterEach(() => {
    cleanup();
  });

  beforeEach(() => {
    mocks.currentTab = { type: "empty" };
    mocks.platform = "macos";
    mocks.leftsidebar.expanded = true;
  });

  it.each(["windows", "linux"] as const)(
    "places the custom title bar above the shell on %s",
    (runtimePlatform) => {
      mocks.platform = runtimePlatform;

      render(<ClassicMainShellFrame />);

      const titleBar = screen.getByTestId("windows-title-bar");
      const scaffold = screen.getByTestId("main-shell-scaffold");

      expect(titleBar.compareDocumentPosition(scaffold)).toBe(
        Node.DOCUMENT_POSITION_FOLLOWING,
      );
      expect(titleBar.parentElement?.contains(scaffold)).toBe(true);
    },
  );

  it("keeps native macOS chrome without the custom title bar", () => {
    render(<ClassicMainShellFrame />);

    expect(screen.queryByTestId("windows-title-bar")).toBeNull();
    expect(screen.getByTestId("main-shell-scaffold")).toBeTruthy();
  });

  it("uses left-edge main surface chrome while the sidebar timeline is expanded", () => {
    render(<ClassicMainShellFrame />);

    expect(screen.getByTestId("toast-notifications")).not.toBeNull();
    expect(screen.getByTestId("sync-status-indicator")).not.toBeNull();
    expect(
      screen
        .getByTestId("main-shell-scaffold")
        .getAttribute("data-main-surface-chrome"),
    ).toBe("left");
  });

  it("shows sync status in note views", () => {
    mocks.currentTab = { type: "sessions" };

    render(<ClassicMainShellFrame />);

    expect(screen.getByTestId("sync-status-indicator")).not.toBeNull();
  });

  it("hides sync status outside empty and note views", () => {
    mocks.currentTab = { type: "settings" };

    render(<ClassicMainShellFrame />);

    expect(screen.queryByTestId("sync-status-indicator")).toBeNull();
  });

  it("uses borderless top-edge main surface chrome while the sidebar timeline is collapsed", () => {
    mocks.leftsidebar.expanded = false;

    render(<ClassicMainShellFrame />);

    expect(screen.getByTestId("toast-notifications")).not.toBeNull();
    expect(
      screen
        .getByTestId("main-shell-scaffold")
        .getAttribute("data-main-surface-chrome"),
    ).toBe("top-borderless");
  });

  it.each(["settings", "automations"])(
    "uses left-edge main surface chrome for the %s custom sidebar",
    (type) => {
      mocks.currentTab = { type };

      render(<ClassicMainShellFrame />);

      expect(
        screen
          .getByTestId("main-shell-scaffold")
          .getAttribute("data-main-surface-chrome"),
      ).toBe("left");
    },
  );

  it("keeps left-edge main surface chrome for changelog tabs while expanded", () => {
    mocks.currentTab = { type: "changelog" };

    render(<ClassicMainShellFrame />);

    expect(
      screen
        .getByTestId("main-shell-scaffold")
        .getAttribute("data-main-surface-chrome"),
    ).toBe("left");
  });

  it("uses the full shell surface for onboarding", () => {
    mocks.currentTab = { type: "onboarding" };

    render(<ClassicMainShellFrame />);

    const scaffold = screen.getByTestId("main-shell-scaffold");

    expect(scaffold.getAttribute("data-edge-to-edge")).toBe("true");
    expect(scaffold.getAttribute("data-main-surface-chrome")).toBeNull();
    expect(screen.queryByTestId("sync-status-indicator")).toBeNull();
  });
});
