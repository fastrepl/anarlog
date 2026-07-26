import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { InteractiveButton } from "./interactive-button";

vi.mock("~/shared/hooks/useNativeContextMenu", () => ({
  useNativeContextMenu: () => vi.fn(),
}));

describe("InteractiveButton", () => {
  afterEach(() => {
    cleanup();
  });

  it("runs the first click immediately when a double-click action is available", () => {
    const onClick = vi.fn();
    const onDoubleClick = vi.fn();

    render(
      <InteractiveButton onClick={onClick} onDoubleClick={onDoubleClick}>
        Open note
      </InteractiveButton>,
    );

    fireEvent.click(screen.getByRole("button", { name: "Open note" }), {
      detail: 1,
    });

    expect(onClick).toHaveBeenCalledTimes(1);
    expect(onDoubleClick).not.toHaveBeenCalled();
  });

  it("runs the double-click action without repeating the click action", () => {
    const onClick = vi.fn();
    const onDoubleClick = vi.fn();

    render(
      <InteractiveButton onClick={onClick} onDoubleClick={onDoubleClick}>
        Open note
      </InteractiveButton>,
    );

    const button = screen.getByRole("button", { name: "Open note" });
    fireEvent.click(button, { detail: 1 });
    fireEvent.click(button, { detail: 2 });
    fireEvent.doubleClick(button);

    expect(onClick).toHaveBeenCalledTimes(1);
    expect(onDoubleClick).toHaveBeenCalledTimes(1);
  });
});
