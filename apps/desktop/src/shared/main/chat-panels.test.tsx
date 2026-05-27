import { cleanup, render, waitFor } from "@testing-library/react";
import * as React from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const { getSizeMock, resizeMock, windowExpandWidthMock } = vi.hoisted(() => ({
  getSizeMock: vi.fn(() => 100),
  resizeMock: vi.fn(),
  windowExpandWidthMock: vi.fn(() => Promise.resolve()),
}));

vi.mock("@hypr/plugin-windows", () => ({
  commands: {
    windowExpandWidth: windowExpandWidthMock,
    windowRestoreWidth: vi.fn(() => Promise.resolve()),
  },
}));

vi.mock("~/chat/components/persistent-chat", () => ({
  PersistentChatPanel: () => null,
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
      { getSize: () => number; resize: (size: number) => void },
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
        getSize: getSizeMock,
        resize: resizeMock,
      }));

      return (
        <div
          data-class-name={className}
          data-default-size={defaultSize}
          data-max-size={maxSize}
          data-min-size={minSize}
          data-testid="panel"
        >
          {children}
        </div>
      );
    }),
    ResizableHandle: ({ className }: { className?: string }) => (
      <div data-class-name={className} data-testid="resize-handle" />
    ),
  };
});

import { MainChatPanels } from "./chat-panels";

describe("MainChatPanels", () => {
  beforeEach(() => {
    cleanup();
    getSizeMock.mockClear();
    resizeMock.mockClear();
    windowExpandWidthMock.mockClear();
  });

  it("only asks the native window to expand while below the chat replacement width", async () => {
    const { rerender } = render(
      <MainChatPanels autoSaveId="test-chat" isRightPanelOpen={false}>
        <div data-testid="main-content" />
      </MainChatPanels>,
    );

    rerender(
      <MainChatPanels autoSaveId="test-chat" isRightPanelOpen>
        <div data-testid="main-content" />
      </MainChatPanels>,
    );

    await waitFor(() => {
      expect(windowExpandWidthMock).toHaveBeenCalledWith(400, 720, true, false);
    });
    expect(resizeMock).toHaveBeenCalledWith(100);
  });
});
