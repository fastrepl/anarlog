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

  it("keeps default outer padding", () => {
    render(
      <MainShellScaffold>
        <div />
      </MainShellScaffold>,
    );

    const shell = screen.getByTestId("main-app-shell");

    expect(shell.className).toContain("px-1");
    expect(shell.className).toContain("pb-1");
  });

  it("removes outer padding when edge-to-edge", () => {
    render(
      <MainShellScaffold edgeToEdge>
        <div />
      </MainShellScaffold>,
    );

    const shell = screen.getByTestId("main-app-shell");

    expect(shell.className).not.toContain("px-1");
    expect(shell.className).not.toContain("pb-1");
    expect(shell.className).toContain(
      "[&_[data-chat-floating-anchor]]:border-x-0",
    );
    expect(shell.className).toContain(
      "[&_[data-chat-floating-anchor]]:border-b-0",
    );
    expect(shell.className).toContain(
      "[&_[data-chat-floating-anchor]]:border-t",
    );
  });
});
