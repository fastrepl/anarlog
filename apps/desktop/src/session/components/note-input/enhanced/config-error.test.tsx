import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

const openNew = vi.hoisted(() => vi.fn());

vi.mock("~/store/zustand/tabs", () => ({
  useTabs: (selector: (state: { openNew: typeof openNew }) => unknown) =>
    selector({ openNew }),
}));

import { ConfigError } from "./config-error";

describe("ConfigError", () => {
  afterEach(() => {
    cleanup();
    openNew.mockReset();
  });

  it("offers Pro and API key setup from the empty summary state", () => {
    render(<ConfigError />);

    expect(screen.getByRole("alert")).not.toBeNull();
    expect(screen.getByText("Set up AI summaries")).not.toBeNull();
    expect(
      screen.getByText(
        "Start a Pro trial or add your own LLM API key to generate a summary from this transcript.",
      ),
    ).not.toBeNull();

    fireEvent.click(screen.getByRole("button", { name: "Get Pro" }));
    expect(openNew).toHaveBeenNthCalledWith(1, {
      type: "settings",
      state: { tab: "account" },
    });

    fireEvent.click(screen.getByRole("button", { name: "Add API key" }));
    expect(openNew).toHaveBeenNthCalledWith(2, {
      type: "settings",
      state: { tab: "intelligence" },
    });
  });
});
