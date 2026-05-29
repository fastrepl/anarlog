import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  windowShow: vi.fn(() => Promise.resolve({ status: "ok" })),
}));

vi.mock("@hypr/plugin-windows", () => ({
  commands: {
    windowShow: mocks.windowShow,
  },
}));

import { ChatCTA } from "./chat-cta";

describe("ChatCTA", () => {
  it("opens the composer window", () => {
    render(<ChatCTA />);

    fireEvent.click(
      screen.getByRole("button", { name: "Ask Anarlog anything" }),
    );

    expect(mocks.windowShow).toHaveBeenCalledWith({ type: "composer" });
  });
});
