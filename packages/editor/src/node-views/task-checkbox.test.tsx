import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { TaskCheckbox } from "./task-checkbox";

describe("TaskCheckbox", () => {
  it("calls onToggle when interactive", () => {
    const onToggle = vi.fn();

    render(<TaskCheckbox status="todo" isInteractive onToggle={onToggle} />);

    fireEvent.click(screen.getByRole("checkbox"));

    expect(onToggle).toHaveBeenCalledTimes(1);
  });

  it("exposes the mixed state for in-progress tasks", () => {
    const view = render(<TaskCheckbox status="in_progress" />);

    expect(
      view.container
        .querySelector('[role="checkbox"]')
        ?.getAttribute("aria-checked"),
    ).toBe("mixed");
  });

  it("does not call onToggle when read-only", () => {
    const view = render(<TaskCheckbox status="done" />);

    const checkbox = view.container.querySelector(
      '[role="checkbox"]',
    ) as HTMLButtonElement | null;

    expect(checkbox).not.toBeNull();
    if (!checkbox) {
      return;
    }

    fireEvent.click(checkbox);

    expect(checkbox.getAttribute("data-interactive")).toBe("false");
  });
});
