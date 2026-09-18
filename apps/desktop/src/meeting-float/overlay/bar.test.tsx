import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { FloatingBarState } from "@anlg/plugin-windows";

import { FloatingBarOverlay } from "./bar";

vi.mock("@anlg/ui/components/ui/dancing-sticks", () => ({
  DancingSticks: ({ amplitude }: { amplitude: number }) => (
    <span data-testid="waveform" data-amplitude={amplitude} />
  ),
}));

function state(overrides: Partial<FloatingBarState> = {}): FloatingBarState {
  return {
    dictation: null,
    amplitude: 0.4,
    title: "Weekly sync",
    status: "recording",
    colorScheme: "light",
    opacity: 0.78,
    liveCaptionOpacity: 0.3,
    liveCaptionWidth: 440,
    liveCaptionLineCount: 1,
    liveCaptionPosition: "topCenter",
    liveCaptionMinimized: true,
    liveCaptionToggleVisible: true,
    transcriptBubbles: [
      {
        id: "1",
        speakerLabel: "Ada",
        text: "Let's start.",
        isSelf: false,
        isFinal: true,
        startMs: 0,
        endMs: 1200,
        overlapsPrevious: false,
        overlapsNext: false,
      },
    ],
    ...overrides,
  };
}

describe("FloatingBarOverlay", () => {
  afterEach(() => {
    cleanup();
  });

  it.each([true, false])(
    "keeps legacy backend controls at the top right (minimized=%s)",
    (liveCaptionMinimized) => {
      render(
        <FloatingBarOverlay
          state={state({ liveCaptionMinimized })}
          onStop={vi.fn()}
          onToggleExpanded={vi.fn()}
        />,
      );
      const controls = screen.getByRole("button", { name: "Stop listening" })
        .parentElement!.parentElement!;
      expect(controls.style.top).toBe("0px");
      expect(controls.style.bottom).toBe("");
      expect(controls.style.left).toBe("calc(100% - 50px)");
    },
  );

  it("uses backend coordinates when layout metadata is available", () => {
    render(
      <FloatingBarOverlay
        state={state({
          liveCaptionMinimized: false,
          layout: { controlsCenterX: 200, expandsUpward: true },
        })}
        onStop={vi.fn()}
        onToggleExpanded={vi.fn()}
      />,
    );
    const controls = screen.getByRole("button", { name: "Stop listening" })
      .parentElement!.parentElement!;
    expect(controls.style.top).toBe("");
    expect(controls.style.bottom).toBe("0px");
    expect(controls.style.left).toBe("196px");
  });

  it("stops listening from the compact bar", () => {
    const onStop = vi.fn();

    render(
      <FloatingBarOverlay
        state={state()}
        onStop={onStop}
        onToggleExpanded={vi.fn()}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Stop listening" }));

    expect(onStop).toHaveBeenCalledOnce();
    expect(screen.getByTestId("waveform")).toBeTruthy();
  });

  it("shows a spinner when live transcription is reconnecting", () => {
    const { container } = render(
      <FloatingBarOverlay
        state={state({ status: "reconnecting" })}
        onStop={vi.fn()}
        onToggleExpanded={vi.fn()}
      />,
    );

    expect(
      screen.getByRole("button", {
        name: "Reconnecting live transcription; stop listening",
      }),
    ).toBeTruthy();
    expect(screen.queryByTestId("waveform")).toBeNull();
    expect(container.querySelector(".animate-spin")).toBeTruthy();
  });

  it("shows a static failure indicator for errors without an active retry", () => {
    const { container } = render(
      <FloatingBarOverlay
        state={state({ status: "error" })}
        onStop={vi.fn()}
        onToggleExpanded={vi.fn()}
      />,
    );
    expect(
      screen.getByRole("button", {
        name: "Transcription unavailable; stop listening",
      }),
    ).toBeTruthy();
    expect(container.querySelector(".animate-spin")).toBeNull();
  });

  it("expands to the live transcript and can collapse again", () => {
    const onToggleExpanded = vi.fn();

    const view = render(
      <FloatingBarOverlay
        state={state()}
        onStop={vi.fn()}
        onToggleExpanded={onToggleExpanded}
      />,
    );

    const waveform = screen.getByTestId("waveform");
    const toggle = screen.getByRole("button", {
      name: "Expand live transcript",
    });
    fireEvent.click(toggle);
    expect(onToggleExpanded).toHaveBeenCalledWith(true);

    view.rerender(
      <FloatingBarOverlay
        state={state({ liveCaptionMinimized: false })}
        onStop={vi.fn()}
        onToggleExpanded={onToggleExpanded}
      />,
    );

    expect(screen.getByTestId("waveform")).toBe(waveform);
    expect(
      screen.getByRole("button", { name: "Collapse live transcript" }),
    ).toBe(toggle);
    expect(screen.queryByText("Weekly sync")).toBeNull();
    expect(
      view.container.querySelector('[data-tauri-drag-region="true"]'),
    ).not.toBeNull();
    fireEvent.mouseEnter(view.container.firstElementChild!);
    expect(
      view.container.querySelector('[data-tauri-drag-region="true"]'),
    ).not.toBeNull();
    expect(screen.getByText("Let's start.")).toBeTruthy();

    fireEvent.click(
      screen.getByRole("button", { name: "Collapse live transcript" }),
    );
    expect(onToggleExpanded).toHaveBeenCalledWith(false);
  });
});
