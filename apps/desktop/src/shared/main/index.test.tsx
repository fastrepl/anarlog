import { cleanup, render, screen } from "@testing-library/react";
import * as React from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const { resizeMock } = vi.hoisted(() => ({
  resizeMock: vi.fn(),
}));

vi.mock("@hypr/ui/components/ui/resizable", async () => {
  const React = await vi.importActual<typeof import("react")>("react");
  return {
    ResizablePanelGroup: ({
      children,
      direction,
    }: {
      children: React.ReactNode;
      direction: string;
    }) => (
      <div data-direction={direction} data-testid="panel-group">
        {children}
      </div>
    ),
    ResizablePanel: React.forwardRef<
      { resize: (size: number) => void },
      {
        children: React.ReactNode;
        className?: string;
        defaultSize?: number;
        maxSize?: number;
        minSize?: number;
      }
    >(function ResizablePanel(
      { children, className, defaultSize, maxSize, minSize },
      ref,
    ) {
      React.useImperativeHandle(ref, () => ({
        resize: resizeMock,
      }));

      return (
        <div
          data-default-size={defaultSize}
          data-class-name={className}
          data-max-size={maxSize}
          data-min-size={minSize}
          data-testid="panel"
        >
          {children}
        </div>
      );
    }),
    ResizableHandle: ({
      className,
      disabled,
    }: {
      className?: string;
      disabled?: boolean;
    }) => (
      <div
        data-class-name={className}
        data-disabled={disabled ? "true" : "false"}
        data-testid="resize-handle"
      />
    ),
  };
});

import { StandardTabWrapper } from "./index";

describe("StandardTabWrapper", () => {
  beforeEach(() => {
    cleanup();
    resizeMock.mockClear();
  });

  it("renders a vertical resizable split for expandable bottom content", () => {
    render(
      <StandardTabWrapper
        afterBorder={<div data-testid="bottom-area" />}
        afterBorderExpanded
        afterBorderResizable
        bottomBorderHandle={<button>Live</button>}
      >
        <div data-testid="main-area" />
      </StandardTabWrapper>,
    );

    expect(screen.getByTestId("panel-group").dataset.direction).toBe(
      "vertical",
    );
    expect(screen.getByTestId("resize-handle").dataset.disabled).toBe("false");
    expect(screen.getByTestId("resize-handle").dataset.className).toContain(
      "data-[panel-group-direction=vertical]:-mb-px",
    );

    const panels = screen.getAllByTestId("panel");
    expect(panels[0]?.dataset.defaultSize).toBe("78");
    expect(panels[1]?.dataset.defaultSize).toBe("22");
    expect(panels[1]?.dataset.className).toContain("min-h-[96px]");
    expect(panels[1]?.dataset.maxSize).toBe("60");
    expect(resizeMock).toHaveBeenCalledWith(22);
  });

  it("removes the resize spacer when bottom content is merged with the main surface", () => {
    render(
      <StandardTabWrapper
        afterBorder={<div data-testid="bottom-area" />}
        afterBorderExpanded
        afterBorderResizable
        bottomBorderHandle={<button>Transcript</button>}
        mergeAfterBorder
      >
        <div data-testid="main-area" />
      </StandardTabWrapper>,
    );

    expect(screen.getByTestId("resize-handle").dataset.className).toContain(
      "data-[panel-group-direction=vertical]:h-0",
    );

    const panels = screen.getAllByTestId("panel");
    expect(panels[1]?.dataset.className).toContain("min-h-[154px]");
  });

  it("sizes collapsed bottom content to its row instead of reserving split space", () => {
    render(
      <StandardTabWrapper
        afterBorder={<div data-testid="bottom-area" />}
        afterBorderResizable
        bottomBorderHandle={<button>Live</button>}
      >
        <div data-testid="main-area" />
      </StandardTabWrapper>,
    );

    expect(screen.getByTestId("panel-group").dataset.direction).toBe(
      "vertical",
    );
    expect(screen.queryByTestId("resize-handle")).toBeNull();
    expect(screen.getAllByTestId("panel")).toHaveLength(1);
    expect(screen.getByTestId("panel").dataset.defaultSize).toBe("100");
    expect(screen.getByTestId("bottom-area")).toBeTruthy();
    expect(resizeMock).not.toHaveBeenCalled();
  });

  it("keeps main content mounted when expandable bottom content toggles", () => {
    const mountMock = vi.fn();

    class MainArea extends React.Component {
      componentDidMount() {
        mountMock();
      }

      render() {
        return <div data-testid="main-area" />;
      }
    }

    const { rerender } = render(
      <StandardTabWrapper
        afterBorder={<div data-testid="bottom-area" />}
        afterBorderResizable
        bottomBorderHandle={<button>Live</button>}
      >
        <MainArea />
      </StandardTabWrapper>,
    );

    rerender(
      <StandardTabWrapper
        afterBorder={<div data-testid="bottom-area" />}
        afterBorderExpanded
        afterBorderResizable
        bottomBorderHandle={<button>Live</button>}
      >
        <MainArea />
      </StandardTabWrapper>,
    );

    expect(mountMock).toHaveBeenCalledTimes(1);
  });
});
