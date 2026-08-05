import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  setTheme: vi.fn(),
  applyThemePreference: vi.fn(),
  theme: "system",
  appIcon: "default",
  appIconAppearance: "auto",
}));

vi.mock("~/settings/queries", () => ({
  useSetSettingValue: () => mocks.setTheme,
}));

vi.mock("~/shared/config", () => ({
  useConfigValue: (key: string) => {
    if (key === "theme") {
      return mocks.theme;
    }
    if (key === "app_icon_appearance") {
      return mocks.appIconAppearance;
    }
    return mocks.appIcon;
  },
}));

vi.mock("~/shared/theme/provider", () => ({
  applyThemePreference: mocks.applyThemePreference,
}));

import { ThemeSelector } from "./theme";

describe("ThemeSelector", () => {
  afterEach(() => {
    cleanup();
    vi.clearAllMocks();
    mocks.theme = "system";
    mocks.appIcon = "default";
    mocks.appIconAppearance = "auto";
  });

  it("shows visual choices and applies the selected theme immediately", () => {
    render(<ThemeSelector />);

    expect(screen.getAllByRole("radio")).toHaveLength(3);
    expect(
      screen
        .getByRole("radio", { name: /System/ })
        .getAttribute("aria-checked"),
    ).toBe("true");
    expect(screen.getByText("Bright canvas")).toBeTruthy();
    expect(screen.getByText("Low-light canvas")).toBeTruthy();
    expect(screen.getByText("Match your device")).toBeTruthy();

    fireEvent.click(screen.getByRole("radio", { name: /Dark/ }));

    expect(mocks.applyThemePreference).toHaveBeenCalledWith(
      "dark",
      "default",
      "auto",
    );
    expect(mocks.setTheme).toHaveBeenCalledWith("dark");
  });

  it("carries an explicit icon appearance into the theme change", () => {
    mocks.appIconAppearance = "dark";

    render(<ThemeSelector />);

    fireEvent.click(screen.getByRole("radio", { name: /Light/ }));

    expect(mocks.applyThemePreference).toHaveBeenCalledWith(
      "light",
      "default",
      "dark",
    );
  });
});
