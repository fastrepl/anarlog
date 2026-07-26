import { render } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { StandaloneWindowShell } from "./window-shell";

describe("StandaloneWindowShell", () => {
  it("renders a top drag region by default", () => {
    const { container } = render(
      <StandaloneWindowShell>
        <div>Content</div>
      </StandaloneWindowShell>,
    );

    const dragRegion = container.querySelector(
      "[data-standalone-window-top-drag-region]",
    );

    expect(dragRegion?.hasAttribute("data-tauri-drag-region")).toBe(true);
  });

  it("can omit the top drag region when nested controls own hit testing", () => {
    const { container } = render(
      <StandaloneWindowShell topDragRegion={false}>
        <div>Content</div>
      </StandaloneWindowShell>,
    );

    expect(
      container.querySelector("[data-standalone-window-top-drag-region]"),
    ).toBeNull();
  });
});
