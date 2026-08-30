import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

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

import { resetSidebarNotes, useSidebarNotes } from "./note-filter";
import { SidebarNoteFilterMenu } from "./note-filter-menu";

function openSortMenu() {
  const trigger = screen.getByRole("button", { name: "Sort notes" });
  fireEvent.pointerDown(trigger);
  fireEvent.click(trigger);
}

function openSubmenu(name: string) {
  const trigger = screen.getByRole("menuitem", { name });
  fireEvent.focus(trigger);
  fireEvent.keyDown(trigger, { key: "ArrowRight" });
}

describe("SidebarNoteFilterMenu", () => {
  beforeEach(() => {
    resetSidebarNotes();
  });

  afterEach(() => {
    cleanup();
    resetSidebarNotes();
  });

  it("does not offer ownership or folder filters", () => {
    render(<SidebarNoteFilterMenu />);
    openSortMenu();

    expect(
      screen.queryByRole("menuitemradio", { name: "My notes" }),
    ).toBeNull();
    expect(screen.queryByRole("menuitemradio", { name: "Shared" })).toBeNull();
    expect(
      screen.queryByRole("menuitemradio", { name: "No folder" }),
    ).toBeNull();
    expect(screen.queryByRole("menuitem", { name: "New folder" })).toBeNull();
  });

  it("switches the timeline to a folder-grouped view", () => {
    render(<SidebarNoteFilterMenu />);

    openSortMenu();
    openSubmenu("Grouping, Date");
    fireEvent.click(screen.getByRole("menuitem", { name: "Folder" }));

    expect(useSidebarNotes.getState().groupBy).toBe("folder");
  });

  it("orders notes from oldest to newest", () => {
    render(<SidebarNoteFilterMenu />);

    openSortMenu();
    openSubmenu("Ordering, Newest");
    fireEvent.click(screen.getByRole("menuitem", { name: "Oldest" }));

    expect(useSidebarNotes.getState().sortOrder).toBe("oldest");
  });
});
