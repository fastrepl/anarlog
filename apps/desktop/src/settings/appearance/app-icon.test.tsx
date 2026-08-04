import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  platform: vi.fn(() => "macos"),
  setAppIcon: vi.fn(),
  applyAppIconPreference: vi.fn(),
  appIcon: "default",
  appIdentifier: "com.hyprnote.stable",
}));

vi.mock("@tanstack/react-query", () => ({
  useQuery: () => ({ data: mocks.appIdentifier }),
}));

vi.mock("@tauri-apps/api/app", () => ({
  getIdentifier: vi.fn(),
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
    mocks.appIdentifier = "com.hyprnote.stable";
  });

  it("applies and stores the selected icon", () => {
    render(<AppIconSelector />);

    const defaultOption = screen.getByRole("radio", { name: "Default" });
    expect(defaultOption.getAttribute("aria-checked")).toBe("true");
    expect(
      defaultOption
        .querySelector('source[media="(prefers-color-scheme: dark)"]')
        ?.getAttribute("srcset"),
    ).toBe("/assets/app-icons/stable-dark.png");
    expect(defaultOption.querySelector("img")?.getAttribute("src")).toBe(
      "/assets/app-icons/stable-light.png",
    );
    expect(screen.getAllByRole("radio")).toHaveLength(4);
    expect(screen.queryByRole("radio", { name: "Production" })).toBeNull();
    expect(screen.getByRole("radio", { name: "Blueprint" })).toBeDefined();
    expect(screen.getByRole("radio", { name: "Sketch" })).toBeDefined();
    expect(screen.queryByText("Blueprint")).toBeNull();

    fireEvent.click(screen.getByRole("radio", { name: "Blueprint" }));

    expect(mocks.applyAppIconPreference).toHaveBeenCalledWith("dev");
    expect(mocks.setAppIcon).toHaveBeenCalledWith("dev");
  });

  it("previews the current channel icon for the default option", () => {
    mocks.appIdentifier = "com.hyprnote.staging";

    render(<AppIconSelector />);

    const defaultOption = screen.getByRole("radio", { name: "Default" });
    expect(defaultOption.querySelector("img")?.getAttribute("src")).toBe(
      "/assets/app-icons/staging-light.png",
    );
    expect(screen.queryByRole("radio", { name: "Sketch" })).toBeNull();
    expect(screen.getByRole("radio", { name: "Production" })).toBeDefined();
  });

  it("selects default for an equivalent channel-specific preference", () => {
    mocks.appIcon = "stable";

    render(<AppIconSelector />);

    expect(
      screen
        .getByRole("radio", { name: "Default" })
        .getAttribute("aria-checked"),
    ).toBe("true");
    expect(screen.queryByRole("radio", { name: "Production" })).toBeNull();
  });

  it("is hidden on platforms that cannot change the running app icon", () => {
    mocks.platform.mockReturnValue("windows");

    render(<AppIconSelector />);

    expect(screen.queryByText("App icon")).toBeNull();
  });
});
