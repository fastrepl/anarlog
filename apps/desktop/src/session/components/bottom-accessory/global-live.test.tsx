import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import * as React from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  live: {
    status: "active" as "inactive" | "active" | "finalizing",
    sessionId: "live-session" as string | null,
    requestedLiveTranscription: true as boolean | null,
    liveTranscriptionActive: true as boolean | null,
  },
  resize: vi.fn(),
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
        resize: mocks.resize,
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

vi.mock("./during-session", () => ({
  DuringSessionAccessory: ({
    fillHeight,
    isExpanded,
    sessionId,
  }: {
    fillHeight?: boolean;
    isExpanded?: boolean;
    sessionId: string;
  }) => (
    <div
      data-fill-height={String(fillHeight)}
      data-is-expanded={String(isExpanded)}
      data-session-id={sessionId}
      data-testid="during-session-accessory"
    />
  ),
}));

vi.mock("~/stt/contexts", () => ({
  useListener: (
    selector: (state: {
      live: {
        status: "inactive" | "active" | "finalizing";
        sessionId: string | null;
        requestedLiveTranscription: boolean | null;
        liveTranscriptionActive: boolean | null;
      };
    }) => unknown,
  ) =>
    selector({
      live: mocks.live,
    }),
}));

import { GlobalLiveTranscriptAccessory } from "./global-live";

