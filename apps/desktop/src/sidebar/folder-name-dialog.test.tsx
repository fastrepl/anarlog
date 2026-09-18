import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { FolderNameDialog } from "./folder-name-dialog";

afterEach(cleanup);

describe("FolderNameDialog", () => {
  it.each([
    ["New folder", "Create", ""],
    ["Rename folder", "Rename", "Old name"],
  ])(
    "uses %s's confirmation as the implicit submit button",
    async (title, confirmLabel, initialValue) => {
      const onSubmit = vi.fn().mockResolvedValue(undefined);
      const onOpenChange = vi.fn();
      render(
        <FolderNameDialog
          open
          title={title}
          confirmLabel={confirmLabel}
          initialValue={initialValue}
          onSubmit={onSubmit}
          onOpenChange={onOpenChange}
        />,
      );

      const input = screen.getByRole("textbox", { name: "Folder name" });
      fireEvent.change(input, { target: { value: " Project notes " } });
      const form = input.closest("form")!;
      // Enter activates the first submit button; jsdom does not implement implicit submission.
      const defaultSubmit = Array.from(form.elements).find(
        (element) =>
          element instanceof HTMLButtonElement && element.type === "submit",
      );
      expect(defaultSubmit).toBe(
        screen.getByRole("button", { name: confirmLabel }),
      );
      fireEvent.click(defaultSubmit!);

      await waitFor(() => expect(onOpenChange).toHaveBeenCalledWith(false));
      expect(onSubmit).toHaveBeenCalledExactlyOnceWith("Project notes");
    },
  );

  it("cancels without submitting the folder name", () => {
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    const onOpenChange = vi.fn();
    render(
      <FolderNameDialog
        open
        title="New folder"
        confirmLabel="Create"
        initialValue="Project notes"
        onSubmit={onSubmit}
        onOpenChange={onOpenChange}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));

    expect(onOpenChange).toHaveBeenCalledExactlyOnceWith(false);
    expect(onSubmit).not.toHaveBeenCalled();
  });
});
