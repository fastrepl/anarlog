import * as stylex from "@stylexjs/stylex";
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  currentTab: { type: "empty" } as { type: string } | null,
  platform: "macos" as "linux" | "macos" | "windows",
}));

vi.mock("@tauri-apps/plugin-os", () => ({
  platform: () => mocks.platform,
}));

vi.mock("~/calendar/components/context", () => ({
  SyncProvider: ({ children }: { children: React.ReactNode }) => (
    <div data-testid="sync-provider">{children}</div>
  ),
}));

vi.mock("~/store/zustand/tabs", () => ({
  useTabs: (
    selector: (state: { currentTab: typeof mocks.currentTab }) => unknown,
  ) => selector({ currentTab: mocks.currentTab }),
}));

import { MainShellScaffold, mainShellScaffoldStyles } from "./shell-scaffold";

describe("MainShellScaffold", () => {
  afterEach(() => {
    cleanup();
    mocks.currentTab = { type: "empty" };
    mocks.platform = "macos";
  });

  it("keeps the top border for regular top chrome", () => {
    render(
      <MainShellScaffold mainSurfaceChrome="top">
        <div data-chat-floating-anchor data-testid="main-surface" />
      </MainShellScaffold>,
    );

    const shell = screen.getByTestId("main-app-shell");

    expectStyle(shell, mainShellScaffoldStyles.topChrome);
  });

  it("removes the top border for borderless top chrome", () => {
    render(
      <MainShellScaffold mainSurfaceChrome="top-borderless">
        <div data-chat-floating-anchor data-testid="main-surface" />
      </MainShellScaffold>,
    );

    const shell = screen.getByTestId("main-app-shell");

    expectStyle(shell, mainShellScaffoldStyles.topBorderlessChrome);
    expectNotStyle(shell, mainShellScaffoldStyles.leftPadding);
  });

  it.each([
    ["windows", "left"],
    ["windows", "top"],
    ["linux", "left"],
    ["linux", "top"],
  ] as const)(
    "does not add %s main surface rounding for %s chrome",
    (currentPlatform, mainSurfaceChrome) => {
      mocks.platform = currentPlatform;

      render(
        <MainShellScaffold mainSurfaceChrome={mainSurfaceChrome}>
          <div data-chat-floating-anchor data-testid="main-surface" />
        </MainShellScaffold>,
      );

      expect(screen.getByTestId("main-app-shell")).toBeTruthy();
      expect(screen.getByTestId("main-surface").className).toBe("");
    },
  );
});

function expectStyle(element: Element, sx: stylex.StyleXStyles) {
  const classNames = stylex.props(sx).className;
  expect(classNames).toBeTruthy();
  for (const className of classNames?.split(" ") ?? []) {
    expect(element.classList.contains(className)).toBe(true);
  }
}

function expectNotStyle(element: Element, sx: stylex.StyleXStyles) {
  const classNames = stylex.props(sx).className;
  expect(classNames).toBeTruthy();
  for (const className of classNames?.split(" ") ?? []) {
    expect(element.classList.contains(className)).toBe(false);
  }
}
