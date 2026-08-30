import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  showFolder: true,
  showTags: false,
  setShowFolder: vi.fn(),
  setShowTags: vi.fn(),
}));

vi.mock("~/settings/queries", () => ({
  useSetSettingValue: (key: string) =>
    key === "sidebar_show_folder" ? mocks.setShowFolder : mocks.setShowTags,
}));

vi.mock("~/shared/config", () => ({
  useConfigValue: (key: string) =>
    key === "sidebar_show_folder" ? mocks.showFolder : mocks.showTags,
}));

import { SidebarItemFieldsSettings } from "./sidebar-item-fields";

describe("SidebarItemFieldsSettings", () => {
  afterEach(() => {
    cleanup();
    vi.clearAllMocks();
    mocks.showFolder = true;
    mocks.showTags = false;
  });

  it("lets users toggle folder and tags on sidebar notes", () => {
    render(<SidebarItemFieldsSettings />);

    expect(
      screen
        .getByRole("switch", { name: "Folder" })
        .getAttribute("aria-checked"),
    ).toBe("true");
    expect(
      screen.getByRole("switch", { name: "Tags" }).getAttribute("aria-checked"),
    ).toBe("false");

    fireEvent.click(screen.getByRole("switch", { name: "Tags" }));

    expect(mocks.setShowTags).toHaveBeenCalledWith(true);
  });
});
