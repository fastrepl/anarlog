import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { TaskCheckbox } from "./task-checkbox";

describe("TaskCheckbox", () => {
  it("calls onToggle when interactive", () => {
    const onToggle = vi.fn();

    render(<TaskCheckbox checked={false} isInteractive onToggle={onToggle} />);

    fireEvent.click(screen.getByRole("checkbox"));

    expect(onToggle).toHaveBeenCalledTimes(1);
  });

  it("does not call onToggle when read-only", () => {
    const view = render(<TaskCheckbox checked />);

    const checkbox = view.container.querySelector(
      'input[type="checkbox"]',
    ) as HTMLInputElement | null;

    expect(checkbox).not.toBeNull();
    if (!checkbox) {
      return;
    }

    fireEvent.click(checkbox);

    expect(checkbox.getAttribute("data-interactive")).toBe("false");
  });
});
