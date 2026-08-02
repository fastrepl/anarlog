import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  platform: vi.fn(() => "macos"),
  setAppIcon: vi.fn(),
  applyAppIconPreference: vi.fn(),
  appIcon: "default",
}));

vi.mock("@tauri-apps/plugin-os", () => ({
  platform: mocks.platform,
}));

vi.mock("~/settings/queries", () => ({
  useSetSettingValue: () => mocks.setAppIcon,
}));

vi.mock("~/shared/config", () => ({
  useConfigValue: () => mocks.appIcon,
}));

vi.mock("~/shared/theme/provider", () => ({
  applyAppIconPreference: mocks.applyAppIconPreference,
}));

import { AppIconSelector } from "./app-icon";

describe("AppIconSelector", () => {
  afterEach(() => {
    cleanup();
    vi.clearAllMocks();
    mocks.platform.mockReturnValue("macos");
    mocks.appIcon = "default";
  });

  it("applies and stores the selected icon", () => {
    render(<AppIconSelector />);

    const defaultOption = screen.getByRole("radio", { name: "Default" });
    expect(defaultOption.getAttribute("aria-checked")).toBe("true");
    expect(
      defaultOption
        .querySelector('source[media="(prefers-color-scheme: dark)"]')
        ?.getAttribute("srcset"),
    ).toBe("/assets/app-icons/default-dark.png");
    expect(defaultOption.querySelector("img")?.getAttribute("src")).toBe(
      "/assets/app-icons/default-light.png",
    );

    fireEvent.click(screen.getByRole("radio", { name: "Anagram" }));

    expect(mocks.applyAppIconPreference).toHaveBeenCalledWith("anagram");
    expect(mocks.setAppIcon).toHaveBeenCalledWith("anagram");
  });

  it("is hidden on platforms that cannot change the running app icon", () => {
    mocks.platform.mockReturnValue("windows");

    render(<AppIconSelector />);

    expect(screen.queryByText("App icon")).toBeNull();
  });
});
