import { cleanup, render, screen } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/plugin-os", () => ({
  platform: () => "macos",
}));

vi.mock("@anlg/ui/components/ui/resizable", () => {
  return {
    ResizablePanelGroup: ({ children }: { children: ReactNode }) => (
      <div>{children}</div>
    ),
    ResizablePanel: ({ children }: { children: ReactNode }) => (
      <div>{children}</div>
    ),
  };
});

import { StandardContentWrapper } from "./index";

describe("StandardContentWrapper", () => {
  afterEach(() => {
    cleanup();
  });

  it("renders the main area and floating button inside the main surface", () => {
    render(
      <StandardContentWrapper floatingButton={<button>Record</button>}>
        <div data-testid="main-area" />
      </StandardContentWrapper>,
    );

    expect(screen.getByTestId("main-area")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Record" })).toBeTruthy();
  });
});
