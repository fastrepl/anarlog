import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { ShareRecapModeSelector } from "./delivery-panel";

describe("ShareRecapModeSelector", () => {
  it("offers invitation, email, and Slack delivery", () => {
    const onValueChange = vi.fn();
    render(
      <ShareRecapModeSelector value="invite" onValueChange={onValueChange} />,
    );

    expect(
      screen
        .getByRole("button", { name: "People" })
        .getAttribute("aria-pressed"),
    ).toBe("true");
    fireEvent.click(screen.getByRole("button", { name: "Email" }));
    fireEvent.click(screen.getByRole("button", { name: "Slack" }));

    expect(onValueChange).toHaveBeenNthCalledWith(1, "email");
    expect(onValueChange).toHaveBeenNthCalledWith(2, "slack");
  });
});
