import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

const openNew = vi.hoisted(() => vi.fn());

vi.mock("~/store/zustand/tabs", () => ({
  useTabs: (selector: (state: { openNew: typeof openNew }) => unknown) =>
    selector({ openNew }),
}));

import { GoogleCalendarAiBoundaryError } from "./google-calendar-ai-boundary-error";

describe("GoogleCalendarAiBoundaryError", () => {
  afterEach(() => {
    cleanup();
    openNew.mockReset();
  });

  it("offers on-device AI when Google data blocks remote models", () => {
    render(<GoogleCalendarAiBoundaryError checkFailed={false} />);

    expect(
      screen.getByText("Use on-device AI for Google Calendar notes"),
    ).not.toBeNull();

    fireEvent.click(
      screen.getByRole("button", { name: "Choose on-device AI" }),
    );
    expect(openNew).toHaveBeenCalledWith({
      type: "settings",
      state: { tab: "intelligence" },
    });
  });

  it("fails closed when the local data check fails", () => {
    render(<GoogleCalendarAiBoundaryError checkFailed />);

    expect(screen.getByText("AI is temporarily unavailable")).not.toBeNull();
    expect(screen.queryByRole("button")).toBeNull();
  });
});
