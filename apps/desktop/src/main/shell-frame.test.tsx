import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  currentTab: { type: "empty" } as { type: string } | null,
  leftsidebar: {
    expanded: true,
    showDevtool: false,
  },
  sidebarTimelineEnabled: false,
}));

vi.mock("./body", () => ({
  ClassicMainBody: () => <div data-testid="classic-main-body" />,
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
    mainSurfaceChrome?: "default" | "top" | "left";
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

vi.mock("~/shared/config", () => ({
  useConfigValue: () => mocks.sidebarTimelineEnabled,
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
    mocks.leftsidebar.expanded = true;
    mocks.leftsidebar.showDevtool = false;
    mocks.sidebarTimelineEnabled = false;
  });

  it("uses top-edge main surface chrome in top timeline mode", () => {
    render(<ClassicMainShellFrame />);

    expect(
      screen
        .getByTestId("main-shell-scaffold")
        .getAttribute("data-main-surface-chrome"),
    ).toBe("top");
  });

  it("uses left-edge main surface chrome in sidebar timeline mode", () => {
    mocks.sidebarTimelineEnabled = true;

    render(<ClassicMainShellFrame />);

    expect(
      screen
        .getByTestId("main-shell-scaffold")
        .getAttribute("data-main-surface-chrome"),
    ).toBe("left");
  });

  it("uses left-edge main surface chrome for custom sidebar tabs", () => {
    mocks.currentTab = { type: "settings" };

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
  });
});
