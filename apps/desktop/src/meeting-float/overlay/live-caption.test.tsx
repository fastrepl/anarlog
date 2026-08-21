import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { LiveCaptionState } from "@anlg/plugin-windows";

import { LiveCaptionOverlay } from "./live-caption";

function state(overrides: Partial<LiveCaptionState> = {}): LiveCaptionState {
  return {
    text: "we should ship this",
    opacity: 0.3,
    width: 440,
    lineCount: 1,
    position: "topCenter",
    minimized: false,
    ...overrides,
  };
}

describe("LiveCaptionOverlay", () => {
  afterEach(() => {
    cleanup();
  });

  it("renders rolling caption text", () => {
    render(
      <LiveCaptionOverlay
        state={state()}
        onOpacityChange={vi.fn()}
        onHide={vi.fn()}
      />,
    );

    expect(screen.getByText("we should ship this")).toBeTruthy();
  });

  it("emits opacity and hide from the footer", () => {
    const onOpacityChange = vi.fn();
    const onHide = vi.fn();

    render(
      <LiveCaptionOverlay
        state={state()}
        onOpacityChange={onOpacityChange}
        onHide={onHide}
      />,
    );

    fireEvent.change(screen.getByLabelText("Transcript opacity"), {
      target: { value: "0.66" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Hide transcript" }));

    expect(onOpacityChange).toHaveBeenCalledWith(0.66);
    expect(onHide).toHaveBeenCalledOnce();
  });
});
