import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

vi.mock("~/store/zustand/tabs", () => ({
  useTabs: (selector: (state: { currentTab: { type: string } }) => unknown) =>
    selector({ currentTab: { type: "empty" } }),
}));

import { MainShellScaffold } from "./shell-scaffold";

describe("MainShellScaffold", () => {
  afterEach(() => {
    cleanup();
  });

  it("keeps only the left outer padding by default", () => {
    render(
      <MainShellScaffold>
        <div />
      </MainShellScaffold>,
    );

    const shell = screen.getByTestId("main-app-shell");

    expect(shell.className).toContain("pl-1");
    expect(shell.className).not.toContain("px-1");
    expect(shell.className).not.toContain("pb-1");
  });

  it("removes outer padding for the top-edge main surface", () => {
    render(
      <MainShellScaffold edgeToEdge>
        <div />
      </MainShellScaffold>,
    );

    const shell = screen.getByTestId("main-app-shell");

    expect(shell.className).not.toContain("px-1");
    expect(shell.className).not.toContain("pb-1");
    expect(shell.className).toContain(
      "[&_[data-chat-floating-anchor]]:rounded-t-xl",
    );
    expect(shell.className).toContain(
      "[&_[data-chat-floating-anchor]]:rounded-b-none",
    );
    expect(shell.className).toContain(
      "[&_[data-chat-floating-anchor]]:border-x-0",
    );
    expect(shell.className).toContain(
      "[&_[data-chat-floating-anchor]]:border-b-0",
    );
    expect(shell.className).toContain(
      "[&_[data-chat-floating-anchor][data-main-show-after-border-divider]]:!border-b",
    );
    expect(shell.className).toContain(
      "[&_[data-main-after-border-content][data-main-after-border-merged]_[data-session-transcript-card]]:border-x-0",
    );
    expect(shell.className).toContain(
      "[&_[data-main-after-border-content][data-main-after-border-merged]_[data-session-transcript-card]]:border-t-0",
    );
    expect(shell.className).toContain(
      "[&_[data-chat-floating-anchor]]:border-t",
    );
  });

  it("keeps only left chrome for the left-edge main surface", () => {
    render(
      <MainShellScaffold mainSurfaceChrome="left">
        <div />
      </MainShellScaffold>,
    );

    const shell = screen.getByTestId("main-app-shell");

    expect(shell.className).toContain("pl-1");
    expect(shell.className).not.toContain("pb-1");
    expect(shell.className).not.toContain("px-1");
    expect(shell.className).toContain(
      "[&_[data-chat-floating-anchor]]:rounded-l-xl",
    );
    expect(shell.className).toContain(
      "[&_[data-chat-floating-anchor]]:rounded-r-none",
    );
    expect(shell.className).toContain(
      "[&_[data-chat-floating-anchor][data-main-has-after-border]]:rounded-bl-none",
    );
    expect(shell.className).toContain(
      "[&_[data-chat-floating-anchor]]:border-y-0",
    );
    expect(shell.className).toContain(
      "[&_[data-chat-floating-anchor][data-main-show-after-border-divider]]:!border-b",
    );
    expect(shell.className).toContain(
      "[&_[data-chat-floating-anchor]]:border-r-0",
    );
    expect(shell.className).toContain(
      "[&_[data-chat-floating-anchor]]:border-l",
    );
    expect(shell.className).toContain(
      "[&_[data-main-after-border-content][data-main-after-border-merged]_[data-session-transcript-card]]:border-t-0",
    );
  });
});