describe("GlobalLiveTranscriptAccessory", () => {
  beforeEach(() => {
    cleanup();
    mocks.live.status = "active";
    mocks.live.sessionId = "live-session";
    mocks.live.requestedLiveTranscription = true;
    mocks.live.liveTranscriptionActive = true;
    mocks.resize.mockClear();
  });

  it("does not duplicate the live transcript on the active session tab", () => {
    render(
      <GlobalLiveTranscriptAccessory
        currentTab={{ type: "sessions", id: "live-session" } as any}
      >
        <div data-testid="tab-content" />
      </GlobalLiveTranscriptAccessory>,
    );

    expect(screen.getByTestId("tab-content")).toBeTruthy();
    expect(screen.queryByTestId("during-session-accessory")).toBeNull();
    expect(screen.queryByRole("button", { name: "Expand Live" })).toBeNull();
  });

  it("keeps tab content mounted when the global live transcript appears", () => {
    mocks.live.sessionId = null;
    const mountMock = vi.fn();

    class TabContent extends React.Component {
      componentDidMount() {
        mountMock();
      }

      render() {
        return <div data-testid="tab-content" />;
      }
    }

    const view = render(
      <GlobalLiveTranscriptAccessory currentTab={{ type: "settings" } as any}>
        <TabContent />
      </GlobalLiveTranscriptAccessory>,
    );

    mocks.live.sessionId = "live-session";
    view.rerender(
      <GlobalLiveTranscriptAccessory currentTab={{ type: "settings" } as any}>
        <TabContent />
      </GlobalLiveTranscriptAccessory>,
    );

    expect(screen.getByTestId("tab-content")).toBeTruthy();
    expect(screen.getByTestId("during-session-accessory")).toBeTruthy();
    expect(mountMock).toHaveBeenCalledTimes(1);
  });

  it("shows and expands the live transcript for other tabs", () => {
    render(
      <GlobalLiveTranscriptAccessory
        currentTab={{ type: "sessions", id: "other-session" } as any}
      >
        <div data-testid="tab-content" />
      </GlobalLiveTranscriptAccessory>,
    );

    expect(
      screen.getByTestId("during-session-accessory").dataset,
    ).toMatchObject({
      sessionId: "live-session",
      isExpanded: "false",
      fillHeight: "false",
    });

    fireEvent.click(screen.getByRole("button", { name: "Expand Live" }));

    expect(
      screen.getByTestId("during-session-accessory").dataset,
    ).toMatchObject({
      sessionId: "live-session",
      isExpanded: "true",
      fillHeight: "true",
    });
    expect(screen.getByTestId("resize-handle")).toBeTruthy();
    expect(mocks.resize).toHaveBeenCalledWith(22);
  });

  it("resets expanded state when the live session changes", () => {
    const view = render(
      <GlobalLiveTranscriptAccessory
        currentTab={{ type: "sessions", id: "other-session" } as any}
      >
        <div data-testid="tab-content" />
      </GlobalLiveTranscriptAccessory>,
    );

    fireEvent.click(screen.getByRole("button", { name: "Expand Live" }));

    expect(
      screen.getByTestId("during-session-accessory").dataset,
    ).toMatchObject({
      sessionId: "live-session",
      isExpanded: "true",
      fillHeight: "true",
    });

    mocks.live.sessionId = "next-live-session";
    view.rerender(
      <GlobalLiveTranscriptAccessory
        currentTab={{ type: "sessions", id: "other-session" } as any}
      >
        <div data-testid="tab-content" />
      </GlobalLiveTranscriptAccessory>,
    );

    expect(
      screen.getByTestId("during-session-accessory").dataset,
    ).toMatchObject({
      sessionId: "next-live-session",
      isExpanded: "false",
      fillHeight: "false",
    });
    expect(screen.queryByTestId("resize-handle")).toBeNull();
  });

  it("keeps the live panel flush against the bottom divider", () => {
    render(
      <GlobalLiveTranscriptAccessory currentTab={{ type: "settings" } as any}>
        <div data-testid="tab-content" />
      </GlobalLiveTranscriptAccessory>,
    );

    const panel = screen.getByTestId("during-session-accessory");
    const afterBorderContent = panel.parentElement;

    expect(afterBorderContent?.className).not.toContain("pt-1.5");
    expect(afterBorderContent?.className).not.toContain("mt-1");
  });

  it("uses the active live footer frame outside the active session tab", () => {
    render(
      <GlobalLiveTranscriptAccessory currentTab={{ type: "settings" } as any}>
        <div data-testid="tab-content" />
      </GlobalLiveTranscriptAccessory>,
    );

    const transcriptCard = screen.getByTestId(
      "during-session-accessory",
    ).parentElement;

    expect(
      transcriptCard?.hasAttribute("data-global-live-transcript-card"),
    ).toBe(true);
    expect(transcriptCard?.className).not.toContain("border-x");
    expect(transcriptCard?.className).not.toContain("border-b");
    expect(transcriptCard?.className).not.toContain("rounded-b-xl");
  });

  it("keeps the current surface divider in top chrome mode", () => {
    render(
      <GlobalLiveTranscriptAccessory
        currentTab={{ type: "sessions", id: "other-session" } as any}
        surfaceChrome="top"
      >
        <div data-chat-floating-anchor data-testid="tab-content" />
      </GlobalLiveTranscriptAccessory>,
    );

    const shell = document.querySelector("[data-global-live-transcript-shell]");

    expect(shell?.className).toContain(
      "[&_[data-chat-floating-anchor]]:!border-b",
    );
    expect(shell?.className).not.toContain("!rounded-bl-none");
  });

  it("keeps the current surface divider and left edge in sidebar timeline mode", () => {
    render(
      <GlobalLiveTranscriptAccessory
        currentTab={{ type: "sessions", id: "other-session" } as any}
        surfaceChrome="left"
      >
        <div data-chat-floating-anchor data-testid="tab-content" />
      </GlobalLiveTranscriptAccessory>,
    );

    const shell = document.querySelector("[data-global-live-transcript-shell]");

    expect(shell?.className).toContain(
      "[&_[data-chat-floating-anchor]]:!border-b",
    );
    expect(shell?.className).toContain(
      "[&_[data-chat-floating-anchor]]:!rounded-bl-none",
    );
  });

  it("does not show a global live transcript while recording without live transcription", () => {
    mocks.live.requestedLiveTranscription = false;
    mocks.live.liveTranscriptionActive = false;

    render(
      <GlobalLiveTranscriptAccessory currentTab={{ type: "settings" } as any}>
        <div data-testid="tab-content" />
      </GlobalLiveTranscriptAccessory>,
    );

    expect(screen.getByTestId("tab-content")).toBeTruthy();
    expect(screen.queryByTestId("during-session-accessory")).toBeNull();
  });

  it("hides the global live transcript while finalizing", () => {
    mocks.live.status = "finalizing";
    mocks.live.requestedLiveTranscription = false;
    mocks.live.liveTranscriptionActive = false;

    render(
      <GlobalLiveTranscriptAccessory currentTab={{ type: "settings" } as any}>
        <div data-testid="tab-content" />
      </GlobalLiveTranscriptAccessory>,
    );

    expect(screen.getByTestId("tab-content")).toBeTruthy();
    expect(screen.queryByTestId("during-session-accessory")).toBeNull();
    expect(screen.queryByRole("button", { name: "Expand Live" })).toBeNull();
  });
});
