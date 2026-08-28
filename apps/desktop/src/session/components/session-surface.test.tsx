import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

vi.mock("~/shared/main", () => ({
  StandardContentWrapper: ({ children }: { children: React.ReactNode }) => (
    <div data-testid="standard-tab-wrapper">{children}</div>
  ),
}));

import { SessionSurface, sessionSurfaceStyles } from "./session-surface";

import { expectStyle } from "~/session/stylex-test";

describe("SessionSurface", () => {
  afterEach(() => {
    cleanup();
  });

  it("keeps note content tight below the outer header", () => {
    render(
      <SessionSurface header={<div data-testid="header" />}>
        <div data-testid="content" />
      </SessionSurface>,
    );

    const headerWrapper = screen.getByTestId("header").parentElement;
    const contentWrapper = screen.getByTestId("content").parentElement;

    expect(headerWrapper?.hasAttribute("data-tauri-drag-region")).toBe(true);
    expectStyle(contentWrapper, sessionSurfaceStyles.content);
  });
});
