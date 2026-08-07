import {
  cleanup,
  fireEvent,
  render,
  screen,
  within,
} from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  platform: vi.fn(() => "macos"),
  setAppIcon: vi.fn(),
  setAppearance: vi.fn(),
  applyAppIconPreference: vi.fn(),
  appIcon: "default",
  appearance: "auto",
  appIdentifier: "com.hyprnote.stable" as string | undefined,
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
  useSetSettingValue: (key: string) =>
    key === "app_icon_appearance" ? mocks.setAppearance : mocks.setAppIcon,
}));

vi.mock("~/shared/config", () => ({
  useConfigValue: (key: string) =>
    key === "app_icon_appearance" ? mocks.appearance : mocks.appIcon,
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
    mocks.appearance = "auto";
    mocks.appIdentifier = "com.hyprnote.stable";
  });

  const iconOptions = () =>
    within(screen.getByRole("radiogroup", { name: "App icon" })).getAllByRole(
      "radio",
    );

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
    expect(iconOptions()).toHaveLength(4);
    expect(screen.queryByRole("radio", { name: "Production" })).toBeNull();
    expect(screen.getByRole("radio", { name: "Blueprint" })).toBeDefined();
    expect(screen.getByRole("radio", { name: "Sketch" })).toBeDefined();
    expect(screen.queryByText("Blueprint")).toBeNull();

    fireEvent.click(screen.getByRole("radio", { name: "Blueprint" }));

    expect(mocks.applyAppIconPreference).toHaveBeenCalledWith("dev", "auto");
    expect(mocks.setAppIcon).toHaveBeenCalledWith("dev");
  });

  it("defaults the icon appearance to auto and previews both schemes", () => {
    render(<AppIconSelector />);

    const auto = screen.getByRole("radio", { name: "Auto" });
    expect(auto.getAttribute("aria-checked")).toBe("true");
    expect(
      screen
        .getByRole("radio", { name: "Default" })
        .querySelector('source[media="(prefers-color-scheme: dark)"]'),
    ).not.toBeNull();
  });

  it("uses the stable icon while the app identifier is loading", () => {
    mocks.appIdentifier = undefined;

    render(<AppIconSelector />);

    expect(screen.queryByRole("radio", { name: "Production" })).toBeNull();
    expect(screen.getByRole("radio", { name: "Blueprint" })).toBeDefined();
  });

  it("applies and stores an explicit icon appearance", () => {
    render(<AppIconSelector />);

    fireEvent.click(screen.getByRole("radio", { name: "Dark" }));

    expect(mocks.applyAppIconPreference).toHaveBeenCalledWith(
      "default",
      "dark",
    );
    expect(mocks.setAppearance).toHaveBeenCalledWith("dark");
  });

  it("pins previews to the selected appearance", () => {
    mocks.appearance = "dark";

    render(<AppIconSelector />);

    const defaultOption = screen.getByRole("radio", { name: "Default" });
    expect(
      defaultOption.querySelector(
        'source[media="(prefers-color-scheme: dark)"]',
      ),
    ).toBeNull();
    expect(defaultOption.querySelector("img")?.getAttribute("src")).toBe(
      "/assets/app-icons/stable-dark.png",
    );
    expect(
      screen.getByRole("radio", { name: "Dark" }).getAttribute("aria-checked"),
    ).toBe("true");
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
