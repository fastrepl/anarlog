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

import { MainShellScaffold } from "./shell-scaffold";

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

    expect(shell.className).toContain(
      "[&_[data-chat-floating-anchor]]:border-t",
    );
    expect(shell.className).not.toContain(
      "[&_[data-chat-floating-anchor]]:!border-t-0",
    );
  });

  it("removes the top border for borderless top chrome", () => {
    render(
      <MainShellScaffold mainSurfaceChrome="top-borderless">
        <div data-chat-floating-anchor data-testid="main-surface" />
      </MainShellScaffold>,
    );

    const shell = screen.getByTestId("main-app-shell");

    expect(shell.className).toContain(
      "[&_[data-chat-floating-anchor]]:!border-t-0",
    );
    expect(shell.className).not.toContain("pl-1");
  });

  it.each([
    ["windows", "left", "[&_[data-chat-floating-anchor]]:rounded-tl-xl"],
    ["windows", "top", "[&_[data-chat-floating-anchor]]:rounded-t-xl"],
    ["linux", "left", "[&_[data-chat-floating-anchor]]:rounded-tl-xl"],
    ["linux", "top", "[&_[data-chat-floating-anchor]]:rounded-t-xl"],
  ] as const)(
    "adds %s main surface rounding for %s chrome",
    (currentPlatform, mainSurfaceChrome, roundedClass) => {
      mocks.platform = currentPlatform;

      render(
        <MainShellScaffold mainSurfaceChrome={mainSurfaceChrome}>
          <div data-chat-floating-anchor data-testid="main-surface" />
        </MainShellScaffold>,
      );

      expect(screen.getByTestId("main-app-shell").className).toContain(
        roundedClass,
      );
    },
  );
});
