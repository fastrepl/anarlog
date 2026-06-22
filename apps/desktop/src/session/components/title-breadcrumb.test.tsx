import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { NoteTitleBreadcrumb } from "./title-breadcrumb";

const mocks = vi.hoisted(() => ({
  folderId: "",
}));

vi.mock("~/store/tinybase/store/main", () => ({
  STORE_ID: "main",
  UI: {
    useCell: () => mocks.folderId,
  },
}));

describe("NoteTitleBreadcrumb", () => {
  beforeEach(() => {
    mocks.folderId = "";
  });

  afterEach(() => {
    cleanup();
  });

  it("renders the root folder crumb before the editable title", () => {
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

    expect(screen.getByText("Select folder")).not.toBeNull();
    expect(breadcrumb.contains(title)).toBe(true);
    expect(breadcrumb.getAttribute("data-tauri-drag-region")).toBe("false");
  });

  it("renders persisted folder path crumbs before the editable title", () => {
    mocks.folderId = "work/meetings";

    render(
      <NoteTitleBreadcrumb
        sessionId="session-1"
        title={<input aria-label="Session title" />}
      />,
    );

    expect(screen.getByText("work")).not.toBeNull();
    expect(screen.getByText("meetings")).not.toBeNull();
    expect(screen.getByLabelText("Session title")).not.toBeNull();
  });
});
