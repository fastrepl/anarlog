import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { ExpandToggle } from "./expand-toggle";

describe("ExpandToggle", () => {
  afterEach(() => {
    cleanup();
  });

  it("masks only the divider segment under the handle", () => {
    render(<ExpandToggle isExpanded onToggle={vi.fn()} label="Transcript" />);

    expect(screen.getByRole("button").className).toContain("after:-bottom-px");
    expect(screen.getByRole("button").className).toContain("after:h-0.5");
    expect(screen.getByRole("button").className).toContain("after:bg-inherit");
  });
});
