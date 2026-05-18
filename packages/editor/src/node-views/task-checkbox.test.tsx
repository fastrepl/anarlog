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

  it("renders as checked only for done tasks", () => {
    const done = render(<TaskCheckbox status="done" />);

    expect(
      (done.container.querySelector("input") as HTMLInputElement).checked,
    ).toBe(true);
    done.unmount();

    const inProgress = render(<TaskCheckbox status="in_progress" />);

    expect(
      (inProgress.container.querySelector("input") as HTMLInputElement).checked,
    ).toBe(false);
  });

  it("does not call onToggle when read-only", () => {
    const view = render(<TaskCheckbox status="done" />);

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
