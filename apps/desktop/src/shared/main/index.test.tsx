import { cleanup, render, screen } from "@testing-library/react";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@hypr/ui/components/ui/resizable", () => {
  return {
    ResizablePanelGroup: ({
      children,
      direction,
    }: {
      children: ReactNode;
      direction: string;
    }) => (
      <div data-direction={direction} data-testid="panel-group">
        {children}
      </div>
    ),
    ResizablePanel: ({
      children,
      className,
      defaultSize,
      minSize,
    }: {
      children: ReactNode;
      className?: string;
      defaultSize?: number;
      minSize?: number;
    }) => {
      return (
        <div
          data-default-size={defaultSize}
          data-class-name={className}
          data-min-size={minSize}
          data-testid="panel"
        >
          {children}
        </div>
      );
    },
  };
});

import { StandardContentWrapper } from "./index";

describe("StandardContentWrapper", () => {
  beforeEach(() => {
    cleanup();
  });

  it("renders a single full-height main panel", () => {
    render(
      <StandardContentWrapper>
        <div data-testid="main-area" />
      </StandardContentWrapper>,
    );

    expect(screen.getByTestId("panel-group").dataset.direction).toBe(
      "vertical",
    );
    expect(screen.getAllByTestId("panel")).toHaveLength(1);
    expect(screen.getByTestId("panel").dataset.defaultSize).toBe("100");
    expect(screen.getByTestId("panel").dataset.minSize).toBe("35");
    expect(screen.getByTestId("main-area")).toBeTruthy();
    expect(
      document.querySelector("[data-chat-floating-anchor]")?.className,
    ).toContain("rounded-xl");
  });

  it("renders the floating button inside the main surface", () => {
    render(
      <StandardContentWrapper floatingButton={<button>Record</button>}>
        <div data-testid="main-area" />
      </StandardContentWrapper>,
    );

    expect(screen.getByRole("button", { name: "Record" })).toBeTruthy();
  });
});
