import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  currentTab: { type: "empty" } as { type: string } | null,
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
});
