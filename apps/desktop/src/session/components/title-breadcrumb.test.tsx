import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { NoteTitleBreadcrumb } from "./title-breadcrumb";

const mocks = vi.hoisted(() => ({
  folderId: "",
}));

vi.mock("~/session/queries", () => ({
  useFolderPaths: () => [],
  useSession: () => ({ folder_id: mocks.folderId }),
  useUpdateSession: () => vi.fn(() => Promise.resolve()),
}));

describe("NoteTitleBreadcrumb", () => {
  beforeEach(() => {
    mocks.folderId = "";
  });

  afterEach(() => {
    cleanup();
  });

  it("renders the folder picker and editable title when no folder is set", () => {
    render(
      <NoteTitleBreadcrumb
        sessionId="session-1"
        title={<input aria-label="Session title" />}
      />,
    );

    const breadcrumb = screen.getByRole("navigation", {
      name: "Note breadcrumb",
    });
    const title = screen.getByLabelText("Session title");

    expect(
      screen.getByRole("combobox", { name: "Select folder" }),
    ).not.toBeNull();
    expect(breadcrumb.contains(title)).toBe(true);
    expect(breadcrumb.getAttribute("data-tauri-drag-region")).toBe("false");
    expect(screen.queryByText("/")).toBeNull();
  });

  it("renders the selected folder before the editable title", () => {
    mocks.folderId = "work";

    render(
      <NoteTitleBreadcrumb
        sessionId="session-1"
        title={<input aria-label="Session title" />}
      />,
    );

    expect(
      screen.getByRole("combobox", { name: "Folder: work" }),
    ).not.toBeNull();
    expect(screen.getByText("/")).not.toBeNull();
    expect(screen.getByLabelText("Session title")).not.toBeNull();
  });
});
