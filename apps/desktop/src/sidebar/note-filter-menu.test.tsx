import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  onValueChange: vi.fn(),
  workspaces: [
    { id: "workspace-1", name: "Fastrepl" },
    { id: "workspace-2", name: "Design partners" },
  ],
}));

vi.mock("@lingui/react/macro", () => ({
  Trans: ({ children }: { children?: ReactNode }) => <>{children}</>,
  useLingui: () => ({
    t: (strings: TemplateStringsArray, ...values: unknown[]) =>
      strings.reduce(
        (message, part, index) =>
          `${message}${part}${index < values.length ? String(values[index]) : ""}`,
        "",
      ),
  }),
}));

vi.mock("~/auth", () => ({
  useAuth: () => ({ session: { user: { id: "user-1" } } }),
}));

vi.mock("~/session-sharing/source", () => ({
  useAvailableShareWorkspaces: () => mocks.workspaces,
}));

import { SidebarNoteFilterMenu } from "./note-filter-menu";

describe("SidebarNoteFilterMenu", () => {
  afterEach(() => {
    cleanup();
    vi.clearAllMocks();
  });

  it("offers personal, sharing, and workspace views", () => {
    render(
      <SidebarNoteFilterMenu value="all" onValueChange={mocks.onValueChange} />,
    );

    const trigger = screen.getByRole("button", { name: "Filter notes" });
    fireEvent.pointerDown(trigger);
    fireEvent.click(trigger);

    expect(
      screen.getByRole("menuitemradio", { name: "All notes" }),
    ).toBeTruthy();
    expect(
      screen.getByRole("menuitemradio", { name: "My notes" }),
    ).toBeTruthy();
    expect(
      screen.getByRole("menuitemradio", { name: "Shared by me" }),
    ).toBeTruthy();
    expect(
      screen.getByRole("menuitemradio", { name: "Shared with me" }),
    ).toBeTruthy();
    expect(
      screen.getByRole("menuitemradio", { name: "Fastrepl" }),
    ).toBeTruthy();

    fireEvent.click(
      screen.getByRole("menuitemradio", { name: "Design partners" }),
    );
    expect(mocks.onValueChange).toHaveBeenCalledWith("workspace:workspace-2");
  });
});
