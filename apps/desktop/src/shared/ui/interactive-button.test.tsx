import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { InteractiveButton } from "./interactive-button";

vi.mock("~/shared/hooks/useNativeContextMenu", () => ({
  useNativeContextMenu: () => vi.fn(),
}));

describe("InteractiveButton", () => {
  afterEach(() => {
    cleanup();
    vi.useRealTimers();
  });

  it("clears a pending single-click callback when unmounted", () => {
    vi.useFakeTimers();
    const onClick = vi.fn();
    const onDoubleClick = vi.fn();

    const { unmount } = render(
      <InteractiveButton onClick={onClick} onDoubleClick={onDoubleClick}>
        Open note
      </InteractiveButton>,
    );

    fireEvent.click(screen.getByRole("button", { name: "Open note" }));
    unmount();
    vi.advanceTimersByTime(350);

    expect(onClick).not.toHaveBeenCalled();
    expect(onDoubleClick).not.toHaveBeenCalled();
  });
});
