import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  createNamedFolder: vi.fn(),
  folderPaths: [] as string[],
  onValueChange: vi.fn(),
}));

vi.mock("~/session/folder-catalog", () => ({
  createNamedFolder: mocks.createNamedFolder,
}));

vi.mock("~/session/queries", () => ({
  useFolderPaths: () => mocks.folderPaths,
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

import { SidebarNoteFilterMenu } from "./note-filter-menu";

describe("SidebarNoteFilterMenu", () => {
  afterEach(() => {
    cleanup();
    mocks.folderPaths = [];
    vi.clearAllMocks();
  });

  it("offers personal and received sharing views", () => {
    render(
      <SidebarNoteFilterMenu
        value="mine"
        onValueChange={mocks.onValueChange}
      />,
    );

    const trigger = screen.getByRole("button", { name: "Filter notes" });
    fireEvent.pointerDown(trigger);
    fireEvent.click(trigger);

    expect(
      screen.getByRole("menuitemradio", { name: "My notes" }),
    ).toBeTruthy();
    expect(screen.getByRole("menuitemradio", { name: "Shared" })).toBeTruthy();

    fireEvent.click(screen.getByRole("menuitemradio", { name: "Shared" }));
    expect(mocks.onValueChange).toHaveBeenCalledWith("shared", null);
  });

  it("lists folders and keeps the timeline on a selected folder", () => {
    mocks.folderPaths = ["CS 101", "work"];

    render(
      <SidebarNoteFilterMenu
        folderFilter="CS 101"
        value="mine"
        onValueChange={mocks.onValueChange}
      />,
    );

    const trigger = screen.getByRole("button", { name: "Filter notes" });
    fireEvent.pointerDown(trigger);
    fireEvent.click(trigger);

    expect(
      screen.getByRole("menuitemradio", { name: "No folder" }),
    ).toBeTruthy();
    expect(screen.getByRole("menuitemradio", { name: "CS 101" })).toBeTruthy();
    expect(screen.getByRole("menuitemradio", { name: "work" })).toBeTruthy();

    fireEvent.click(screen.getByRole("menuitemradio", { name: "work" }));
    expect(mocks.onValueChange).toHaveBeenCalledWith("mine", "work");
  });

  it("lets the user create a folder before any notes exist", async () => {
    mocks.createNamedFolder.mockResolvedValue("CS 101");

    render(
      <SidebarNoteFilterMenu
        value="mine"
        onValueChange={mocks.onValueChange}
      />,
    );

    const trigger = screen.getByRole("button", { name: "Filter notes" });
    fireEvent.pointerDown(trigger);
    fireEvent.click(trigger);

    expect(
      screen.getByRole("menuitemradio", { name: "No folder" }),
    ).toBeTruthy();
    fireEvent.click(screen.getByRole("menuitem", { name: "New folder" }));

    fireEvent.change(screen.getByLabelText("Folder name"), {
      target: { value: "CS 101" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Create" }));

    await waitFor(() => {
      expect(mocks.createNamedFolder).toHaveBeenCalledWith("CS 101");
      expect(mocks.onValueChange).toHaveBeenCalledWith("mine", "CS 101");
    });
  });

  it("creates a subfolder under the selected folder", async () => {
    mocks.folderPaths = ["CS 101"];
    mocks.createNamedFolder.mockResolvedValue("CS 101/Week 1");

    render(
      <SidebarNoteFilterMenu
        folderFilter="CS 101"
        value="mine"
        onValueChange={mocks.onValueChange}
      />,
    );

    const trigger = screen.getByRole("button", { name: "Filter notes" });
    fireEvent.pointerDown(trigger);
    fireEvent.click(trigger);
    fireEvent.click(screen.getByRole("menuitem", { name: "New subfolder" }));

    fireEvent.change(screen.getByLabelText("Folder name"), {
      target: { value: "Week 1" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Create" }));

    await waitFor(() => {
      expect(mocks.createNamedFolder).toHaveBeenCalledWith("CS 101/Week 1");
      expect(mocks.onValueChange).toHaveBeenCalledWith("mine", "CS 101/Week 1");
    });
  });
});
