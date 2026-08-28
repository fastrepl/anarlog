import * as stylex from "@stylexjs/stylex";
import { cleanup, render, screen } from "@testing-library/react";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  platform: "macos" as "linux" | "macos" | "windows",
}));

vi.mock("@tauri-apps/plugin-os", () => ({
  platform: () => mocks.platform,
}));

vi.mock("@anlg/ui/components/ui/resizable", () => {
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

import { mainStyles, StandardContentWrapper } from "./index";

describe("StandardContentWrapper", () => {
  beforeEach(() => {
    cleanup();
    mocks.platform = "macos";
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
    expectStyle(
      document.querySelector("[data-chat-floating-anchor]"),
      mainStyles.floatingAnchorMacos,
    );
  });

  it.each(["windows", "linux"] as const)(
    "uses square main panel corners on %s",
    (currentPlatform) => {
      mocks.platform = currentPlatform;

      render(
        <StandardContentWrapper>
          <div data-testid="main-area" />
        </StandardContentWrapper>,
      );

      expectNotStyle(
        document.querySelector("[data-chat-floating-anchor]"),
        mainStyles.floatingAnchorMacos,
      );
    },
  );

  it("renders the floating button inside the main surface", () => {
    render(
      <StandardContentWrapper floatingButton={<button>Record</button>}>
        <div data-testid="main-area" />
      </StandardContentWrapper>,
    );

    expect(screen.getByRole("button", { name: "Record" })).toBeTruthy();
  });
});

function expectStyle(element: Element | null, sx: stylex.StyleXStyles) {
  expect(element).toBeTruthy();
  const classNames = stylex.props(sx).className;
  expect(classNames).toBeTruthy();
  for (const className of classNames?.split(" ") ?? []) {
    expect(element?.classList.contains(className)).toBe(true);
  }
}

function expectNotStyle(element: Element | null, sx: stylex.StyleXStyles) {
  expect(element).toBeTruthy();
  const classNames = stylex.props(sx).className;
  expect(classNames).toBeTruthy();
  for (const className of classNames?.split(" ") ?? []) {
    expect(element?.classList.contains(className)).toBe(false);
  }
}
