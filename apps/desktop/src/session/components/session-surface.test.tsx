import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

vi.mock("~/shared/main", () => ({
  StandardTabWrapper: ({ children }: { children: React.ReactNode }) => (
    <div data-testid="standard-tab-wrapper">{children}</div>
  ),
}));

import { SessionSurface } from "./session-surface";

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
    expect(contentWrapper?.className).not.toContain("mt-2");
    expect(contentWrapper?.className).toContain("min-h-0");
    expect(contentWrapper?.className).toContain("flex-1");
  });
});
